//! AXT host flow coverage for DefaultHost and WsvHost.

use std::{collections::HashMap, sync::Arc};

use iroha_crypto::KeyPair;
use iroha_data_model::nexus::{
    AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, DataSpaceId, LaneId,
};
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    axt::{
        self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, RemoteSpendIntent, SpendOp,
        TouchManifest,
    },
    host::DefaultHost,
    mock_wsv::{
        DataspaceAxtPolicy, DomainId as HostDomainId, MockWorldStateView, ScopedAccountId, WsvHost,
    },
    syscalls,
};

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(pty as u16).to_be_bytes());
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

fn sample_wsv_caller() -> ScopedAccountId {
    let kp = KeyPair::random();
    let (public_key, _) = kp.into_parts();
    let domain: HostDomainId = "wonderland".parse().expect("domain id");
    ScopedAccountId::new(domain, public_key)
}

fn begin_with_touch<T: IVMHost>(
    vm: &mut IVM,
    host: &mut T,
    descriptor: &axt::AxtDescriptor,
    manifest: &TouchManifest,
) -> (DataSpaceId, u64) {
    let desc_ptr = store_tlv(
        vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_BEGIN, vm), Ok(0));

    let dsid = descriptor
        .dsids
        .first()
        .copied()
        .expect("descriptor contains dataspace");
    let ds_ptr = store_tlv(
        vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest_ptr = store_tlv(
        vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_TOUCH, vm), Ok(0));
    (dsid, ds_ptr)
}

fn build_handle(
    dsid: DataSpaceId,
    binding: [u8; 32],
    scope: &[&str],
    account: &str,
    remaining: u128,
    per_use: Option<u128>,
) -> AssetHandle {
    AssetHandle {
        scope: scope.iter().map(|s| (*s).to_string()).collect(),
        subject: HandleSubject {
            account: account.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget { remaining, per_use },
        handle_era: 1,
        sub_nonce: 7,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 10,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![1; 32],
        expiry_slot: 99,
        max_clock_skew_ms: Some(0),
    }
}

fn use_handle<T: IVMHost>(
    vm: &mut IVM,
    host: &mut T,
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
    proof: Option<&axt::ProofBlob>,
) -> Result<u64, VMError> {
    let handle_ptr = store_tlv(
        vm,
        PointerType::AssetHandle,
        &norito::to_bytes(handle).expect("encode handle"),
    );
    let intent_ptr = store_tlv(
        vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    match proof {
        Some(p) => {
            let proof_ptr = store_tlv(
                vm,
                PointerType::ProofBlob,
                &norito::to_bytes(p).expect("encode proof"),
            );
            vm.set_register(12, proof_ptr);
        }
        None => vm.set_register(12, 0),
    }
    host.syscall(syscalls::SYSCALL_USE_ASSET_HANDLE, vm)
}

#[test]
fn default_host_axt_flow_happy_path() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(42);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_bytes = norito::to_bytes(&descriptor).expect("encode descriptor");
    let desc_ptr = store_tlv(&mut vm, PointerType::AxtDescriptor, &desc_bytes);
    vm.set_register(10, desc_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm), Ok(0));

    let ds_bytes = norito::to_bytes(&dsid).expect("encode dsid");
    let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
    let manifest = TouchManifest {
        read: vec!["orders/123".into()],
        write: vec!["ledger/123".into()],
    };
    let manifest_bytes = norito::to_bytes(&manifest).expect("encode manifest");
    let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm), Ok(0));

    let binding = axt::compute_binding(&descriptor).expect("compute binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
                .into(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 500,
            per_use: Some(500),
        },
        handle_era: 1,
        sub_nonce: 7,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 10,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![1; 32],
        expiry_slot: 99,
        max_clock_skew_ms: Some(0),
    };
    let handle_bytes = norito::to_bytes(&handle).expect("encode handle");
    let handle_ptr = store_tlv(&mut vm, PointerType::AssetHandle, &handle_bytes);

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "200".into(),
        },
    };
    let intent_bytes = norito::to_bytes(&intent).expect("encode intent");
    let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &intent_bytes);

    let handle_proof = axt::ProofBlob {
        payload: vec![1, 2, 3, 4],
        expiry_slot: None,
    };
    let proof_bytes = norito::to_bytes(&handle_proof).expect("encode proof");
    let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);

    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    // Register an explicit DS proof for completeness
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm), Ok(0));
}

#[test]
fn asset_handle_roundtrip_preserves_origin_dsid() {
    let dsid = DataSpaceId::new(7);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec![],
            write: vec![],
        }],
    };
    let binding = axt::compute_binding(&descriptor).expect("compute binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        Some(10),
    );

    let bytes = norito::to_bytes(&handle).expect("encode handle");
    let decoded: AssetHandle =
        norito::decode_from_bytes(&bytes).expect("decode handle bytes should succeed");

    assert_eq!(decoded.subject.origin_dsid, Some(dsid));
}

#[test]
fn handle_subject_roundtrip() {
    let dsid = DataSpaceId::new(11);
    let subject = HandleSubject {
        account: "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"
            .to_string(),
        origin_dsid: Some(dsid),
    };

    let bytes = norito::to_bytes(&subject).expect("encode subject");
    let decoded: HandleSubject =
        norito::decode_from_bytes(&bytes).expect("decode subject bytes should succeed");

    assert_eq!(decoded.origin_dsid, Some(dsid));
}

#[test]
fn dataspace_id_roundtrip() {
    let dsid = DataSpaceId::new(13);
    let bytes = norito::to_bytes(&dsid).expect("encode dsid");
    let decoded: DataSpaceId =
        norito::decode_from_bytes(&bytes).expect("decode dsid should succeed");
    assert_eq!(decoded, dsid);
}

#[test]
fn default_host_rejects_binding_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(7);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.axt_binding = vec![9; 32];
    handle.axt_binding[0] ^= 0xFF;

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, None);
    assert!(matches!(result, Err(VMError::PermissionDenied)));
}

#[test]
fn default_host_allows_multiple_handle_usages_within_budget() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(11);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/alpha".into()],
        write: vec!["ledger/alpha".into()],
    };
    let (_dsid, ds_ptr) = begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        300,
        Some(200),
    );
    let proof = axt::ProofBlob {
        payload: vec![9, 9, 9],
        expiry_slot: None,
    };

    let first_intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "150".into(),
        },
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &first_intent, Some(&proof)),
        Ok(0)
    );

    let second_intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
            amount: "40".into(),
        },
    };
    handle.sub_nonce += 1;
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &second_intent, Some(&proof)),
        Ok(0)
    );

    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm), Ok(0));
}

#[test]
fn default_host_rejects_handle_scope_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(12);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/999".into()],
        write: vec!["ledger/999".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        50,
        None,
    );
    let proof = axt::ProofBlob {
        payload: vec![1],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "burn".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "10".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof));
    assert!(
        matches!(result, Err(VMError::PermissionDenied)),
        "unexpected result {result:?}"
    );
}

#[test]
fn default_host_rejects_handle_subject_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(13);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/42".into()],
        write: vec!["ledger/42".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        80,
        None,
    );
    let proof = axt::ProofBlob {
        payload: vec![1],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "10".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof));
    assert!(
        matches!(result, Err(VMError::PermissionDenied)),
        "unexpected result {result:?}"
    );
}

#[test]
fn default_host_rejects_commit_without_required_proof() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(14);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/abc".into()],
        write: vec!["ledger/abc".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        60,
        None,
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "10".into(),
        },
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &intent, None),
        Ok(0)
    );
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn handle_proof_satisfies_dataspace_requirement() {
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
    let manifest = TouchManifest {
        read: vec!["orders/h1".into()],
        write: vec!["ledger/h1".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        90,
        None,
    );
    let proof = axt::ProofBlob {
        payload: vec![7],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "15".into(),
        },
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Ok(0)
    );

    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm), Ok(0));
}

#[test]
fn default_host_rejects_handle_with_invalid_manifest_root() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(22);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/bad".into()],
        write: vec!["ledger/bad".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        50,
        None,
    );
    handle.manifest_view_root = vec![1, 2, 3]; // invalid length
    let proof = axt::ProofBlob {
        payload: vec![3],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "10".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof));
    assert!(
        matches!(result, Err(VMError::NoritoInvalid)),
        "unexpected result {result:?}"
    );
}

#[test]
fn default_host_rejects_handle_with_empty_scope() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(23);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/scope".into()],
        write: vec!["ledger/scope".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.scope.clear();
    let proof = axt::ProofBlob {
        payload: vec![4],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof));
    assert!(
        matches!(result, Err(VMError::PermissionDenied)),
        "unexpected result {result:?}"
    );
}

#[test]
fn default_host_rejects_handle_with_zero_era_or_nonce_or_expiry() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(24);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/era".into()],
        write: vec!["ledger/era".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let base = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    let proof = axt::ProofBlob {
        payload: vec![5],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };

    let mut zero_era = base.clone();
    zero_era.handle_era = 0;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &zero_era, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut zero_nonce = base.clone();
    zero_nonce.sub_nonce = 0;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &zero_nonce, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut zero_expiry = base.clone();
    zero_expiry.expiry_slot = 0;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &zero_expiry, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn default_host_rejects_handle_with_zero_budget_or_empty_group() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(25);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/budget".into()],
        write: vec!["ledger/budget".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut base = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    let proof = axt::ProofBlob {
        payload: vec![6],
        expiry_slot: None,
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };

    base.budget.remaining = 0;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &base, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut empty_group = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    empty_group.group_binding.composability_group_id.clear();
    assert!(matches!(
        use_handle(&mut vm, &mut host, &empty_group, &intent, Some(&proof)),
        Err(VMError::NoritoInvalid)
    ));
}

#[test]
fn commit_requires_proof_for_every_dataspace() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let ds_a = DataSpaceId::new(30);
    let ds_b = DataSpaceId::new(31);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![ds_a, ds_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: ds_a,
                read: vec!["orders/a".into()],
                write: vec!["ledger/a".into()],
            },
            axt::AxtTouchSpec {
                dsid: ds_b,
                read: vec!["orders/b".into()],
                write: vec!["ledger/b".into()],
            },
        ],
    };
    let manifest_a = TouchManifest {
        read: vec!["orders/a/1".into()],
        write: vec!["ledger/a/1".into()],
    };
    let manifest_b = TouchManifest {
        read: vec!["orders/b/1".into()],
        write: vec!["ledger/b/1".into()],
    };

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm), Ok(0));

    // Touch ds_a
    let ds_a_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&ds_a).expect("encode ds"),
    );
    let manifest_a_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest_a).expect("encode manifest"),
    );
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, manifest_a_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm), Ok(0));

    // Touch ds_b
    let ds_b_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&ds_b).expect("encode ds"),
    );
    let manifest_b_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest_b).expect("encode manifest"),
    );
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, manifest_b_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm), Ok(0));

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = build_handle(
        ds_a,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    let mut handle_b = build_handle(
        ds_b,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle_b.sub_nonce = handle_a.sub_nonce + 1;
    let proof_a = axt::ProofBlob {
        payload: vec![8],
        expiry_slot: None,
    };
    let proof_b = axt::ProofBlob {
        payload: vec![9],
        expiry_slot: None,
    };

    // Attach proofs only via handles
    let intent_a = RemoteSpendIntent {
        asset_dsid: ds_a,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle_a, &intent_a, Some(&proof_a)),
        Ok(0)
    );
    let intent_b = RemoteSpendIntent {
        asset_dsid: ds_b,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle_b, &intent_b, Some(&proof_b)),
        Ok(0)
    );

    // Missing proof for ds_b would be rejected; we provided both so commit should pass.
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm), Ok(0));

    // Now show failure if a dataspace proof is absent
    let mut vm_fail = IVM::new(1_000_000);
    let mut host_fail = DefaultHost::new();
    let desc_ptr_fail = store_tlv(
        &mut vm_fail,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm_fail.set_register(10, desc_ptr_fail);
    assert_eq!(
        host_fail.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm_fail),
        Ok(0)
    );
    let ds_a_ptr_fail = store_tlv(
        &mut vm_fail,
        PointerType::DataSpaceId,
        &norito::to_bytes(&ds_a).expect("encode ds"),
    );
    let manifest_a_ptr_fail = store_tlv(
        &mut vm_fail,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest_a).expect("encode manifest"),
    );
    vm_fail.set_register(10, ds_a_ptr_fail);
    vm_fail.set_register(11, manifest_a_ptr_fail);
    assert_eq!(
        host_fail.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail),
        Ok(0)
    );
    let ds_b_ptr_fail = store_tlv(
        &mut vm_fail,
        PointerType::DataSpaceId,
        &norito::to_bytes(&ds_b).expect("encode ds"),
    );
    let manifest_b_ptr_fail = store_tlv(
        &mut vm_fail,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest_b).expect("encode manifest"),
    );
    vm_fail.set_register(10, ds_b_ptr_fail);
    vm_fail.set_register(11, manifest_b_ptr_fail);
    assert_eq!(
        host_fail.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail),
        Ok(0)
    );

    assert_eq!(
        use_handle(
            &mut vm_fail,
            &mut host_fail,
            &handle_a,
            &intent_a,
            Some(&proof_a)
        ),
        Ok(0)
    );
    assert!(matches!(
        host_fail.syscall(syscalls::SYSCALL_AXT_COMMIT, &mut vm_fail),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn axt_begin_rejects_invalid_descriptor() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    // Duplicate dsids should be rejected
    let dup_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(1), DataSpaceId::new(1)],
        touches: Vec::new(),
    };
    let dup_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&dup_descriptor).expect("encode"),
    );
    vm.set_register(10, dup_ptr);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));

    // Touch for unknown dsid should be rejected
    let bad_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(2)],
        touches: vec![axt::AxtTouchSpec {
            dsid: DataSpaceId::new(9),
            read: vec![],
            write: vec![],
        }],
    };
    let bad_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&bad_touch_descriptor).expect("encode"),
    );
    vm.set_register(10, bad_ptr);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));

    // Duplicate touch for same dsid should be rejected
    let dup_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(3)],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(3),
                read: vec![],
                write: vec![],
            },
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(3),
                read: vec![],
                write: vec![],
            },
        ],
    };
    let dup_touch_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&dup_touch_descriptor).expect("encode"),
    );
    vm.set_register(10, dup_touch_ptr);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

struct DenyTouchPolicy {
    denied: DataSpaceId,
}

impl axt::AxtPolicy for DenyTouchPolicy {
    fn allow_touch(
        &self,
        dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        if dsid == self.denied {
            Err(VMError::PermissionDenied)
        } else {
            Ok(())
        }
    }

    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Ok(())
    }
}

struct DenyHandlePolicy;

impl axt::AxtPolicy for DenyHandlePolicy {
    fn allow_touch(
        &self,
        _dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        Ok(())
    }

    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Err(VMError::PermissionDenied)
    }
}

#[test]
fn axt_policy_rejects_touch() {
    let mut vm = IVM::new(1_000_000);
    let policy = Arc::new(DenyTouchPolicy {
        denied: DataSpaceId::new(77),
    });
    let mut host = DefaultHost::new().with_axt_policy(policy);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(77)],
        touches: vec![axt::AxtTouchSpec {
            dsid: DataSpaceId::new(77),
            read: vec![],
            write: vec![],
        }],
    };
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm), Ok(0));

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&descriptor.dsids[0]).expect("encode"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_policy_checks_root_and_expiry() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(120);
    let manifest_root = [9u8; 32];
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller),
        HashMap::new(),
    )
    .with_axt_manifest_root(dsid, manifest_root);
    host.set_current_time_ms(50);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/policy".into()],
        write: vec!["ledger/policy".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut expired_handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    expired_handle.expiry_slot = 40; // before current slot
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &expired_handle, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut bad_root = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    bad_root.expiry_slot = 60;
    bad_root.manifest_view_root = vec![1; 32];
    assert!(matches!(
        use_handle(&mut vm, &mut host, &bad_root, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut ok_handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    ok_handle.expiry_slot = 60;
    ok_handle.manifest_view_root = manifest_root.to_vec();
    assert_eq!(
        use_handle(&mut vm, &mut host, &ok_handle, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn wsv_host_uses_slot_length_and_skew_for_expiry() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(140);
    let manifest_root = [0x7A; 32];
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_max_clock_skew_ms(15);
    wsv.set_current_time_ms(25); // current_slot = 2
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );
    let mut host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&caller), HashMap::new());

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/slot".into()],
        write: vec!["ledger/slot".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.expiry_slot = 1;
    handle.max_clock_skew_ms = Some(15);
    handle.manifest_view_root = manifest_root.to_vec();
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn wsv_host_rejects_handle_skew_above_config() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(141);
    let manifest_root = [0x8B; 32];
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_max_clock_skew_ms(5);
    wsv.set_current_time_ms(20); // current_slot = 2
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );
    let mut host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&caller), HashMap::new());

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/skew".into()],
        write: vec!["ledger/skew".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.expiry_slot = 10;
    handle.max_clock_skew_ms = Some(10);
    handle.manifest_view_root = manifest_root.to_vec();
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_accepts_proof_within_skew() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(142);
    let manifest_root = [0xAB; 32];
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_max_clock_skew_ms(15);
    wsv.set_current_time_ms(25); // current_slot = 2
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );
    let mut host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&caller), HashMap::new());

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/skew".into()],
        write: vec!["ledger/skew".into()],
    };
    let (_dsid, ds_ptr) = begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: Some(1),
    };
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );
}

#[test]
fn wsv_host_rejects_inline_proof_expired_with_skew() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(143);
    let manifest_root = [0xBC; 32];
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_max_clock_skew_ms(5);
    wsv.set_current_time_ms(50); // current_slot = 5
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );
    let mut host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&caller), HashMap::new());

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/inline".into()],
        write: vec!["ledger/inline".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.expiry_slot = 20;
    handle.manifest_view_root = manifest_root.to_vec();
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: Some(3),
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_rejects_zero_manifest_root_and_handle_root() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(125);
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    )
    .with_axt_manifest_root(dsid, [0; 32]);
    host.set_current_time_ms(10);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/zero".into()],
        write: vec!["ledger/zero".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut zero_root_handle =
        build_handle(dsid, binding, &["transfer"], &caller.to_string(), 10, None);
    zero_root_handle.manifest_view_root = vec![0; 32];
    zero_root_handle.expiry_slot = 20;
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: caller.to_string(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: vec![0; 32],
        expiry_slot: Some(30),
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &zero_root_handle, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_rejects_missing_policy_binding() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(126);
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    host.set_current_time_ms(5);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/missing".into()],
        write: vec!["ledger/missing".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(dsid, binding, &["transfer"], &caller.to_string(), 10, None);
    handle.manifest_view_root = vec![5; 32];
    handle.expiry_slot = 10;
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: caller.to_string(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: vec![7],
        expiry_slot: Some(15),
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_policy_checks_target_lane() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(121);
    let manifest_root = [1u8; 32];
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller),
        HashMap::new(),
    )
    .with_axt_target_lane(dsid, 7);
    host.set_axt_manifest_root(dsid, manifest_root);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/lane".into()],
        write: vec!["ledger/lane".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut wrong_lane = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    wrong_lane.target_lane = LaneId::new(1);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };
    assert!(matches!(
        use_handle(&mut vm, &mut host, &wrong_lane, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut ok_lane = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    ok_lane.target_lane = LaneId::new(7);
    assert_eq!(
        use_handle(&mut vm, &mut host, &ok_lane, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn wsv_host_applies_policy_snapshot_lane_and_root() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(130);
    let manifest_root = [0x22; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(4),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    }];
    let policy_snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    )
    .with_axt_policy_snapshot(policy_snapshot);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/snap".into()],
        write: vec!["ledger/snap".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: caller.to_string(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };

    let mut wrong_lane = build_handle(dsid, binding, &["transfer"], &caller.to_string(), 10, None);
    wrong_lane.target_lane = LaneId::new(1);
    wrong_lane.manifest_view_root = manifest_root.to_vec();
    assert!(matches!(
        use_handle(&mut vm, &mut host, &wrong_lane, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut ok_lane = build_handle(dsid, binding, &["transfer"], &caller.to_string(), 10, None);
    ok_lane.target_lane = LaneId::new(4);
    ok_lane.manifest_view_root = manifest_root.to_vec();
    assert_eq!(
        use_handle(&mut vm, &mut host, &ok_lane, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn wsv_host_respects_explicit_policy_slot_over_time() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(150);
    let manifest_root = [0xDD; 32];
    let mut wsv = MockWorldStateView::new();
    wsv.set_current_time_ms(100); // current_slot = 100 (slot_length default = 1)
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 5,
        },
    );
    let mut host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&caller), HashMap::new());

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/slot".into()],
        write: vec!["ledger/slot".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    handle.expiry_slot = 10;
    handle.manifest_view_root = manifest_root.to_vec();
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };
    assert_eq!(
        use_handle(&mut vm, &mut host, &handle, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn wsv_host_policy_checks_min_era_and_nonce() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(122);
    let manifest_root = [1u8; 32];
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller),
        HashMap::new(),
    )
    .with_axt_min_handle_era(dsid, 3)
    .with_axt_min_sub_nonce(dsid, 5);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/era".into()],
        write: vec!["ledger/era".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);
    host.set_axt_manifest_root(dsid, manifest_root);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: None,
    };

    let mut low_era = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    low_era.handle_era = 2;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &low_era, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut low_nonce = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    low_nonce.sub_nonce = 4;
    assert!(matches!(
        use_handle(&mut vm, &mut host, &low_nonce, &intent, Some(&proof)),
        Err(VMError::PermissionDenied)
    ));

    let mut ok = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    ok.handle_era = 3;
    ok.sub_nonce = 5;
    assert_eq!(
        use_handle(&mut vm, &mut host, &ok, &intent, Some(&proof)),
        Ok(0)
    );
}

#[test]
fn axt_policy_rejects_handle_usage() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_axt_policy(Arc::new(DenyHandlePolicy));

    let dsid = DataSpaceId::new(88);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["orders/policy".into()],
        write: vec!["ledger/policy".into()],
    };
    begin_with_touch(&mut vm, &mut host, &descriptor, &manifest);

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = build_handle(
        dsid,
        binding,
        &["transfer"],
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        10,
        None,
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB".into(),
            to: "sorauロ1Q2クBKzrシStハYyXフ1ケHソセkSveノyサネHラソug7zWムヰyRMH888".into(),
            amount: "1".into(),
        },
    };
    let result = use_handle(&mut vm, &mut host, &handle, &intent, None);
    assert!(matches!(result, Err(VMError::PermissionDenied)));
}

#[test]
fn wsv_host_rejects_invalid_descriptor() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller),
        HashMap::new(),
    );

    let descriptor = axt::AxtDescriptor {
        dsids: Vec::new(),
        touches: Vec::new(),
    };
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn wsv_host_applies_axt_policy() {
    let mut vm = IVM::new(1_000_000);
    let caller = sample_wsv_caller();
    let dsid = DataSpaceId::new(99);
    let policy = Arc::new(DenyTouchPolicy { denied: dsid });
    let mut host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        ivm::mock_wsv::AccountId::from(&caller),
        HashMap::new(),
    )
    .with_axt_policy(policy);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: Vec::new(),
            write: Vec::new(),
        }],
    };
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert_eq!(host.syscall(syscalls::SYSCALL_AXT_BEGIN, &mut vm), Ok(0));

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
