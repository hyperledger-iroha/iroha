// Actual tag2 host custody, child rollback and strict canonical boundary controls.
// These malformed-proof tests establish queue ownership/refusal, never proof acceptance.
use crate::executor::attachment_custody::x509::tests::{
    planned as x509_planned_fixture, request as x509_request_fixture,
};

fn x509_host_source(
    state: &State,
    authority: &AccountId,
    root: &ContractAddress,
    target: &ContractAddress,
    path: Vec<u32>,
) -> (
    iroha_data_model::transaction::SignedTransaction,
    iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1,
) {
    use iroha_data_model::transaction::{
        Executable, FeePaymentIntent, TransactionBuilder, signed_attachments::*,
    };
    let request = x509_request_fixture(*state.network_id_ref(), target.subject_id());
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(x509_planned_fixture(
        &request,
        ActionInvocationBindingV1::Contract {
            root_index: 0,
            call_path: path,
        },
        ActionAuthorityBindingV1::ContractSubject(target.subject_id()),
    ));
    let source = TransactionBuilder::new(
        *state.network_id_ref(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::ContractCall(contract_invocation_from_json(
        state,
        root.clone(),
        "main",
        &Json::new(()),
    )))
    .with_attachment_plan(plan)
    .unwrap()
    .sign(fixture_signing_keypair(authority).private_key());
    (source, request)
}
fn set_x509_request(
    vm: &mut IVM,
    request: &iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1,
) {
    let bytes = norito::encode_canonical(&InstructionBox::from(request.clone())).unwrap();
    let pointer = store_tlv(vm, PointerType::NoritoBytes, &bytes);
    vm.set_register(10, pointer);
    vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_PRESENT_X509);
}

#[test]
fn x509_host_tag2_queues_original_owner_but_raw_drain_refuses() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CredentialQueue { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let (source, request) = x509_host_source(&state, &authority, &root, &root, vec![]);
    let mut custody = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let budget = view.execution_budget();
    let baseline = budget.reserved_bytes();
    {
        let mut vm = IVM::try_new_with_memory_budget(10_000_000, &budget).unwrap();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            custody
                .contract_host(0, attachment_root_call(&source, 0))
                .unwrap(),
        )
        .unwrap();
        let metadata_demand = host.x509_metadata_clone_bytes().unwrap();
        assert!(metadata_demand > 0);
        set_x509_request(&mut vm, &request);
        host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
            .unwrap();
        assert_eq!(
            vm.register(10),
            0,
            "only original action ordinal is returned"
        );
        assert_eq!(host.queued.len(), 1);
        let borrowed = host.queued[0].instruction().unwrap();
        let artifacts_view = host.queued.iter().filter_map(|queued| queued.instruction());
        assert!(std::ptr::eq(
            artifacts_view.into_iter().next().unwrap(),
            borrowed
        ));
        assert!(matches!(
            host.queued[0].payload,
            QueuedEffectPayload::AttachedX509(_)
        ));
        assert!(
            state.world.privacy_nullifiers.view().iter().next().is_none(),
            "queueing grants no nullifier effect"
        );
        assert!(
            host.drain_instructions().is_err(),
            "inspection may not strip custody from the proof-free request"
        );
    }
    assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn x509_host_artifacts_reject_substituted_transaction_before_effects() {
    use iroha_data_model::isi::error::{InstructionExecutionError, InvalidParameterError};

    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CredentialApply { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let (source, request) = x509_host_source(&state, &authority, &root, &root, vec![]);
    let mut custody = AttachmentCustody::new(&source).unwrap();
    let artifacts = {
        let view = state.view();
        let mut vm = IVM::try_new_with_memory_budget(10_000_000, &view.execution_budget()).unwrap();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            custody
                .contract_host(0, attachment_root_call(&source, 0))
                .unwrap(),
        )
        .unwrap();
        set_x509_request(&mut vm, &request);
        host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
            .unwrap();
        let context = host.current_contract_runtime_context.clone();
        let artifacts = host.into_execution_artifacts(context).unwrap();
        assert_eq!(artifacts.queued_instruction_authority_count(), 1);
        assert!(std::ptr::eq(
            artifacts.queued_instruction_views().next().unwrap().1,
            artifacts.queued[0].instruction().unwrap()
        ));
        artifacts
    };
    custody.finish().unwrap();
    let mut block = state.block(BlockHeader::new(
        nonzero!(100_u64),
        None,
        None,
        1_800_000_000_000,
        0,
    ));
    let mut transaction = block.transaction();
    let before = transaction.privacy_budget_for_testing();
    // Give the substituted transaction its own genuine signed-root budget so the
    // attachment's original-source check, not missing execution ownership, refuses it.
    let substituted_source = artifact_test_signed_root(*state.network_id_ref(), &authority);
    assert_ne!(substituted_source.hash(), source.hash());
    bind_artifact_test_source_root(&mut transaction, &substituted_source);
    let error = artifacts
        .apply_to_transaction(&mut transaction, &authority)
        .unwrap_err();
    assert!(
        matches!(
            &error,
            ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(reason)
            )) if reason == "X.509 attachment lost its original signed source or authority"
        ),
        "{error:?}"
    );
    assert_eq!(transaction.privacy_budget_for_testing(), before);
    assert!(transaction.world.privacy_nullifiers.iter().next().is_none());
}

#[test]
fn x509_host_rejects_blob_trailing_wrong_instruction_and_changed_public_inputs() {
    for case in 0..4 {
        let authority = fixture_account("alice");
        let state = contract_test_state(&authority);
        let root = install_contract(
            &state,
            &authority,
            "seiyaku CredentialBoundary { kotoage fn main() authorize(anyone) -> int { return 0; } }",
            0,
        );
        let (source, mut request) = x509_host_source(&state, &authority, &root, &root, vec![]);
        let mut custody = AttachmentCustody::new(&source).unwrap();
        let view = state.view();
        let mut vm = IVM::try_new_with_memory_budget(10_000_000, &view.execution_budget()).unwrap();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            custody
                .contract_host(0, attachment_root_call(&source, 0))
                .unwrap(),
        )
        .unwrap();
        if case == 3 {
            request.statement.context.action_index += 1;
        }
        let instruction = if case == 2 {
            InstructionBox::from(iroha_data_model::isi::Log::new(
                iroha_data_model::Level::INFO,
                "wrong wire".into(),
            ))
        } else {
            request.into()
        };
        let mut bytes = norito::encode_canonical(&instruction).unwrap();
        if case == 1 {
            bytes.push(0);
        }
        let pointer = store_tlv(
            &mut vm,
            if case == 0 {
                PointerType::Blob
            } else {
                PointerType::NoritoBytes
            },
            &bytes,
        );
        vm.set_register(10, pointer);
        vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_PRESENT_X509);
        assert!(
            host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
                .is_err()
        );
        assert!(host.queued.is_empty());
        drop(host);
        assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
    }
}

#[test]
fn x509_actual_compiled_child_queues_then_failure_restores_business_state_and_custody() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CredentialRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let child = install_contract(
        &state,
        &authority,
        r#"
        seiyaku CredentialChild {
            state StateMap<int, int> Values;
            kotoage fn main(bytes request) authorize(anyone) -> Result<int, int> {
                let PendingAction pending = ledger::credentials::present_x509(request: request);
                Values[1] = 9;
                return Result::err(7);
            }
        }
    "#,
        1,
    );
    let (source, request) = x509_host_source(&state, &authority, &root, &child, vec![0]);
    let mut custody = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut vm = IVM::try_new_with_memory_budget(10_000_000, &view.execution_budget()).unwrap();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        custody
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    let bytes = norito::encode_canonical(&InstructionBox::from(request)).unwrap();
    store_typed_contract_call(
        &mut vm,
        &state,
        &child,
        "main",
        &Json::new(norito::json!({"request": (format!("0x{}", hex::encode(bytes)))})),
    );
    let error = host
        .syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("failed child poisons consumed original attachment");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert_eq!(host.nested_contract_call_depth, 0);
    assert!(host.queued.is_empty());
    assert!(host.durable_state_overlay.is_empty());
    assert_eq!(
        host.current_contract_runtime_context
            .as_ref()
            .unwrap()
            .contract_subject,
        root.subject_id()
    );
    drop(host);
    assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn x509_receipt_capacity_refusal_precedes_call_hash_and_confidential_effects() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let budget = state.view().execution_budget();
    let mut host = CoreHost::new(authority.clone());
    // An ordinary instruction isolates receipt storage. This does not construct
    // a proof or claim X509 acceptance.
    host.queue_instruction(
        iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "receipt capacity".into())
            .into(),
    )
    .unwrap();
    host.reserve_x509_queue_backing(&budget).unwrap();
    let mut artifacts = host.into_execution_artifacts(None).unwrap();
    artifacts.confidential_gas_delta = 7;
    let mut block = state.block(BlockHeader::new(
        nonzero!(100_u64),
        None,
        None,
        1_800_000_000_000,
        0,
    ));
    let mut transaction = block.transaction();
    let before_hash = transaction.tx_call_hash;
    let before_gas = transaction.confidential_gas_used_in_tx;
    let before_privacy = transaction.privacy_budget_for_testing();
    budget.set_limit_bytes(budget.reserved_bytes());
    let error = artifacts
        .apply_to_transaction(&mut transaction, &authority)
        .unwrap_err();
    assert!(transaction.is_execution_deferred(), "{error}");
    assert_eq!(transaction.tx_call_hash, before_hash);
    assert_eq!(transaction.confidential_gas_used_in_tx, before_gas);
    assert_eq!(transaction.privacy_budget_for_testing(), before_privacy);
    // Restore capacity only after observing refusal; the original attempt stays deferred.
    budget.set_limit_bytes(1024 * 1024 * 1024);
}

#[test]
fn x509_source_limit_uses_complete_transaction_limit_independent_of_attachment_expansion() {
    use iroha_data_model::parameter::TransactionParameter;
    let authority = fixture_account("alice");
    let world = World::with(
        [Domain::new(fixture_domain_id()).build(&authority)],
        [build_fixture_account(&authority, &authority)],
        [],
    );
    // Limits belong to the original signed genesis; preloaded World parameters
    // are replaced when that genesis executes. The non-default source cap still
    // admits a maximal privacy action and its required signature context.
    let mut config = crate::sumeragi::test_chain::TestChainConfig::new(world, 0);
    config.genesis_parameters.extend([
        Parameter::Transaction(TransactionParameter::MaxTxBytes(nonzero!(10_000_000_u64))),
        Parameter::Transaction(TransactionParameter::MaxDecompressedBytes(nonzero!(
            512_u64
        ))),
    ]);
    let state = signed_contract_host_state(config);
    let view = state.view();
    let reference = QueryStateRef::View(&view);
    assert_eq!(reference.transaction_source_byte_limit(), 10_000_000);
    assert_eq!(
        view.world()
            .parameters()
            .transaction()
            .max_decompressed_bytes()
            .get(),
        512
    );
}
