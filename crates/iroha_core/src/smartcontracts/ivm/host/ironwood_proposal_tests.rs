// Retained artifacts must obey the selected proposal grammar at their publication boundary.
#[test]
fn ironwood_retained_host_proposal_artifacts_refuse_before_any_effect() {
    use iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1;
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let mut block = state.block(BlockHeader::new(
        (u64::try_from(state.view().height()).unwrap() + 1)
            .try_into()
            .unwrap(),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    for (path, bytes) in crate::runtime::ironwood::admission::tests::proposal_rows(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    ) {
        let ordinary: StatePath = "ordinary_before_ironwood".parse().unwrap();
        let metadata: Name = "ironwood_artifact_must_not_apply".parse().unwrap();
        let original = vec![7];
        let mut tx = block.transaction();
        tx.world
            .smart_contract_state
            .insert(path.clone(), original.clone());
        bind_artifact_test_signed_root(&mut tx, &authority);
        // Ordinary generic execution cannot produce durable writes. This deliberately forged
        // retained artifact exercises the independent publication owner, not syscall authority.
        let writes = BTreeMap::from([
            (ordinary.clone(), Some(vec![1])),
            (path.clone(), Some(bytes)),
        ]);
        let artifacts = HostExecutionArtifacts {
            x509_queue_backing: None,
            queued: vec![QueuedEffect {
                payload: QueuedEffectPayload::Instruction(
                    SetKeyValue::account(authority.clone(), metadata.clone(), Json::new(true))
                        .into(),
                ),
                authority: authority.clone(),
                contract_runtime_context: None,
                entrypoint_authorization: None,
            }],
            entrypoint_authorization: None,
            confidential_gas_delta: 7,
            completed_axt: Vec::new(),
            durable_state_authorizations: writes.keys().cloned().map(|key| (key, None)).collect(),
            durable_state_overlay: writes,
        };
        let error = artifacts
            .apply_to_transaction(&mut tx, &authority)
            .unwrap_err();
        assert!(
            matches!(error, ValidationFail::NotPermitted(message) if message.contains("inactive Ironwood"))
        );
        assert_eq!(tx.world.smart_contract_state.get(&path), Some(&original));
        assert!(tx.world.smart_contract_state.get(&ordinary).is_none());
        assert!(
            tx.world
                .account(&authority)
                .unwrap()
                .metadata()
                .get(&metadata)
                .is_none()
        );
        assert_eq!(tx.confidential_gas_used_in_tx, 0);
        tx.finish_execution_effect_budget().unwrap();
    }
}

#[test]
fn ironwood_host_proposal_publication_preserves_old_and_opaque_rows() {
    use iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1;
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let mut block = state.block(BlockHeader::new(
        (u64::try_from(state.view().height()).unwrap() + 1)
            .try_into()
            .unwrap(),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    for (path, bytes) in crate::runtime::ironwood::admission::tests::proposal_rows(
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1,
    ) {
        for replacement in [Some(bytes), Some(vec![0xff]), None] {
            let original = vec![7];
            let mut tx = block.transaction();
            tx.world
                .smart_contract_state
                .insert(path.clone(), original.clone());
            bind_artifact_test_signed_root(&mut tx, &authority);
            let overlay = BTreeMap::from([(path.clone(), replacement.clone())]);
            // The inactive-Ironwood proposal grammar admits predecessor and opaque rows and
            // deletions, so it never masks the native-state rule below with its own refusal.
            assert!(
                crate::runtime::ironwood::admission::durable_state_overlay(
                    &overlay,
                    tx.world.runtime_selection(),
                )
                .is_ok()
            );
            // Native multisig proposals change only through their own instructions; a
            // contract durable-state artifact can neither replace nor delete them.
            let artifacts = HostExecutionArtifacts {
                x509_queue_backing: None,
                queued: Vec::new(),
                entrypoint_authorization: None,
                confidential_gas_delta: 0,
                completed_axt: Vec::new(),
                durable_state_authorizations: BTreeMap::from([(path.clone(), None)]),
                durable_state_overlay: overlay,
            };
            let error = artifacts
                .apply_to_transaction(&mut tx, &authority)
                .unwrap_err();
            assert!(
                matches!(&error, ValidationFail::NotPermitted(message) if message.contains("native multisig state")),
                "{error:?}"
            );
            assert_eq!(tx.world.smart_contract_state.get(&path), Some(&original));
            assert_eq!(tx.confidential_gas_used_in_tx, 0);
            tx.finish_execution_effect_budget().unwrap();
        }
    }
}

#[test]
fn ironwood_opaque_host_decoders_preserve_predecessor_vm_errors() {
    use iroha_data_model::{
        isi::privacy::SchedulePrivacyIronwoodLimitsUpdateV1,
    };
    let authority = fixture_account("alice");
    let instruction: InstructionBox = SchedulePrivacyIronwoodLimitsUpdateV1::new(100, iroha_data_model::privacy::OrchardLimitsV1 { max_action_count: iroha_data_model::privacy::ORCHARD_MAX_ACTIONS_V1 })
    .into();
    for activated in [false, true] {
        let mut world = World::default();
        if activated {
            crate::runtime::ironwood::install_for_test(&mut world);
        }
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        host.set_local_contract_debug_execution();
        let mut vm = IVM::new(1_000_000);
        let bytes = norito::encode_canonical(&instruction).unwrap();
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &bytes);
        vm.set_register(10, ptr);
        vm.set_register(11, 0);
        let result = host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm);
        if activated {
            assert!(
                matches!(result, Err(ivm::VMError::PermissionDenied)),
                "{result:?}"
            );
        } else {
            assert!(
                matches!(result, Err(ivm::VMError::NoritoInvalid)),
                "{result:?}"
            );
        }
        assert!(host.queued.is_empty());
        let trigger = Trigger::new(
            "ironwood_opaque_trigger".parse().unwrap(),
            Action::new(
                vec![instruction.clone()],
                Repeats::Exactly(1),
                authority.clone(),
                DataEventFilter::Any,
            )
            .unwrap(),
        );
        let payload = Json::new(trigger);
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&payload));
        vm.set_register(10, ptr);
        let result = host.syscall(ivm_sys::SYSCALL_CREATE_TRIGGER, &mut vm);
        if activated {
            result.unwrap();
            assert_eq!(host.queued.len(), 1);
        } else {
            assert!(
                matches!(result, Err(ivm::VMError::DecodeError)),
                "{result:?}"
            );
            assert!(host.queued.is_empty());
        }
    }
}

#[test]
fn ironwood_host_without_committed_selection_refuses_instead_of_assuming_baseline() {
    let (_, instruction, _) = crate::runtime::catalog::tests::privacy_resolution_instructions();
    let authority = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(1_000_000);
    let bytes = norito::encode_canonical(&instruction).unwrap();
    let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &bytes);
    vm.set_register(10, ptr);
    assert!(matches!(
        host.decode_opaque_instruction(&vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    let trigger = Trigger::new(
        "ironwood_requires_selection".parse().unwrap(),
        Action::new(
            vec![instruction],
            Repeats::Exactly(1),
            authority,
            DataEventFilter::Any,
        )
        .unwrap(),
    );
    let payload = Json::new(trigger);
    let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&payload));
    vm.set_register(10, ptr);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_CREATE_TRIGGER, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    assert!(host.queued.is_empty());
}

#[test]
fn ironwood_custom_trigger_sccp_preflight_uses_compiled_grammar_without_state() {
    use iroha_executor_data_model::isi::multisig::{MultisigProposalNonce, MultisigPropose};
    let authority = fixture_account("alice");
    let (_, ironwood, _) = crate::runtime::catalog::tests::privacy_resolution_instructions();
    let record: InstructionBox =
        crate::smartcontracts::isi::sccp::test_support::SampleInstructions::record().into();
    let propose = |instructions, nonce| {
        InstructionBox::from(MultisigPropose::new(
            authority.clone(),
            MultisigProposalNonce::new([nonce; 32]).unwrap(),
            instructions,
            None,
        ))
    };
    let direct = propose(vec![ironwood, record], 0x71);
    assert!(
        direct
            .as_any()
            .is::<iroha_data_model::isi::CustomInstruction>()
    );
    for (index, proposal) in [direct.clone(), propose(vec![direct], 0x72)]
        .into_iter()
        .enumerate()
    {
        let executable = Executable::Instructions(vec![proposal].into());
        // Ordinary admission still leaves Custom JSON opaque; classification happens only at
        // this syscall's native parser boundary, including its existing bounded recursion.
        assert!(!crate::runtime::ironwood::admission::executable_requires_ironwood(&executable));
        let trigger = Trigger::new(
            format!("ironwood_sccp_preflight_{index}").parse().unwrap(),
            Action::new(
                executable,
                Repeats::Exactly(1),
                authority.clone(),
                DataEventFilter::Any,
            )
            .unwrap(),
        );
        let payload = Json::new(trigger);
        for selected in [None, Some(false), Some(true)] {
            let mut world = World::default();
            if selected == Some(true) {
                crate::runtime::ironwood::install_for_test(&mut world);
            }
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let view = state.view();
            let mut host = CoreHostImpl::new(authority.clone());
            if selected.is_some() {
                host.set_query_state(&view);
            }
            host.set_local_contract_debug_execution();
            let mut vm = IVM::new(1_000_000);
            let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&payload));
            vm.set_register(10, ptr);
            let result = host.syscall(ivm_sys::SYSCALL_CREATE_TRIGGER, &mut vm);
            if selected == Some(false) {
                result.expect(
                    "the exact predecessor parser still leaves this opaque proposal unclassified",
                );
                assert_eq!(host.queued.len(), 1);
            } else {
                assert!(
                    matches!(result, Err(ivm::VMError::PermissionDenied)),
                    "selection {selected:?}: {result:?}"
                );
                assert!(host.queued.is_empty());
            }
        }
    }
}
