use iroha_data_model::nexus::{AxtFastpqBinding, DataSpaceId, LaneId};
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    axt::{
        self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, ProofBlob, RemoteSpendIntent,
        SpendOp, TouchManifest,
    },
    host::DefaultHost,
};

const AXT_VERIFY_EMPTY_GAS: u64 = 64;
const AXT_GAS_BASE: u64 = 16;

fn axt_gas(payload_len: usize) -> u64 {
    AXT_GAS_BASE.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
}

#[test]
fn default_host_unknown_syscall_returns_unknown() {
    let mut vm = IVM::new(1000);
    let mut host = DefaultHost::new();
    match host.syscall(0xDF, &mut vm) {
        Err(VMError::UnknownSyscall(n)) => assert_eq!(n, 0xDF),
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

fn test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
    let mut payload = Vec::new();
    payload.extend_from_slice(domain);
    for part in parts {
        payload.extend_from_slice(part);
    }
    iroha_crypto::Hash::new(payload)
}

fn proof_blob_for(dsid: DataSpaceId, manifest_root: [u8; 32], proof_seed: &[u8]) -> ProofBlob {
    let source_tx_commitment = test_digest(b"ivm-host-test:source-tx", &[proof_seed]);
    let claim_digest = test_digest(b"ivm-host-test:claim", &[proof_seed]);
    let witness_commitment = test_digest(b"ivm-host-test:witness", &[proof_seed]);
    let policy_commitment = test_digest(b"ivm-host-test:policy", &[&manifest_root]);
    let proof_digest = test_digest(b"ivm-host-test:proof", &[proof_seed, &manifest_root]);
    let envelope = axt::AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: proof_digest.as_ref().to_vec(),
        fastpq_binding: Some(AxtFastpqBinding {
            parameter: "fastpq-lane-balanced".to_string(),
            source_dsid: dsid.as_u64(),
            source_dataspace: "ivm-host-test-dataspace".to_string(),
            source_receipt_id: format!("receipt-{}", hex::encode(source_tx_commitment.as_ref())),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_string(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: "test_effect".to_string(),
            corridor: "ivm-host-test-corridor".to_string(),
            verifier_id: "fastpq".to_string(),
            verifier_version: "v1".to_string(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
        }),
        committed_amount: None,
        amount_commitment: None,
    };
    ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
        expiry_slot: None,
    }
}

#[test]
fn default_host_axt_syscalls_dispatch_and_fail_closed_without_verifier() {
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
    let desc_bytes = norito::to_bytes(&descriptor).expect("encode descriptor");
    let desc_ptr = store_tlv(&mut vm, PointerType::AxtDescriptor, &desc_bytes);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(axt_gas(desc_bytes.len()))
    );

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
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(axt_gas(ds_bytes.len().saturating_add(manifest_bytes.len())))
    );

    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(AXT_VERIFY_EMPTY_GAS)
    );

    let proof = proof_blob_for(dsid, [1; 32], b"default-host-sequence");
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

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
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
        manifest_view_root: vec![1; 32],
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
            from: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            to: "sorauﾛ1Q2ｸBKzrｼStﾊYyXﾌ1ｹHｿｾkSveﾉyｻﾈHﾗｿug7zWﾑヰyRMH888".into(),
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
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));

    // The inline proof failed closed, so commit must not turn the preflighted
    // proof shape into AXT acceptance.
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Err(VMError::PermissionDenied)
    ));

    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
