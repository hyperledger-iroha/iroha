//! Ensure CoreHost enforces the same ABI gating as DefaultHost.
#[path = "../../iroha_data_model/tests/fixtures/axt_golden.rs"]
mod axt_golden;
use std::{collections::HashMap, sync::Arc};

use iroha_data_model::{
    nexus as model,
    nexus::{AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, DataSpaceId, LaneId},
};
use ivm::{
    CoreHost, IVM, IVMHost, PointerType, VMError,
    axt::{
        self, AssetHandle, AxtProofEnvelope, GroupBinding, HandleBudget, HandleSubject, ProofBlob,
        RemoteSpendIntent, SpendOp, TouchManifest,
    },
    mock_wsv::{DataspaceAxtPolicy, MockWorldStateView, SpaceDirectoryAxtPolicy},
};
mod common;

#[test]
fn core_host_rejects_unknown_syscalls() {
    let mut vm = IVM::new(0);
    let mut host = CoreHost::new();
    // Choose an unknown syscall number outside the canonical list
    let res = host.syscall(0xFFFF, &mut vm);
    assert!(matches!(res, Err(VMError::UnknownSyscall(_))));
}

fn make_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(ty, payload);
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ty as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

fn store_tlv(vm: &mut IVM, ty: PointerType, value: &[u8]) -> u64 {
    let tlv = make_tlv(ty, value);
    vm.alloc_input_tlv(&tlv).expect("alloc input")
}

fn make_descriptor(dsid: DataSpaceId) -> axt::AxtDescriptor {
    axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    }
}

fn make_handle(
    binding: [u8; 32],
    target_lane: LaneId,
    manifest_root: [u8; 32],
    handle_era: u64,
    sub_nonce: u64,
    expiry_slot: u64,
) -> AssetHandle {
    AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "alice@wonderland".into(),
            origin_dsid: None,
        },
        budget: HandleBudget {
            remaining: 200,
            per_use: Some(150),
        },
        handle_era,
        sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane,
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot,
        max_clock_skew_ms: Some(0),
    }
}

fn make_policy_snapshot(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    target_lane: LaneId,
    min_handle_era: u64,
    min_sub_nonce: u64,
    current_slot: u64,
) -> AxtPolicySnapshot {
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane,
            min_handle_era,
            min_sub_nonce,
            current_slot,
        },
    }];
    AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    }
}

fn proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_bytes: Vec<u8>,
    expiry_slot: u64,
) -> ProofBlob {
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: proof_bytes,
    };
    ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
        expiry_slot: Some(expiry_slot),
    }
}

#[test]
fn core_host_handles_axt_syscalls_with_valid_tlvs() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(7);
    let manifest_root = [0x11; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 1);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
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
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = make_handle(binding, LaneId::new(0), manifest_root, 1, 42, 10);
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
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

    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
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
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );

    // Follow-up calls without an active AXT sequence must be rejected.
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_rejects_duplicate_touch() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(8);
    let snapshot = make_policy_snapshot(dsid, [0x12; 32], LaneId::new(0), 1, 1, 1);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/dup".into()],
        write: vec!["ledger/dup".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("first touch");

    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_rejects_zero_manifest_root_ds_proof() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(9);
    let snapshot = make_policy_snapshot(dsid, [0; 32], LaneId::new(0), 1, 1, 2);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/proof".into()],
        write: vec!["ledger/proof".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch recorded");

    let proof = proof_blob_for(dsid, [0; 32], vec![0xA5], 10);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

struct DenyAllPolicy;

impl axt::AxtPolicy for DenyAllPolicy {
    fn allow_touch(
        &self,
        _dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        Err(VMError::PermissionDenied)
    }

    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Err(VMError::PermissionDenied)
    }
}

#[test]
fn core_host_policy_rejects_touch_when_policy_denies() {
    let mut vm = IVM::new(1_000_000);
    let host = CoreHost::new().with_axt_policy(Arc::new(DenyAllPolicy));
    let mut host = host;

    let dsid = DataSpaceId::new(3);
    let descriptor = make_descriptor(dsid);
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
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_enforces_space_directory_policy_on_handles() {
    let dsid = DataSpaceId::new(9);
    let descriptor = make_descriptor(dsid);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut policies = HashMap::new();
    policies.insert(
        dsid,
        DataspaceAxtPolicy {
            manifest_root: [1; 32],
            target_lane: LaneId::new(2),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 3,
        },
    );
    let policy = SpaceDirectoryAxtPolicy::from_snapshot(policies);
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new().with_axt_policy(Arc::new(policy));

    // Begin AXT
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

    // Touch the dataspace so handles are permitted.
    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    // Lane/manifest match → allowed.
    let handle = make_handle(binding, LaneId::new(2), [1; 32], 2, 2, 10);
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
            amount: "50".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    // Lane mismatch → denied.
    let bad_handle = make_handle(binding, LaneId::new(3), [1; 32], 2, 3, 10);
    let bad_handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&bad_handle).expect("encode handle"),
    );
    vm.set_register(10, bad_handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

fn fixture_intent(dsid: DataSpaceId) -> RemoteSpendIntent {
    RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "alice@wonderland".into(),
            to: "merchant@wonderland".into(),
            amount: "50".into(),
        },
    }
}

fn convert_descriptor(model: &model::AxtDescriptor) -> axt::AxtDescriptor {
    axt::AxtDescriptor {
        dsids: model.dsids.clone(),
        touches: model
            .touches
            .iter()
            .map(|touch| axt::AxtTouchSpec {
                dsid: touch.dsid,
                read: touch.read.clone(),
                write: touch.write.clone(),
            })
            .collect(),
    }
}

fn convert_handle(model: &model::AssetHandle) -> AssetHandle {
    AssetHandle {
        scope: model.scope.clone(),
        subject: HandleSubject {
            account: model.subject.account.clone(),
            origin_dsid: model.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: model.budget.remaining,
            per_use: model.budget.per_use,
        },
        handle_era: model.handle_era,
        sub_nonce: model.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: model.group_binding.composability_group_id.clone(),
            epoch_id: model.group_binding.epoch_id,
        },
        target_lane: model.target_lane,
        axt_binding: model.axt_binding.as_bytes().to_vec(),
        manifest_view_root: model.manifest_view_root.to_vec(),
        expiry_slot: model.expiry_slot,
        max_clock_skew_ms: model.max_clock_skew_ms,
    }
}

fn run_policy_snapshot_case(
    snapshot: &AxtPolicySnapshot,
    dsid: DataSpaceId,
    mut mutate_handle: impl FnMut(&mut AssetHandle),
) -> Result<u64, VMError> {
    let descriptor = make_descriptor(dsid);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let policy_entry = snapshot
        .entries
        .iter()
        .find(|entry| entry.dsid == dsid)
        .map(|entry| entry.policy);
    let mut host = CoreHost::new().with_axt_policy_snapshot(snapshot);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/policy".into()],
        write: vec!["ledger/policy".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;

    let mut handle = make_handle(
        binding,
        policy_entry.map_or_else(|| LaneId::new(0), |entry| entry.target_lane),
        policy_entry.map_or([0; 32], |entry| entry.manifest_root),
        policy_entry.map_or(1, |entry| core::cmp::max(entry.min_handle_era, 1)),
        policy_entry.map_or(1, |entry| core::cmp::max(entry.min_sub_nonce, 1)),
        policy_entry.map_or(10, |entry| {
            core::cmp::max(entry.current_slot.saturating_add(5), 1)
        }),
    );
    mutate_handle(&mut handle);

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
            amount: "50".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}

#[test]
fn core_host_builds_space_directory_policy_from_snapshot() {
    let dsid = DataSpaceId::new(11);
    let snapshot = make_policy_snapshot(dsid, [0xAB; 32], LaneId::new(6), 4, 7, 30);
    assert_eq!(run_policy_snapshot_case(&snapshot, dsid, |_| {}), Ok(0));
}

#[test]
fn core_host_policy_snapshot_rejects_policy_mismatches() {
    let dsid = DataSpaceId::new(12);
    let snapshot = make_policy_snapshot(dsid, [0xCD; 32], LaneId::new(3), 5, 9, 40);

    let manifest_root_mismatch = run_policy_snapshot_case(&snapshot, dsid, |handle| {
        handle.manifest_view_root = vec![0xEE; 32];
    });
    assert!(matches!(
        manifest_root_mismatch,
        Err(VMError::PermissionDenied)
    ));

    let expired_handle = run_policy_snapshot_case(&snapshot, dsid, |handle| {
        handle.expiry_slot = 35;
    });
    assert!(matches!(expired_handle, Err(VMError::PermissionDenied)));

    let low_handle_era = run_policy_snapshot_case(&snapshot, dsid, |handle| {
        handle.handle_era = 4;
    });
    assert!(matches!(low_handle_era, Err(VMError::PermissionDenied)));

    let low_sub_nonce = run_policy_snapshot_case(&snapshot, dsid, |handle| {
        handle.sub_nonce = 8;
    });
    assert!(matches!(low_sub_nonce, Err(VMError::PermissionDenied)));
}

#[test]
fn core_host_policy_snapshot_rejects_unknown_dataspace() {
    let snapshot = make_policy_snapshot(DataSpaceId::new(99), [0x11; 32], LaneId::new(1), 1, 1, 10);
    let res = run_policy_snapshot_case(&snapshot, DataSpaceId::new(100), |_| {});
    assert!(matches!(res, Err(VMError::PermissionDenied)));
}

fn run_wsv_policy_case(
    wsv: MockWorldStateView,
    dsid: DataSpaceId,
    mut mutate_handle: impl FnMut(&mut AssetHandle),
) -> Result<u64, VMError> {
    let descriptor = make_descriptor(dsid);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let policies = wsv.axt_policy_snapshot();
    let policy = policies.get(&dsid).cloned().unwrap_or_default();
    let mut host = CoreHost::new().with_wsv_policy(&wsv);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/wsv".into()],
        write: vec!["ledger/wsv".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;

    let mut handle = make_handle(
        binding,
        policy.target_lane,
        policy.manifest_root,
        policy.min_handle_era.max(1),
        policy.min_sub_nonce.max(1),
        wsv.current_slot().saturating_add(5),
    );
    mutate_handle(&mut handle);

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
            amount: "50".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}

#[test]
fn core_host_builds_policy_from_wsv_snapshot() {
    let dsid = DataSpaceId::new(77);
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_current_time_ms(50); // current_slot = 5
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root: [0x99; 32],
            target_lane: LaneId::new(4),
            min_handle_era: 2,
            min_sub_nonce: 3,
            current_slot: 0,
        },
    );

    let res = run_wsv_policy_case(wsv, dsid, |_| {});
    assert_eq!(res, Ok(0));
}

#[test]
fn core_host_wsv_policy_rejects_lane_and_expiry_mismatches() {
    let dsid = DataSpaceId::new(78);
    let mut wsv = MockWorldStateView::new();
    wsv.set_slot_length_ms(10);
    wsv.set_current_time_ms(30); // current_slot = 3
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root: [0x55; 32],
            target_lane: LaneId::new(2),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let wrong_lane = run_wsv_policy_case(wsv.clone(), dsid, |handle| {
        handle.target_lane = LaneId::new(1);
    });
    assert!(matches!(wrong_lane, Err(VMError::PermissionDenied)));

    let expired = run_wsv_policy_case(wsv, dsid, |handle| {
        handle.expiry_slot = 2; // less than current slot -> expired
    });
    assert!(matches!(expired, Err(VMError::PermissionDenied)));
}

#[test]
fn core_host_wsv_policy_respects_explicit_current_slot() {
    let dsid = DataSpaceId::new(79);
    let mut wsv = MockWorldStateView::new();
    wsv.set_current_time_ms(100); // current_slot = 100
    wsv.set_axt_policy(
        dsid,
        DataspaceAxtPolicy {
            manifest_root: [0x33; 32],
            target_lane: LaneId::new(2),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 5,
        },
    );

    let res = run_wsv_policy_case(wsv, dsid, |handle| {
        handle.expiry_slot = 10;
    });
    assert_eq!(res, Ok(0));
}

#[test]
fn core_host_enforces_policy_snapshot() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(21);
    let manifest_root = [0xAA; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(2),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    }];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
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
        read: vec!["orders/policy".into()],
        write: vec!["ledger/policy".into()],
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

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let mut handle = make_handle(binding, LaneId::new(1), manifest_root, 1, 10, 5);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "alice@wonderland".into(),
            to: "merchant@wonderland".into(),
            amount: "1".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    let proof = proof_blob_for(dsid, manifest_root, vec![0xBB], 10);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );

    // Wrong lane -> reject
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));

    // Correct lane -> accept
    handle.target_lane = LaneId::new(2);
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    vm.set_register(10, handle_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );
}

#[test]
fn core_host_rejects_inline_proof_manifest_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(22);
    let manifest_root = [0xAB; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 4);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
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
        read: vec!["orders/inline".into()],
        write: vec!["ledger/inline".into()],
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

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = make_handle(binding, LaneId::new(0), manifest_root, 1, 2, 8);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "alice@wonderland".into(),
            to: "merchant@wonderland".into(),
            amount: "2".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    let wrong_proof = proof_blob_for(dsid, [0xCD; 32], vec![0xEE, 0xEE], 6);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&wrong_proof).expect("encode proof"),
    );

    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_accepts_proof_within_skew() {
    use std::num::NonZeroU64;

    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(23);
    let manifest_root = [0xAC; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 5);
    let mut host = CoreHost::new()
        .with_axt_timing(NonZeroU64::new(1).expect("slot length"), 1)
        .with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
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
    let proof = proof_blob_for(dsid, manifest_root, vec![0xAA], 4);
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
}

#[test]
fn core_host_rejects_inline_proof_zero_expiry_slot() {
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(24);
    let manifest_root = [0xAD; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 0);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);

    let descriptor = make_descriptor(dsid);
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
        read: vec!["orders/inline".into()],
        write: vec!["ledger/inline".into()],
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

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = make_handle(binding, LaneId::new(0), manifest_root, 1, 1, 10);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "alice@wonderland".into(),
            to: "merchant@wonderland".into(),
            amount: "1".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    let proof = proof_blob_for(dsid, manifest_root, vec![0xBB], 0);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::NoritoInvalid)
    ));
}

fn exercise_fixture_handle(
    descriptor: &axt::AxtDescriptor,
    snapshot: &AxtPolicySnapshot,
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
) -> Result<u64, VMError> {
    let slot_length_ms = std::num::NonZeroU64::new(1).expect("non-zero slot length");
    let max_clock_skew_ms = handle.max_clock_skew_ms.map(u64::from).unwrap_or(0);
    let mut host = CoreHost::new()
        .with_axt_timing(slot_length_ms, max_clock_skew_ms)
        .with_axt_policy_snapshot(snapshot);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;

    let dsid = descriptor
        .dsids
        .first()
        .copied()
        .expect("descriptor contains a dataspace");
    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/ready".into()],
        write: vec!["ledger/ready".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;

    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(handle).expect("encode handle"),
    );
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}

#[test]
fn core_host_enforces_fixture_snapshot_fields() {
    let model_descriptor: model::AxtDescriptor =
        norito::decode_from_bytes(axt_golden::AXT_DESCRIPTOR).expect("decode descriptor");
    let descriptor = convert_descriptor(&model_descriptor);
    let snapshot: AxtPolicySnapshot =
        norito::decode_from_bytes(axt_golden::AXT_POLICY).expect("decode snapshot");
    let model_handle: model::AssetHandle =
        norito::decode_from_bytes(axt_golden::AXT_HANDLE).expect("decode handle");
    let base_handle = convert_handle(&model_handle);
    let policy_entry = snapshot
        .entries
        .first()
        .expect("snapshot contains policy entry");
    let base_intent = fixture_intent(
        descriptor
            .dsids
            .first()
            .copied()
            .expect("dataspace id present"),
    );

    {
        use std::num::NonZeroU64;

        use axt::AxtPolicy;
        let dsid = base_intent.asset_dsid;
        let binding = axt::compute_binding(&descriptor).expect("binding");
        assert_eq!(
            base_handle
                .binding_array()
                .expect("fixture handle must carry binding bytes"),
            binding,
            "fixture handle binding must match descriptor"
        );
        let mut state = axt::HostAxtState::new(descriptor.clone(), binding);
        state
            .record_touch(
                dsid,
                TouchManifest {
                    read: vec!["orders/ready".into()],
                    write: vec!["ledger/ready".into()],
                },
            )
            .expect("touch recorded");
        let usage = axt::HandleUsage {
            handle: base_handle.clone(),
            intent: base_intent.clone(),
            proof: None,
            amount: 50,
        };
        let max_clock_skew_ms = base_handle.max_clock_skew_ms.map(u64::from).unwrap_or(0);
        let policy = SpaceDirectoryAxtPolicy::from_policy_snapshot_with_timing(
            &snapshot,
            NonZeroU64::new(1).expect("non-zero slot length"),
            max_clock_skew_ms,
        );
        policy
            .allow_handle(&usage)
            .expect("policy should allow fixture handle");
        state
            .record_handle(usage)
            .expect("state should accept fixture handle");
    }

    assert_eq!(
        exercise_fixture_handle(&descriptor, &snapshot, &base_handle, &base_intent),
        Ok(0)
    );

    let mut wrong_lane = base_handle.clone();
    wrong_lane.target_lane = LaneId::new(policy_entry.policy.target_lane.as_u32() + 1);
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &wrong_lane, &base_intent),
        Err(VMError::PermissionDenied)
    ));

    let mut wrong_manifest_root = base_handle.clone();
    wrong_manifest_root.manifest_view_root = vec![0xFF; 32];
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &wrong_manifest_root, &base_intent),
        Err(VMError::PermissionDenied)
    ));

    let mut expired_handle = base_handle.clone();
    expired_handle.expiry_slot = policy_entry.policy.current_slot.saturating_sub(1);
    expired_handle.max_clock_skew_ms = Some(0);
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &expired_handle, &base_intent),
        Err(VMError::PermissionDenied)
    ));

    let mut low_handle_era = base_handle.clone();
    low_handle_era.handle_era = policy_entry.policy.min_handle_era - 1;
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &low_handle_era, &base_intent),
        Err(VMError::PermissionDenied)
    ));

    let mut low_sub_nonce = base_handle.clone();
    low_sub_nonce.sub_nonce = policy_entry.policy.min_sub_nonce - 1;
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &low_sub_nonce, &base_intent),
        Err(VMError::PermissionDenied)
    ));

    let empty_snapshot = AxtPolicySnapshot::default();
    assert!(matches!(
        exercise_fixture_handle(&descriptor, &empty_snapshot, &base_handle, &base_intent),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_caches_valid_ds_proof_per_slot() {
    let dsid = DataSpaceId::new(25);
    let descriptor = make_descriptor(dsid);
    let manifest_root = [0xAB; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(4), 1, 1, 10);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/ready".into()],
        write: vec!["ledger/ready".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch recorded");

    let proof = proof_blob_for(dsid, manifest_root, vec![0xAA], 50);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof blob"),
    );

    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("first proof accepted");
    // Second verification in the same slot should hit the cache.
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("cached proof accepted");

    let cache_status = host
        .axt_cached_proof_status(dsid)
        .expect("cache entry present");
    assert!(cache_status.0, "expected cached proof to be valid");
    assert_eq!(
        cache_status.1.expect("manifest cached"),
        manifest_root,
        "cached manifest root should match policy"
    );
    let recorded = host
        .axt_recorded_proof_payload(dsid)
        .expect("proof recorded");
    assert_eq!(recorded, proof.payload);
}

#[test]
fn core_host_caches_rejected_proof_per_slot() {
    let dsid = DataSpaceId::new(26);
    let descriptor = make_descriptor(dsid);
    let manifest_root = [0xCD; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(4), 1, 1, 8);
    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&TouchManifest {
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        })
        .expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch recorded");

    let wrong_root = [0xEF; 32];
    let proof = proof_blob_for(dsid, wrong_root, vec![0x11, 0x22], 20);
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof blob"),
    );

    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    // Cached rejection should short-circuit the retry.
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    assert!(host.axt_recorded_proof_payload(dsid).is_none());
    let cache_status = host
        .axt_cached_proof_status(dsid)
        .expect("cache entry present");
    assert!(!cache_status.0, "cached proof should be marked invalid");
    assert_eq!(
        cache_status.1.expect("cached manifest root"),
        wrong_root,
        "cached manifest root tracks the offending payload"
    );
}

#[test]
fn core_host_handles_multi_dataspace_axt_flow() {
    let dsid_a = DataSpaceId::new(31);
    let dsid_b = DataSpaceId::new(32);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid_a, dsid_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: dsid_a,
                read: vec!["orders/".into()],
                write: vec!["ledger/".into()],
            },
            axt::AxtTouchSpec {
                dsid: dsid_b,
                read: vec!["fx/".into()],
                write: vec!["settle/".into()],
            },
        ],
    };
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let policy_a = AxtPolicyBinding {
        dsid: dsid_a,
        policy: AxtPolicyEntry {
            manifest_root: [0xA1; 32],
            target_lane: LaneId::new(3),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 12,
        },
    };
    let policy_b = AxtPolicyBinding {
        dsid: dsid_b,
        policy: AxtPolicyEntry {
            manifest_root: [0xB2; 32],
            target_lane: LaneId::new(3),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 12,
        },
    };
    let entries = vec![policy_a, policy_b];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };

    let mut host = CoreHost::new().with_axt_policy_snapshot(&snapshot);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    for (dsid, manifest) in [
        (
            dsid_a,
            TouchManifest {
                read: vec!["orders/abc".into()],
                write: vec!["ledger/abc".into()],
            },
        ),
        (
            dsid_b,
            TouchManifest {
                read: vec!["fx/spot".into()],
                write: vec!["settle/spot".into()],
            },
        ),
    ] {
        let ds_ptr = store_tlv(
            &mut vm,
            PointerType::DataSpaceId,
            &norito::to_bytes(&dsid).expect("encode dsid"),
        );
        let manifest_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&manifest).expect("encode manifest"),
        );
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
            .expect("touch recorded");
    }

    for (dsid, root) in [(dsid_a, [0xA1; 32]), (dsid_b, [0xB2; 32])] {
        let proof = proof_blob_for(dsid, root, vec![0x33, 0x44], 40);
        let ds_ptr = store_tlv(
            &mut vm,
            PointerType::DataSpaceId,
            &norito::to_bytes(&dsid).expect("encode dsid"),
        );
        let proof_ptr = store_tlv(
            &mut vm,
            PointerType::ProofBlob,
            &norito::to_bytes(&proof).expect("encode proof blob"),
        );
        vm.set_register(10, ds_ptr);
        vm.set_register(11, proof_ptr);
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
            .expect("proof accepted");
    }

    for (idx, dsid, root, sub_nonce) in [
        (0usize, dsid_a, [0xA1; 32], 2),
        (1usize, dsid_b, [0xB2; 32], 3),
    ] {
        let handle = make_handle(binding, LaneId::new(3), root, 2, sub_nonce, 30 + idx as u64);
        let intent = fixture_intent(dsid);
        let handle_ptr = store_tlv(
            &mut vm,
            PointerType::AssetHandle,
            &norito::to_bytes(&handle).expect("encode handle"),
        );
        let intent_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&intent).expect("encode intent"),
        );
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect("handle accepted");
    }

    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );
}

#[test]
fn core_host_rejects_zero_manifest_root_policy() {
    let dsid = DataSpaceId::new(17);
    let descriptor = make_descriptor(dsid);
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = make_handle(binding, LaneId::new(4), [0; 32], 0, 0, 5);
    let intent = fixture_intent(dsid);
    let snapshot = make_policy_snapshot(dsid, [0; 32], LaneId::new(4), 0, 0, 1);

    assert!(matches!(
        exercise_fixture_handle(&descriptor, &snapshot, &handle, &intent),
        Err(VMError::PermissionDenied)
    ));
}
