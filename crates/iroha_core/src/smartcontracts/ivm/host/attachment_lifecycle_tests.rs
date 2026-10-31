// Actual host/call-frame lifecycle controls; native proof consumption is still I.3.
use crate::executor::attachment_custody::{ActionRequest, AttachmentCustody, CustodyError};

fn attachment_root_call(
    source: &iroha_data_model::transaction::SignedTransaction,
    index: u32,
) -> &iroha_data_model::transaction::executable::ContractInvocation {
    use iroha_data_model::transaction::{Executable, ExecutableBatchItem};
    match source.instructions() {
        Executable::ContractCall(call) if index == 0 => call,
        Executable::Batch(items) => match &items[index as usize] {
            ExecutableBatchItem::ContractCall(call) => call,
            _ => panic!("fixture slot must be a deployed call"),
        },
        _ => panic!("fixture must retain a signed deployed call"),
    }
}

fn attachment_host_source(
    state: &State,
    authority: &AccountId,
    root: &ContractAddress,
    root_index: u32,
    declarations: &[(&ContractAddress, &[u32])],
) -> iroha_data_model::transaction::SignedTransaction {
    use iroha_data_model::{
        Level,
        transaction::signed_attachments::{
            ActionAuthorityBindingV1, ActionInvocationBindingV1, ActionOperationV1,
            ActionPublicInputsDigestV1, ActionRoleV1, AttachmentKindV1, AttachmentRoleV1,
            PlannedActionV1, PlannedAttachmentV1,
        },
        transaction::{
            Executable, ExecutableBatchItem, FeePaymentIntent, SignedAttachmentPlanV1,
            TransactionBuilder,
        },
    };
    let call = contract_invocation_from_json(state, root.clone(), "main", &Json::new(()));
    let instruction = InstructionBox::from(iroha_data_model::isi::Log::new(
        Level::INFO,
        "attachment lifecycle".into(),
    ));
    let executable = if root_index == 0 {
        Executable::ContractCall(call)
    } else {
        let mut items = vec![ExecutableBatchItem::ContractCall(call.clone())];
        for _ in 1..root_index {
            items.push(ExecutableBatchItem::Instruction(instruction.clone()));
        }
        items.push(ExecutableBatchItem::ContractCall(call));
        Executable::Batch(items.into())
    };
    let mut builder = TransactionBuilder::new(
        *state.network_id_ref(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(executable);
    if !declarations.is_empty() {
        let mut plan = SignedAttachmentPlanV1::new();
        for (index, (subject, path)) in declarations.iter().enumerate() {
            plan.push_action(PlannedActionV1 {
                operation: ActionOperationV1::NativeInstruction(
                    iroha_data_model::isi::instruction_wire_id(&instruction)
                        .unwrap()
                        .to_owned(),
                ),
                role: ActionRoleV1::ProofEffect,
                authority: ActionAuthorityBindingV1::ContractSubject(subject.subject_id()),
                invocation: ActionInvocationBindingV1::Contract {
                    root_index,
                    call_path: path.to_vec(),
                },
                public_inputs_digest: ActionPublicInputsDigestV1::new([0x5a; 32]),
                attachments: vec![PlannedAttachmentV1 {
                    kind: AttachmentKindV1::Proof,
                    role: AttachmentRoleV1::ActionProof,
                    bytes: vec![0x80 + index as u8; 48],
                }],
            });
        }
        builder = builder
            .with_attachment_plan(plan)
            .expect("canonical declarations");
    }
    builder.sign(fixture_signing_keypair(authority).private_key())
}

// Test-only native-consumer stand-in: exercises exact host custody, not verification.
fn consume_attachment_host_fixture<QS: Default + QueryStateAccess>(
    host: &mut CoreHostImpl<'_, '_, QS>,
    declaration: &iroha_data_model::transaction::signed_attachments::ActionIntentV1,
) -> Result<(), CustodyError> {
    host.with_attachment_invocation(|frame| {
        let request = ActionRequest {
            index: declaration.action_index,
            operation: &declaration.operation,
            role: declaration.role,
            public_inputs_digest: declaration.public_inputs_digest,
        };
        for (slot, reference) in declaration.attachments.iter().enumerate() {
            let handle = frame.open(request, slot as u32, reference.kind, reference.role)?;
            let original = frame.consume(request, handle, reference.kind, reference.role)?;
            assert!(!original.is_empty());
        }
        frame.finish_action(request)
    })
}

#[test]
fn attachment_host_actual_root_refuses_unused_batch_declaration_before_artifacts() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku AttachmentRoot { kotoage fn main() authorize(anyone) -> int { return 9; } }",
        0,
    );
    let source = attachment_host_source(&state, &authority, &root, 2, &[(&root, &[])]);
    let original = source.attachments().unwrap().occurrences()[0]
        .bytes
        .as_ptr();
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    {
        let mut first = CoreHostImpl::new(authority.clone());
        first.set_query_state(&view);
        let mut first_vm = IVM::new(1_000_000);
        bind_nested_caller(&mut first, &mut first_vm, &view, &authority, &root);
        first
            .bind_attachment_custody(
                owner
                    .contract_host(0, attachment_root_call(&source, 0))
                    .unwrap(),
            )
            .unwrap();
        first_vm.set_register(1, first_vm.memory.code_len());
        first_vm
            .run_with_host(&mut first)
            .expect("execute first real Batch root");
        assert!(!first.finish_contract_result(&first_vm).unwrap());
        let context = first.current_contract_runtime_context.clone();
        first
            .into_execution_artifacts(context)
            .expect("first root has no declaration; later root remains on the same owner");
    }
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(2, attachment_root_call(&source, 2))
            .unwrap(),
    )
    .unwrap();
    vm.set_register(1, vm.memory.code_len());
    vm.run_with_host(&mut host)
        .expect("execute actual deployed root");
    assert!(!host.finish_contract_result(&vm).unwrap());
    let context = host.current_contract_runtime_context.clone();
    assert!(matches!(host.into_execution_artifacts(context),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(message)))
        if message == CustodyError::UnconsumedAction(0).to_string()));
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    assert_eq!(
        source.attachments().unwrap().occurrences()[0]
            .bytes
            .as_ptr(),
        original
    );
}

#[test]
fn attachment_host_actual_child_rejects_unused_declaration_and_restores_business_state() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku AttachmentCaller { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let child = install_contract(
        &state,
        &authority,
        "seiyaku AttachmentChild { state StateMap<int, int> Values; kotoage fn main() authorize(anyone) -> int { Values[1] = 9; return 9; } }",
        1,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&child, &[0])]);
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    store_typed_contract_call(&mut vm, &state, &child, "main", &Json::new(()));
    let error = host
        .syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("unused child declaration rejects after genuine execution");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert_eq!(host.nested_contract_call_depth, 0);
    assert!(host.durable_state_overlay.is_empty());
    assert!(host.queued.is_empty());
    assert_eq!(
        host.current_contract_runtime_context
            .as_ref()
            .unwrap()
            .contract_subject,
        root.subject_id()
    );
    drop(host);
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn attachment_host_actual_recoverable_child_results_never_reuse_ordinals() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku OrdinalCaller { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let child = install_contract(
        &state,
        &authority,
        "seiyaku OrdinalChild { state StateMap<int, int> Values; kotoage fn main() authorize(anyone) -> Result<int, int> { Values[1] = 9; return Result::err(7); } }",
        1,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[]);
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        store_typed_contract_call(&mut vm, &state, &child, "main", &Json::new(()));
        host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
            .expect("attachment-free business Err remains recoverable");
        assert_eq!(host.nested_contract_call_depth, 0);
        assert!(host.durable_state_overlay.is_empty());
    }
    store_typed_contract_call(&mut vm, &state, &child, "missing", &Json::new(()));
    host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("failed dispatch never entered a child");
    host.finish_attachment_root().unwrap();
    drop(host);
    owner
        .finish()
        .expect("empty original owner survives recoverable child rollback");
}

#[test]
fn attachment_host_consumed_root_result_error_poison_cannot_publish() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku PoisonRoot { kotoage fn main() authorize(anyone) -> Result<int, int> { return Result::err(7); } }",
        0,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&root, &[])]);
    let declaration = &source.attachments().unwrap().actions()[0];
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    assert_ne!(
        host.authority,
        root.subject_id(),
        "callee effect authority differs from the caller"
    );
    consume_attachment_host_fixture(&mut host, declaration)
        .expect("scope selects the actual contract subject");
    vm.set_register(1, vm.memory.code_len());
    vm.run_with_host(&mut host)
        .expect("execute genuine Result::Err");
    assert!(matches!(
        host.finish_contract_result(&vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    let context = host.current_contract_runtime_context.clone();
    assert!(matches!(host.into_execution_artifacts(context),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(message)))
        if message == CustodyError::FailedAttempt.to_string()));
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn attachment_host_wrong_root_or_caller_subject_cannot_consume() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku BoundRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let other = install_contract(
        &state,
        &authority,
        "seiyaku OtherRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        1,
    );
    for (declared, actual_root_index) in [(&root, 0), (&other, 1)] {
        let source = attachment_host_source(&state, &authority, &root, 1, &[(declared, &[])]);
        let declaration = &source.attachments().unwrap().actions()[0];
        let mut owner = AttachmentCustody::new(&source).unwrap();
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            owner
                .contract_host(
                    actual_root_index,
                    attachment_root_call(&source, actual_root_index),
                )
                .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            consume_attachment_host_fixture(&mut host, declaration),
            Err(CustodyError::WrongInvocation(_)) | Err(CustodyError::WrongAuthority(_))
        ));
        drop(host);
        assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    }
}

#[test]
fn attachment_host_actual_grandchild_uses_nested_ordinal_path() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku PathRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let grandchild = install_contract(
        &state,
        &authority,
        "seiyaku PathLeaf { kotoage fn main() authorize(anyone) -> int { return 9; } }",
        1,
    );
    let middle_source = format!(
        r#"
seiyaku PathMiddle {{
    import seiyaku "path-leaf.to" as PathLeaf;
    kotoage fn main() authorize(anyone) -> int {{
        let leaf = PathLeaf::at(address: b"{grandchild}");
        return leaf.main();
    }}
}}
"#
    );
    let code = compile_bound_contract_import(&state, &grandchild, "path-leaf.to", &middle_source);
    let middle = install_contract_artifact_with_interface_and_lifecycle(
        &state,
        &authority,
        code,
        2,
        false,
        |_| {},
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&grandchild, &[0, 0])]);
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    store_typed_contract_call(&mut vm, &state, &middle, "main", &Json::new(()));
    host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("unused declaration belongs to actual grandchild [0, 0]");
    assert_eq!(host.nested_contract_call_depth, 0);
    assert!(host.queued.is_empty());
    drop(host);
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn attachment_host_real_child_success_releases_original_owner_with_artifacts() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CustodySuccessRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let child = install_contract(
        &state,
        &authority,
        "seiyaku CustodySuccessChild { state StateMap<int, int> Values; kotoage fn main() authorize(anyone) -> int { Values[1] = 9; return 9; } }",
        1,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&root, &[])]);
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    consume_attachment_host_fixture(&mut host, &source.attachments().unwrap().actions()[0])
        .unwrap();
    store_typed_contract_call(&mut vm, &state, &child, "main", &Json::new(()));
    host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .unwrap();
    assert_eq!(host.nested_contract_call_depth, 0);
    assert!(!host.durable_state_overlay.is_empty());
    let context = host.current_contract_runtime_context.clone();
    let _artifacts = host.into_execution_artifacts(context).unwrap();
    owner
        .finish()
        .expect("actual child success and complete root permit artifacts");
}

#[test]
fn attachment_host_recoverable_child_rollback_still_advances_signed_path() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CustodyOrdinalRoot { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let child = install_contract(
        &state,
        &authority,
        "seiyaku CustodyOrdinalChild { state StateMap<int, int> Values; kotoage fn main() authorize(anyone) -> Result<int, int> { Values[1] = 9; return Result::err(7); } }",
        1,
    );
    // Only the second entered child is declared. Business rollback must not reuse [0].
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&child, &[1])]);
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
    host.bind_attachment_custody(
        owner
            .contract_host(0, attachment_root_call(&source, 0))
            .unwrap(),
    )
    .unwrap();
    store_typed_contract_call(&mut vm, &state, &child, "main", &Json::new(()));
    host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect("undeclared first child rolls back normally");
    assert!(host.durable_state_overlay.is_empty());
    store_typed_contract_call(&mut vm, &state, &child, "main", &Json::new(()));
    let error = host
        .syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("second entered path [1] has an unconsumed declaration");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert!(host.durable_state_overlay.is_empty());
    drop(host);
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn attachment_host_root_must_use_original_call_and_current_authorization() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CustodyOriginal { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let other = install_contract(
        &state,
        &authority,
        "seiyaku CustodySubstitute { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        1,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[]);
    let original = attachment_root_call(&source, 0);
    let equal = original.clone();
    let mut owner = AttachmentCustody::new(&source).unwrap();
    assert!(matches!(
        owner.contract_host(0, &equal),
        Err(CustodyError::ForeignOwner)
    ));
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let mut frame = owner.contract_host(0, original).unwrap();
    assert_eq!(
        frame.require_original_call(&equal),
        Err(CustodyError::ForeignOwner)
    );
    drop(frame);
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    let mut owner = AttachmentCustody::new(&source).unwrap();
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &other);
    assert_eq!(
        host.bind_attachment_custody(owner.contract_host(0, original).unwrap()),
        Err(CustodyError::WrongInvocation(0))
    );
    drop(host);
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
}

#[test]
fn attachment_host_consumption_is_once_and_unfinished_or_unwound_hosts_poison() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let root = install_contract(
        &state,
        &authority,
        "seiyaku CustodyOnce { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let source = attachment_host_source(&state, &authority, &root, 0, &[(&root, &[])]);
    let declaration = &source.attachments().unwrap().actions()[0];
    let view = state.view();
    let mut owner = AttachmentCustody::new(&source).unwrap();
    {
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            owner
                .contract_host(0, attachment_root_call(&source, 0))
                .unwrap(),
        )
        .unwrap();
        consume_attachment_host_fixture(&mut host, declaration).unwrap();
        assert_eq!(
            consume_attachment_host_fixture(&mut host, declaration),
            Err(CustodyError::ReusedAction(0))
        );
    }
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    // A fresh prepass/VM begin cannot erase effects after custody consumption.
    let mut owner = AttachmentCustody::new(&source).unwrap();
    {
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
        host.bind_attachment_custody(
            owner
                .contract_host(0, attachment_root_call(&source, 0))
                .unwrap(),
        )
        .unwrap();
        consume_attachment_host_fixture(&mut host, declaration).unwrap();
        assert!(matches!(
            host.begin_tx(&ivm::parallel::StateAccessSet::default()),
            Err(ivm::VMError::PermissionDenied)
        ));
    }
    assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    for unwind in [false, true] {
        let mut owner = AttachmentCustody::new(&source).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut host = CoreHostImpl::new(authority.clone());
            host.set_query_state(&view);
            let mut vm = IVM::new(1_000_000);
            bind_nested_caller(&mut host, &mut vm, &view, &authority, &root);
            host.bind_attachment_custody(
                owner
                    .contract_host(0, attachment_root_call(&source, 0))
                    .unwrap(),
            )
            .unwrap();
            if unwind {
                panic!("injected host unwinding before finish");
            }
            // Ordinary unfinished-host destruction has the same poisoning owner.
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    }
}
