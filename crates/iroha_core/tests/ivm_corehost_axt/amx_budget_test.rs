#[test]
fn axt_commit_enforces_amx_budget() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(11);
    let manifest_root = [0x31; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, LaneId::new(0), 5);
    host.set_amx_limits(AmxLimits {
        per_dataspace_budget_ms: 0,
        group_budget_ms: 0,
        per_instruction_ns: 1,
        per_memory_access_ns: 1,
        per_syscall_ns: 1,
    });
    host.set_amx_analysis(ProgramAnalysis {
        metadata: ivm::ProgramMetadata::default(),
        instruction_count: 32,
        registers: RegisterUsage::default(),
        memory: MemoryAccesses::default(),
        syscalls: Vec::new(),
    });
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 20,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let amount = Quantity::from(5_u64);
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0xAB],
        20,
        &handle,
        &intent,
        &amount,
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    match host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm) {
        Err(VMError::AmxBudgetExceeded { stage, .. }) => {
            assert_eq!(stage, iroha_data_model::errors::AmxStage::Commit);
        }
        other => panic!("expected AMX budget error, got {other:?}"),
    }
}
