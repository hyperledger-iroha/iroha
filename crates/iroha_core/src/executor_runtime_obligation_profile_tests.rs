//! Signed contract execution derives kernel-obligation ownership from the actual invocation.

use super::*;
use iroha_data_model::{
    isi::runtime_obligation::{CompleteRuntimeObligationV1, RegisterContractRuntimeObligationV1},
    runtime::{lifecycle::ObligationKindV1, obligation::ObligationId},
    smart_contract::ContractArtifactId,
    transaction::executable::ContractArgumentRecord,
};

fn invocation(
    state: &State,
    program: &[u8],
    address: &ContractAddress,
    entrypoint: &str,
    request: &InstructionBox,
) -> SignedTransaction {
    let parsed =
        ivm::ProgramMetadata::parse_for_policy(program, ivm::SyscallPolicy::AbiV1RuntimeLifecycle)
            .expect("named contract metadata");
    let schema = parsed
        .contract_interface
        .as_ref()
        .unwrap()
        .entrypoints
        .iter()
        .find(|entry| entry.name == entrypoint)
        .unwrap()
        .argument_schema
        .as_ref()
        .unwrap();
    let payload = format!("0x{}", hex::encode(norito::to_bytes(request).unwrap()));
    let arguments = ivm_abi::arguments::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({ "request": payload })),
    )
    .expect("canonical bytes argument");
    TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(Executable::ContractCall(ContractInvocation {
        contract_address: address.clone(),
        expected_code_hash: ivm::contract_code_hash(program),
        entrypoint: entrypoint.to_owned(),
        arguments: Some(ContractArgumentRecord::try_new(arguments).unwrap()),
    }))
    .sign(ALICE_KEYPAIR.private_key())
}

#[test]
fn named_profile_signed_contract_calls_register_and_complete_only_the_actual_subject() {
    let compiler = kotodama_lang::compiler::Compiler::new_with_options(
        kotodama_lang::compiler::CompilerOptions {
            host_policy: ivm::SyscallPolicy::AbiV1RuntimeLifecycle,
            ..Default::default()
        },
    );
    let (program, manifest) = compiler
        .compile_source_with_manifest(
            r#"
seiyaku KernelRegistration {
  kotoage fn register(bytes request) authorize(anyone) {
    ledger::runtime::register_obligation(request);
  }
  kotoage fn complete(bytes request) authorize(anyone) {
    ledger::runtime::complete_obligation(request);
  }
}
"#,
        )
        .expect("compile genuine named-profile contract");
    let state = state_after_genesis(World::with(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    ));
    for module in [
        crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
        crate::runtime::catalog::additions::CONTRACT_LIFECYCLE.module,
    ] {
        crate::runtime::activate_additions_for_testing(&state.world, module).unwrap();
    }
    let address =
        ContractAddress::derive(&state.network_id, &ALICE_ID, 874, DataSpaceId::UNIVERSAL).unwrap();
    let subject = address.subject_id();
    let code_hash = ivm::contract_code_hash(&program);
    let artifact = ContractArtifactId::for_address(&address, code_hash).unwrap();
    let signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let mut world = state.world.block();
    world.contract_code.insert(artifact, program.clone());
    world.contract_manifests.insert(
        artifact,
        manifest
            .try_signed(signing.context(), signing.max_frame_bytes(), &ALICE_KEYPAIR)
            .unwrap(),
    );
    world.commit();
    bind_executor_test_contract_after_genesis(&state, &address, &ALICE_ID, code_hash);

    let registration: InstructionBox = RegisterContractRuntimeObligationV1 {
        kind: ObligationKindV1::Escrow,
        terminal_effects: vec![Log::new(Level::INFO, "original contract terminal".into()).into()],
        expires_ms: 60_000,
    }
    .into();
    let registered_id = ObligationId::derive(&subject).unwrap();
    let caller_id = ObligationId::derive(&ALICE_ID).unwrap();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    let mut cache = IvmCache::new();
    let register = invocation(&state, &program, &address, "register", &registration);
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(register.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    crate::executor::Executor::Initial
        .execute_transaction(&mut tx, &ALICE_ID, &register, &mut cache)
        .expect("real contract queues and executes its kernel registration");
    assert!(
        tx.last_tx_gas_used > 0,
        "the actual VM execution is metered"
    );
    let record = tx.world.runtime_obligations.get(&registered_id).unwrap();
    assert_eq!(record.owner, subject);
    assert_eq!(record.contract.as_ref(), Some(&address));
    assert_eq!(record.created_at, 2);
    assert!(!record.capture.host_profiles.is_empty());
    assert!(tx.world.runtime_obligations.get(&caller_id).is_none());
    tx.apply();

    // A warm-cache invocation still cannot substitute the caller for the bound subject.
    let wrong_owner = invocation(
        &state,
        &program,
        &address,
        "complete",
        &CompleteRuntimeObligationV1 {
            owner: ALICE_ID.clone(),
        }
        .into(),
    );
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(wrong_owner.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    assert!(matches!(
        crate::executor::Executor::Initial.execute_transaction(
            &mut tx,
            &ALICE_ID,
            &wrong_owner,
            &mut cache
        ),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(_))
    ));
    assert!(tx.last_tx_gas_used > 0);
    assert!(tx.world.runtime_obligations.get(&registered_id).is_some());
    drop(tx);

    let complete = invocation(
        &state,
        &program,
        &address,
        "complete",
        &CompleteRuntimeObligationV1 { owner: subject }.into(),
    );
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(complete.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    crate::executor::Executor::Initial
        .execute_transaction(&mut tx, &ALICE_ID, &complete, &mut cache)
        .expect("real contract completes its own obligation");
    assert!(tx.last_tx_gas_used > 0);
    assert!(tx.world.runtime_obligations.get(&registered_id).is_none());
    assert!(tx.world.runtime_obligations.get(&caller_id).is_none());
}
