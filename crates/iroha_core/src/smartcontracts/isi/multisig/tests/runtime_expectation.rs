//! A retained approval never silently authorizes changed runtime semantics.

use super::*;
use iroha_data_model::runtime::{
    definition::{DefinitionDescriptorV1, ImplementationCodeV1, RestrictionEntryV1, RestrictionV1},
    lifecycle::RuntimeRejectionV1,
};

fn rebind_runtime(tx: &mut StateTransaction<'_, '_>, instruction: &InstructionBox) {
    let wire = iroha_data_model::isi::instruction_wire_id(instruction).unwrap();
    let mut selection = tx.world.runtime_selection.get().clone();
    let binding = selection
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == wire)
        .unwrap();
    let mut descriptor = tx
        .world
        .runtime_definitions
        .get(&binding.implementation)
        .unwrap()
        .clone();
    let DefinitionDescriptorV1::Implementation(implementation) = &mut descriptor else {
        panic!("implementation")
    };
    implementation.code = ImplementationCodeV1::Native {
        module: "test.changed.semantics".into(),
        reference_closure: [0x83; 32],
    };
    let id = descriptor.id().unwrap();
    binding.implementation = id;
    selection.active.push(id);
    selection.active.sort();
    selection.active.dedup();
    tx.world.runtime_definitions.insert(id, descriptor);
    selection
        .check_closure(&crate::runtime::change_set::PublishedDefinitions(
            &tx.world.runtime_definitions,
        ))
        .unwrap();
    *tx.world.runtime_selection.get_mut() = selection;
}

#[test]
fn a_multisig_rebinding_requires_a_new_occurrence_and_fresh_approvals() {
    with_occurrence_fixture(|tx, first, second, account, asset| {
        let instructions: Vec<InstructionBox> = vec![
            iroha_data_model::isi::Transfer::asset_quantity(asset.clone(), 3_u32, first.clone())
                .into(),
        ];
        let hash = HashOf::new(&instructions);
        let nonce = MultisigProposalNonce::new([0x81; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), nonce, instructions.clone(), None),
        )
        .unwrap();
        let original = proposal_state(tx, account, &id).unwrap();
        let key = multisig_proposal_expectation_key(account, &id);
        let capture = tx.world.smart_contract_state.get(&key).unwrap().clone();
        assert_eq!(
            norito::decode_from_bytes::<Hash>(&capture).unwrap(),
            crate::runtime::expectation::digest_owned(
                &crate::runtime::expectation::capture_instructions(&instructions, &tx.world)
                    .unwrap(),
                &tx.world,
            )
            .unwrap()
        );

        rebind_runtime(tx, &instructions[0]);
        assert!(matches!(
            execute_approve(tx, second, &MultisigApprove::new(account.clone(), id, hash)),
            Err(ValidationFail::InstructionFailed(
                InstructionExecutionError::Runtime(RuntimeRejectionV1::ExpectationMismatch { .. })
            ))
        ));
        assert_eq!(proposal_state(tx, account, &id).unwrap(), original);
        assert_eq!(tx.world.smart_contract_state.get(&key), Some(&capture));

        let renewed_nonce = MultisigProposalNonce::new([0x82; 32]).unwrap();
        let renewed_id = MultisigProposalId::derive(&tx.network_id, account, &renewed_nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), renewed_nonce, instructions, None),
        )
        .unwrap();
        execute_approve(
            tx,
            second,
            &MultisigApprove::new(account.clone(), renewed_id, hash),
        )
        .unwrap();
        assert!(
            tx.world
                .smart_contract_state
                .get(&multisig_proposal_expectation_key(account, &renewed_id))
                .is_none()
        );
        assert_eq!(
            proposal_state(tx, account, &id).unwrap(),
            original,
            "renewal never reuses old consent"
        );
    });
}

#[test]
fn changed_multisig_consent_can_be_cancelled_and_its_capture_is_retired() {
    with_occurrence_fixture(|tx, first, _, account, asset| {
        let instructions: Vec<InstructionBox> = vec![
            iroha_data_model::isi::Transfer::asset_quantity(asset.clone(), 3_u32, first.clone())
                .into(),
        ];
        let hash = HashOf::new(&instructions);
        let nonce = MultisigProposalNonce::new([0x84; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), nonce, instructions.clone(), None),
        )
        .unwrap();
        rebind_runtime(tx, &instructions[0]);
        execute_cancel(tx, account, &MultisigCancel::new(account.clone(), id, hash)).unwrap();
        assert!(
            tx.world
                .smart_contract_state
                .get(&multisig_proposal_expectation_key(account, &id))
                .is_none()
        );
        assert!(proposal_state(tx, account, &id).is_err());
        validate_persisted_proposals(&tx.world, Some(&tx.network_id)).unwrap();
    });
}

#[test]
fn missing_multisig_capture_never_adopts_the_current_runtime_implicitly() {
    with_occurrence_fixture(|tx, first, second, account, _| {
        let instructions: Vec<InstructionBox> =
            vec![Log::new(Level::INFO, "original consent".into()).into()];
        let hash = HashOf::new(&instructions);
        let nonce = MultisigProposalNonce::new([0x85; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), nonce, instructions, None),
        )
        .unwrap();
        tx.world
            .smart_contract_state
            .remove(multisig_proposal_expectation_key(account, &id));
        assert!(matches!(
            execute_approve(tx, second, &MultisigApprove::new(account.clone(), id, hash)),
            Err(ValidationFail::InstructionFailed(
                InstructionExecutionError::Runtime(RuntimeRejectionV1::ExpectationMismatch { .. })
            ))
        ));
        assert!(validate_persisted_proposals(&tx.world, Some(&tx.network_id)).is_err());
    });
}

#[test]
fn restricted_multisig_admission_never_creates_a_pending_occurrence() {
    for restriction in [
        RestrictionV1::AdmissionClosed(0),
        RestrictionV1::CompletionOnly(0),
        RestrictionV1::Disabled(0),
    ] {
        with_occurrence_fixture(|tx, first, _, account, asset| {
            let instructions: Vec<InstructionBox> = vec![
                iroha_data_model::isi::Transfer::asset_quantity(
                    asset.clone(),
                    3_u32,
                    first.clone(),
                )
                .into(),
            ];
            let wire = iroha_data_model::isi::instruction_wire_id(&instructions[0]).unwrap();
            let definition = tx
                .world
                .runtime_selection
                .get()
                .bindings
                .iter()
                .find(|binding| binding.wire_id == wire)
                .unwrap()
                .interface;
            tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
                definition,
                restriction,
            }];
            let nonce = MultisigProposalNonce::new([0x88; 32]).unwrap();
            let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
            let before: BTreeMap<_, _> = tx
                .world
                .smart_contract_state
                .iter()
                .map(|(key, bytes)| (key.clone(), bytes.clone()))
                .collect();
            let events_before = tx.world.internal_event_buf.len();
            let error = execute_propose(
                tx,
                first,
                &MultisigPropose::new(account.clone(), nonce, instructions, None),
            )
            .unwrap_err();
            assert!(matches!(error,
                ValidationFail::InstructionFailed(InstructionExecutionError::Runtime(RuntimeRejectionV1::DefinitionRestricted { definition: blocked, restriction: actual }))
                    if blocked == definition && actual == restriction));
            assert_eq!(
                tx.world
                    .smart_contract_state
                    .iter()
                    .map(|(key, bytes)| (key.clone(), bytes.clone()))
                    .collect::<BTreeMap<_, _>>(),
                before,
                "neither a proposal, its initial approval, nor its capture may be published"
            );
            assert!(
                tx.world
                    .smart_contract_state
                    .get(&multisig_proposal_state_key(account, &id))
                    .is_none()
            );
            assert!(
                tx.world
                    .smart_contract_state
                    .get(&multisig_proposal_expectation_key(account, &id))
                    .is_none()
            );
            assert_eq!(tx.world.internal_event_buf.len(), events_before);
        });
    }
}

#[test]
fn replacing_the_installed_executor_requires_fresh_multisig_consent() {
    with_occurrence_fixture(|tx, first, second, account, _| {
        let instructions: Vec<InstructionBox> =
            vec![Log::new(Level::INFO, "executor-bound".into()).into()];
        let hash = HashOf::new(&instructions);
        let nonce = MultisigProposalNonce::new([0x86; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), nonce, instructions, None),
        )
        .unwrap();
        let original = proposal_state(tx, account, &id).unwrap();
        *tx.world.executor.get_mut() = crate::executor::allowing_executor_for_testing();
        assert!(matches!(
            execute_approve(tx, second, &MultisigApprove::new(account.clone(), id, hash)),
            Err(ValidationFail::InstructionFailed(
                InstructionExecutionError::Runtime(RuntimeRejectionV1::ExpectationMismatch { .. })
            ))
        ));
        assert_eq!(proposal_state(tx, account, &id).unwrap(), original);
    });
}

#[test]
fn a_local_capture_decoder_refusal_retains_retry_custody_and_original_consent() {
    with_occurrence_fixture(|tx, first, _, account, _| {
        let instructions: Vec<InstructionBox> =
            vec![Log::new(Level::INFO, "original capture".into()).into()];
        let nonce = MultisigProposalNonce::new([0x87; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), nonce, instructions, None),
        )
        .unwrap();
        let original = proposal_state(tx, account, &id).unwrap();
        // Hash decoding borrows its frame and copies fixed bytes. Constrain
        // its root field, since it needs no sequence or allocation budget.
        let result = norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64),
            || check_proposal_expectation(tx, &original),
        );
        assert!(result.is_err());
        assert!(
            tx.take_execution_deferral().is_some(),
            "a local ceiling is never a final rejection"
        );
        assert_eq!(proposal_state(tx, account, &id).unwrap(), original);
    });
}

#[test]
fn retained_and_unloaded_executables_capture_the_same_semantics() {
    use crate::runtime::expectation::{
        capture_executable, capture_instructions, capture_loaded_executable,
    };
    use crate::smartcontracts::isi::triggers::set::ExecutableRef;
    use iroha_data_model::transaction::{
        Executable,
        executable::{ExecutableBatchItem, IvmBytecode},
    };
    with_occurrence_fixture(|tx, _, _, _, _| {
        let instructions: Vec<InstructionBox> =
            vec![Log::new(Level::INFO, "same captured call".into()).into()];
        let expected = capture_instructions(&instructions, &tx.world).unwrap();
        let batch: iroha_primitives::const_vec::ConstVec<_> = instructions
            .iter()
            .cloned()
            .map(ExecutableBatchItem::Instruction)
            .collect();
        assert_eq!(
            capture_executable(
                &Executable::Instructions(instructions.clone().into()),
                &tx.world
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            capture_loaded_executable(&ExecutableRef::Instructions(instructions.into()), &tx.world)
                .unwrap(),
            expected
        );
        assert_eq!(
            capture_executable(&Executable::Batch(batch.clone()), &tx.world).unwrap(),
            expected
        );
        assert_eq!(
            capture_loaded_executable(&ExecutableRef::Batch(batch), &tx.world).unwrap(),
            expected
        );
        let code = IvmBytecode::from_compiled(Vec::new());
        let loaded = ExecutableRef::Ivm(HashOf::new(&code));
        let opaque = capture_executable(&Executable::Ivm(code), &tx.world).unwrap();
        assert!(opaque.exact_bindings);
        assert_eq!(
            capture_loaded_executable(&loaded, &tx.world).unwrap(),
            opaque
        );
    });
}
