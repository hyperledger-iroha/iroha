mod axt_unanchored_admission_tests {
    //! Production admission must not confuse a reusable capability with an anchored spend.

    use super::*;

    fn signed_handle_fixture() -> (CoreHost, IVM, axt::HandleUsage) {
        let dsid = DataSpaceId::new(113);
        let manifest_root = [0x79; 32];
        let incarnation = fixture_axt_asset_incarnation(0x53);
        let asset_definition_id = fixture_axt_asset_definition_id();
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let issuer = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let issuer_id = UniversalAccountId::from_hash(Hash::new(b"anchored-spend-issuer"));
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"anchored-spend-genesis"),
        ));
        let mut host = CoreHost::new(ALICE_ID.clone())
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot")
            .with_axt_issuer_key_for_tests(
                dsid,
                manifest_root,
                issuer_id,
                issuer.public_key().clone(),
            );
        host.set_network_id(network_id);
        host.set_axt_asset_policy_for_tests(
            asset_definition_id.clone(),
            AssetBalancePolicy::DataspaceRestricted,
        );
        host.set_axt_asset_incarnation_for_tests(asset_definition_id.clone(), incarnation);
        let mut vm = IVM::new(1_000_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        Arc::make_mut(host.axt_state.as_mut().expect("active AXT envelope"))
            .record_touch(
                dsid,
                TouchManifest {
                    read: Vec::new(),
                    write: Vec::new(),
                },
            )
            .expect("record empty touch");
        let context = AxtHandleIssuerContextV1 {
            network_id,
            asset_dsid: dsid,
            asset_definition_incarnation: incarnation,
            issuer: issuer_id,
            issuer_manifest_root: manifest_root,
            code_root: vm.code_hash(),
            abi_version: u16::from(vm.abi_version()),
            abi_hash: ivm::syscalls::compute_abi_hash(vm.syscall_policy()),
        };
        let mut handle = AssetHandle {
            asset_definition_id: asset_definition_id.clone(),
            scope: vec!["transfer".to_owned()],
            subject: HandleSubject {
                account: ALICE_ID.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
            issuer_context: context,
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        };
        let signed = iroha_data_model::nexus::AssetHandle::try_from(&handle)
            .expect("canonical model handle")
            .draft()
            .sign_by_issuer_v1(context, issuer.private_key())
            .expect("sign reusable handle");
        signed
            .verify_issuer_signature_v1(context, issuer.public_key())
            .expect("the reusable capability is authentically issuer signed");
        handle.issuer_signature = signed.issuer_signature;
        let usage = axt::HandleUsage {
            handle,
            intent: RemoteSpendIntent {
                asset_dsid: dsid,
                op: SpendOp {
                    asset_definition_id,
                    kind: "transfer".to_owned(),
                    from: ALICE_ID.to_string(),
                    to: BOB_ID.to_string(),
                    amount: Some(Quantity::from(5_u64)),
                },
            },
            proof: None,
            amount: Quantity::from(5_u64),
            amount_commitment: None,
        };
        (host, vm, usage)
    }

    #[test]
    fn valid_issuer_handle_cannot_authorize_an_unanchored_spend_or_mutate_state() {
        let (mut host, mut vm, usage) = signed_handle_fixture();
        let active_before = Arc::clone(host.axt_state.as_ref().expect("active envelope"));
        let cache_before = Arc::clone(&host.axt_proof_cache);
        let replay_before = Arc::clone(&host.axt_replay_ledger);
        let budgets_before = host.axt_handle_budget_ledger.clone();
        let output_count_before = host.instruction_queue_count;
        let handle_ptr = store_tlv(
            &mut vm,
            PointerType::AssetHandle,
            &norito_blob(&usage.handle),
        );
        let intent_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&usage.intent),
        );
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);

        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm),
            Err(VMError::PermissionDenied)
        );
        let rejection = host.take_axt_reject_for_tests().expect("rejection context");
        assert_eq!(rejection.reason, AxtRejectReason::Proof);
        assert_eq!(rejection.dataspace, Some(usage.intent.asset_dsid));
        assert_eq!(
            rejection.detail,
            crate::fastpq::AXT_UNANCHORED_REMOTE_SPEND_REJECTION
        );
        assert!(Arc::ptr_eq(
            host.axt_state.as_ref().expect("active envelope retained"),
            &active_before,
        ));
        assert_eq!(host.axt_proof_cache.as_ref(), cache_before.as_ref());
        assert_eq!(host.axt_replay_ledger.as_ref(), replay_before.as_ref());
        assert_eq!(host.axt_handle_budget_ledger, budgets_before);
        assert_eq!(host.instruction_queue_count, output_count_before);
        assert!(host.completed_axt.is_empty());
    }

    #[test]
    fn unanchored_handle_gate_preserves_invalid_issuer_signature_rejection() {
        let (mut host, vm, mut usage) = signed_handle_fixture();
        usage.handle.issuer_signature = iroha_crypto::Signature::from_bytes(&[1_u8; 64]);
        assert_eq!(
            host.authenticate_axt_handle_usage(&vm, &usage),
            Err(VMError::PermissionDenied)
        );
        let rejection = host.take_axt_reject_for_tests().expect("rejection context");
        assert_eq!(rejection.reason, AxtRejectReason::PolicyDenied);
        assert_eq!(rejection.detail, "AXT handle issuer signature is invalid");
    }

    #[test]
    fn commit_rejects_preexisting_unanchored_handles_without_staging_effects() {
        let (mut host, mut vm, usage) = signed_handle_fixture();
        Arc::make_mut(host.axt_state.as_mut().expect("active envelope"))
            .record_handle(usage)
            .expect("construct preexisting unanchored state");
        let active_before = Arc::clone(host.axt_state.as_ref().expect("active envelope"));
        let budgets_before = host.axt_handle_budget_ledger.clone();
        let replay_before = Arc::clone(&host.axt_replay_ledger);
        let cache_before = Arc::clone(&host.axt_proof_cache);

        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_COMMIT, &mut vm),
            Err(VMError::PermissionDenied)
        );
        let rejection = host.take_axt_reject_for_tests().expect("rejection context");
        assert_eq!(rejection.reason, AxtRejectReason::Proof);
        assert_eq!(
            rejection.detail,
            crate::fastpq::AXT_UNANCHORED_REMOTE_SPEND_REJECTION
        );
        assert!(Arc::ptr_eq(
            host.axt_state.as_ref().expect("active envelope retained"),
            &active_before,
        ));
        assert_eq!(host.axt_handle_budget_ledger, budgets_before);
        assert_eq!(host.axt_replay_ledger.as_ref(), replay_before.as_ref());
        assert_eq!(host.axt_proof_cache.as_ref(), cache_before.as_ref());
        assert!(host.completed_axt.is_empty());
    }
}
