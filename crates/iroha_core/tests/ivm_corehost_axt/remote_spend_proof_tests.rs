const FIXTURE_MERCHANT_ACCOUNT_LITERAL: &str =
    "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
const FIXTURE_VENDOR_ACCOUNT_LITERAL: &str =
    "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";

#[test]
fn core_host_enforces_exact_remote_spend_claim_consumption() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(107);
    let manifest_root = [0x67; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle_a, intent_a, amount_a) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let (handle_b, intent_b, amount_b) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        2,
        Some(dsid),
        FIXTURE_VENDOR_ACCOUNT_LITERAL,
    );
    let proof = proof_blob_for_remote_spends(
        dsid,
        manifest_root,
        b"host-unconsumed-proof-claim".to_vec(),
        25,
        &[
            (&handle_a, &intent_a, &amount_a),
            (&handle_b, &intent_b, &amount_b),
        ],
    );
    let mut host = host_with_policy(authority, dsid, manifest_root, lane, 5);
    assert_eq!(
        commit_single_handle_envelope(&mut host, &descriptor, &proof, &handle_a, &intent_a),
        Err(VMError::PermissionDenied),
        "a proof-bound claim omitted by the envelope must fail closed"
    );
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert!(reject.detail.contains("not consumed exactly once"));
}

#[test]
fn core_host_enforces_shared_budget_across_completed_envelopes() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(123);
    let manifest_root = [0x7B; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle_a, mut intent_a, _) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let (handle_b, mut intent_b, _) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        2,
        Some(dsid),
        FIXTURE_VENDOR_ACCOUNT_LITERAL,
    );
    let attack_amount = Quantity::from(7_u64);
    intent_a.op.amount = Some(attack_amount.clone());
    intent_b.op.amount = Some(attack_amount.clone());
    let proof_a = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-split-envelope-budget-a".to_vec(),
        25,
        &handle_a,
        &intent_a,
        &attack_amount,
    );
    let proof_b = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-split-envelope-budget-b".to_vec(),
        25,
        &handle_b,
        &intent_b,
        &attack_amount,
    );
    let mut attack_host = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
    commit_single_handle_envelope(
        &mut attack_host,
        &descriptor,
        &proof_a,
        &handle_a,
        &intent_a,
    )
    .expect("the first envelope is within the shared signed budget");
    assert_eq!(
        commit_single_handle_envelope(
            &mut attack_host,
            &descriptor,
            &proof_b,
            &handle_b,
            &intent_b,
        ),
        Err(VMError::PermissionDenied),
        "a second envelope must not reset the shared handle budget"
    );
    let reject = attack_host
        .take_axt_reject_for_tests()
        .expect("split-envelope budget reject context");
    assert_eq!(reject.reason, AxtRejectReason::Budget);
    assert!(
        reject
            .detail
            .contains("shared handle budget exceeded across completed AXT envelopes")
    );

    let control_amount = Quantity::from(5_u64);
    intent_a.op.amount = Some(control_amount.clone());
    intent_b.op.amount = Some(control_amount.clone());
    let control_proof_a = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-split-envelope-control-a".to_vec(),
        25,
        &handle_a,
        &intent_a,
        &control_amount,
    );
    let control_proof_b = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-split-envelope-control-b".to_vec(),
        25,
        &handle_b,
        &intent_b,
        &control_amount,
    );
    let mut control_host = host_with_policy(authority, dsid, manifest_root, lane, 5);
    commit_single_handle_envelope(
        &mut control_host,
        &descriptor,
        &control_proof_a,
        &handle_a,
        &intent_a,
    )
    .expect("first control envelope is within the shared signed budget");
    commit_single_handle_envelope(
        &mut control_host,
        &descriptor,
        &control_proof_b,
        &handle_b,
        &intent_b,
    )
    .expect("two envelopes may consume exactly the shared signed budget");
}

#[test]
fn core_host_rejects_duplicate_use_of_one_proof_claim() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(108);
    let manifest_root = [0x68; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-duplicate-proof-claim".to_vec(),
        25,
        &handle,
        &intent,
        &amount,
    );
    let mut host = host_with_policy(authority, dsid, manifest_root, lane, 5);
    let mut vm = IVM::new(1_000_000);
    let descriptor_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, descriptor_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let dsid_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let touch_ptr = store_tlv_norito(
        &mut vm,
        PointerType::NoritoBytes,
        &TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        },
    );
    vm.set_register(10, dsid_ptr);
    vm.set_register(11, touch_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied),
        "the same proof-bound handle cannot be recorded twice"
    );
}

#[test]
fn core_host_enforces_registered_asset_balance_policy() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(109);
    let manifest_root = [0x69; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-asset-policy".to_vec(),
        25,
        &handle,
        &intent,
        &amount,
    );

    let mut restricted = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
    commit_single_handle_envelope(&mut restricted, &descriptor, &proof, &handle, &intent)
        .expect("restricted asset may use the exact signed intent dataspace");

    let mut global = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
    global.set_axt_asset_policy_for_tests(
        axt_test_asset_definition_id(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
    );
    assert_eq!(
        use_single_handle_envelope(&mut global, &descriptor, &proof, &handle, &intent),
        Err(VMError::PermissionDenied),
        "USE must reject a global asset presented as a private-dataspace balance"
    );
    assert_eq!(
        global
            .take_axt_reject_for_tests()
            .expect("global policy reject context")
            .reason,
        AxtRejectReason::PolicyDenied
    );

    let snapshot = make_policy_snapshot(dsid, manifest_root, lane, 1, 1, 5);
    let mut missing = CoreHost::new(authority)
        .with_axt_policy_snapshot(&snapshot)
        .expect("canonical policy snapshot");
    configure_axt_test_host_without_asset(&mut missing, [(dsid, manifest_root)]);
    assert_eq!(
        use_single_handle_envelope(&mut missing, &descriptor, &proof, &handle, &intent),
        Err(VMError::DecodeError),
        "USE must reject an unregistered asset definition"
    );
    assert_eq!(
        missing
            .take_axt_reject_for_tests()
            .expect("missing-definition reject context")
            .reason,
        AxtRejectReason::PolicyDenied
    );
}

#[test]
fn core_host_rejects_correctly_signed_handle_for_another_asset_at_use() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(111);
    let manifest_root = [0x6B; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (mut handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-signed-other-asset".to_vec(),
        25,
        &handle,
        &intent,
        &amount,
    );
    handle.asset_definition_id =
        AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 2])
            .expect("valid alternate AXT fixture asset id");
    let handle = signed_abi_handle(handle, dsid);
    let mut host = host_with_policy(authority, dsid, manifest_root, lane, 5);

    assert_eq!(
        use_single_handle_envelope(&mut host, &descriptor, &proof, &handle, &intent),
        Err(VMError::PermissionDenied),
        "USE must reject a correctly signed handle for another asset in the same dataspace"
    );
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::PolicyDenied);
    assert!(
        reject
            .detail
            .contains("handle asset does not match remote spend intent asset")
    );
}

fn signed_abi_handle_with_incarnation(
    handle: AssetHandle,
    dataspace: DataSpaceId,
    asset_definition_incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1,
) -> AssetHandle {
    let binding = handle
        .binding_array()
        .expect("fixture AXT binding must be 32 bytes");
    let manifest_root: [u8; 32] = handle
        .manifest_view_root
        .as_slice()
        .try_into()
        .expect("fixture manifest root must be 32 bytes");
    let draft = iroha_data_model::nexus::AssetHandleDraft {
        asset_definition_id: handle.asset_definition_id,
        scope: handle.scope,
        subject: iroha_data_model::nexus::HandleSubject {
            account: handle.subject.account,
            origin_dsid: handle.subject.origin_dsid,
        },
        budget: iroha_data_model::nexus::HandleBudget {
            remaining: handle.budget.remaining,
            per_use: handle.budget.per_use,
        },
        handle_era: handle.handle_era,
        sub_nonce: handle.sub_nonce,
        group_binding: iroha_data_model::nexus::GroupBinding {
            composability_group_id: handle.group_binding.composability_group_id,
            epoch_id: handle.group_binding.epoch_id,
        },
        target_lane: handle.target_lane,
        axt_binding: AxtBinding::new(binding),
        manifest_view_root: manifest_root,
        expiry_slot: handle.expiry_slot,
        max_clock_skew_ms: handle.max_clock_skew_ms,
    };
    let mut context = axt_test_issuer_context(dataspace, draft.manifest_view_root);
    context.asset_definition_incarnation = asset_definition_incarnation;
    let model = draft
        .sign_by_issuer_v1(context, axt_test_issuer().private_key())
        .expect("sign AXT issuer fixture with explicit asset incarnation");
    abi_asset_handle_from_signed_model(model)
}

#[test]
fn core_host_rejects_correctly_signed_stale_asset_incarnation_at_use() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(112);
    let manifest_root = [0x6C; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let control_proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-current-asset-incarnation".to_vec(),
        25,
        &handle,
        &intent,
        &amount,
    );
    let mut control = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
    use_single_handle_envelope(&mut control, &descriptor, &control_proof, &handle, &intent)
        .expect("a handle signed for the exact live asset incarnation must pass USE");

    let stale_incarnation = iroha_data_model::nexus::AxtAssetIncarnationV1::derive(
        &axt_test_network_id(),
        &axt_test_asset_definition_id(),
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"iroha-corehost-axt-stale-registration-header",
        )),
        &Hash::new(b"iroha-corehost-axt-stale-registration-execution"),
        9,
    );
    assert_ne!(stale_incarnation, axt_test_asset_incarnation());
    let stale_handle = signed_abi_handle_with_incarnation(handle, dsid, stale_incarnation);
    let stale_proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-stale-asset-incarnation".to_vec(),
        25,
        &stale_handle,
        &intent,
        &amount,
    );
    let mut attack = host_with_policy(authority, dsid, manifest_root, lane, 5);
    assert_eq!(
        use_single_handle_envelope(
            &mut attack,
            &descriptor,
            &stale_proof,
            &stale_handle,
            &intent,
        ),
        Err(VMError::PermissionDenied),
        "USE must reject a correctly signed handle from an earlier asset incarnation"
    );
    let reject = attack.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::PolicyDenied);
    assert!(
        reject.detail.contains("stale asset-definition incarnation"),
        "unexpected stale-incarnation rejection: {}",
        reject.detail
    );
}

#[test]
fn core_host_rejects_historical_incarnation_proof_for_current_handle() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(113);
    let manifest_root = [0x6D; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (current_handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(dsid),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let current_proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-current-incarnation-proof".to_vec(),
        25,
        &current_handle,
        &intent,
        &amount,
    );
    let mut control = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
    use_single_handle_envelope(
        &mut control,
        &descriptor,
        &current_proof,
        &current_handle,
        &intent,
    )
    .expect("proof and handle from the exact current asset incarnation must pass USE");

    let historical_incarnation = iroha_data_model::nexus::AxtAssetIncarnationV1::derive(
        &axt_test_network_id(),
        &axt_test_asset_definition_id(),
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"iroha-corehost-axt-historical-proof-registration",
        )),
        &Hash::new(b"iroha-corehost-axt-historical-proof-execution"),
        11,
    );
    assert_ne!(historical_incarnation, axt_test_asset_incarnation());
    let historical_handle =
        signed_abi_handle_with_incarnation(current_handle.clone(), dsid, historical_incarnation);
    let historical_proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-historical-incarnation-proof".to_vec(),
        25,
        &historical_handle,
        &intent,
        &amount,
    );
    let mut attack = host_with_policy(authority, dsid, manifest_root, lane, 5);
    assert_eq!(
        use_single_handle_envelope(
            &mut attack,
            &descriptor,
            &historical_proof,
            &current_handle,
            &intent,
        ),
        Err(VMError::PermissionDenied),
        "a historical proof must not authorize a freshly signed current-incarnation handle"
    );
    let reject = attack.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert_eq!(
        reject.detail,
        "FASTPQ proof does not commit to the exact remote spend intent"
    );
}

#[test]
fn core_host_rejects_signed_origin_outside_bound_descriptor() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(110);
    let manifest_root = [0x6A; 32];
    let lane = LaneId::new(0);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    let (handle, intent, amount) = single_remote_spend(
        &authority,
        binding,
        dsid,
        manifest_root,
        lane,
        1,
        Some(DataSpaceId::new(9_999)),
        FIXTURE_MERCHANT_ACCOUNT_LITERAL,
    );
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        b"host-signed-undeclared-origin".to_vec(),
        25,
        &handle,
        &intent,
        &amount,
    );
    let mut host = host_with_policy(authority, dsid, manifest_root, lane, 5);
    assert_eq!(
        use_single_handle_envelope(&mut host, &descriptor, &proof, &handle, &intent),
        Err(VMError::PermissionDenied),
        "USE must reject an authenticated origin outside the active descriptor"
    );
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Descriptor);
    assert!(reject.detail.contains("origin dataspace is not declared"));
}

#[test]
fn core_host_resolves_hidden_amount_from_verified_dataspace_proof() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(73);
    let manifest_root = [0x73; 32];
    let lane = LaneId::new(0);
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
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
    let touch = TouchManifest {
        read: vec!["orders/hidden".into()],
        write: vec!["ledger/hidden".into()],
    };
    let touch_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, touch_ptr);
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
            target_lane: lane,
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 20,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: None,
        },
    };
    let effective_amount = Quantity::from(5_u64);
    let proof = proof_blob_for_remote_spends_with_committed_amount(
        dsid,
        manifest_root,
        vec![0x73],
        25,
        &[(&handle, &intent, &effective_amount)],
        Some(5),
    );
    let short_proof = proof_blob_for_remote_spends_with_committed_amount(
        dsid,
        manifest_root,
        vec![0x73],
        handle.expiry_slot - 1,
        &[(&handle, &intent, &effective_amount)],
        Some(5),
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &short_proof);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied),
        "a verified fallback proof must cover the authenticated handle lifetime"
    );
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Expiry);

    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
}
#[test]
fn core_host_rejects_standalone_replacement_of_handle_bound_proof() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(74);
    let manifest_root = [0x74; 32];
    let lane = LaneId::new(0);
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, lane, 5);
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
    let touch = TouchManifest {
        read: vec!["orders/replacement".into()],
        write: vec!["ledger/replacement".into()],
    };
    let touch_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, touch_ptr);
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
            target_lane: lane,
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 20,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
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
    let effective_amount = Quantity::from(5_u64);
    let initial_proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0x74],
        25,
        &handle,
        &intent,
        &effective_amount,
    );
    let initial_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &initial_proof);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, initial_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    let cache_before = host.axt_proof_cache_snapshot();
    let short_replacement = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0x74],
        handle.expiry_slot - 1,
        &handle,
        &intent,
        &effective_amount,
    );
    let replacement_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &short_replacement);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, replacement_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied),
        "caller-carried FASTPQ must not replace an issuer-authenticated proof"
    );
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert!(
        reject
            .detail
            .contains("authoritative finalized source-state anchor")
    );
    assert_eq!(
        host.axt_proof_cache_snapshot(),
        cache_before,
        "the rejected standalone replacement must not change the authenticated cache entry"
    );
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
}
#[test]
fn core_host_rejects_unanchored_proof_before_using_envelope_dataspace() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(17);
    let other_dsid = DataSpaceId::new(18);
    let manifest_root = [0x31; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority, dsid, manifest_root, LaneId::new(0), 5);
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
    let wrong_proof = proof_blob_for(other_dsid, manifest_root, b"other-dsid".to_vec(), 25);
    let wrong_proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &wrong_proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, wrong_proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let reject = host
        .take_axt_reject_for_tests()
        .expect("proof rejection context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert!(
        reject
            .detail
            .contains("authoritative finalized source-state anchor")
    );
}
#[test]
fn core_host_rejects_unanchored_proof_before_using_fastpq_binding() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(19);
    let manifest_root = [0x32; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority, dsid, manifest_root, LaneId::new(0), 5);
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
    let mut proof = proof_blob_for(dsid, manifest_root, b"source-dsid-mismatch".to_vec(), 25);
    let mut envelope: axt::AxtProofEnvelope =
        norito::decode_from_bytes(&proof.payload).expect("decode proof envelope");
    envelope
        .fastpq_binding
        .as_mut()
        .expect("proof helper should bind FastPQ metadata")
        .source_dsid = dsid.as_u64() + 1;
    proof.payload = norito::to_bytes(&envelope).expect("re-encode mutated proof envelope");
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let reject = host
        .take_axt_reject_for_tests()
        .expect("proof rejection context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert!(
        reject
            .detail
            .contains("authoritative finalized source-state anchor")
    );
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_rejects_standalone_proof_without_finalized_anchor() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(77);
    let manifest_root = [0xAB; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(0),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 5,
        },
    }];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
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
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/proof".into()],
        write: vec!["ledger/proof".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let ok_proof = proof_blob_for(dsid, manifest_root, vec![0x03, 0x04], 10);
    let ok_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &ok_proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, ok_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    );
    assert!(host.axt_proof_cache_snapshot().is_empty());
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::Proof);
    assert!(
        reject
            .detail
            .contains("authoritative finalized source-state anchor")
    );
}
