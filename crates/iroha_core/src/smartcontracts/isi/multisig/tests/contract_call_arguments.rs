// Canonical contract-call envelope execution; included after the runtime test macros.
/// Propose and approve, as the signer of a 1-of-1 multisig, one call built by the
/// canonical constructor that Torii and the JS codec use, and return the metadata
/// value the entrypoint recorded. With `fixed_record`, the nested invocation is
/// given a fixed argument record (the envelope Torii emitted before the fix).
fn approve_canonical_multisig_contract_call(
    entrypoint: &str,
    payload: norito::json::Value,
    fixed_record: bool,
) -> Result<Option<norito::json::Value>, ValidationFail> {
    use iroha_data_model::{
        HasMetadata,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::RegisterBox,
        prelude::Json,
        smart_contract::multisig_call::build_multisig_contract_call,
        transaction::executable::ContractArgumentRecord,
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    use iroha_model_base::name::Name;
    use kotodama_lang::compiler::{Compiler as KotodamaCompiler, CompilerMode, CompilerOptions};
    runtime_tx!(
        state,
        block,
        tx,
        World::new(),
        "multisig-canonical-contract-call-arguments"
    );
    let network_id = tx.network_id;
    tx.tx_call_hash = Some(tx.direct_execution_identity());
    let signer = checked_keypair();
    let signer_id = new_account_id(&signer);
    let owner_keypair = checked_keypair();
    let owner_id = new_account_id(&owner_keypair);
    for (account_id, label) in [
        (&owner_id, "register owner"),
        (&signer_id, "register signer"),
    ] {
        Register::account(iroha_data_model::account::NewAccount::new(
            account_id.clone(),
        ))
        .execute(&owner_id, &mut tx)
        .expect(label);
    }
    // A 1-of-1 multisig: the proposer's own approval reaches the quorum.
    execute_register(
        &mut tx,
        &owner_id,
        MultisigRegister::with_account(
            new_account_id(&checked_keypair()),
            None::<iroha_model_base::domain::DomainId>,
            spec(BTreeMap::from([(signer_id.clone(), 1)]), 1),
        ),
    )
    .expect("register multisig account");
    let multisig_id = tx
        .world
        .accounts_iter()
        .find(|account| account.id().multisig_policy().is_some())
        .map(|account| account.id().clone())
        .expect("registered multisig account");
    let (program, manifest) = KotodamaCompiler::new_with_options(CompilerOptions {
        mode: CompilerMode::Production,
        ..CompilerOptions::default()
    })
    .compile_source_with_manifest(
        r#"
seiyaku MultisigCallArguments { permission Admin;
  kotoage fn record(bool accepted) authorize(Admin) {
    if accepted {
      ledger::account::set_metadata(account: context::authority(), key: Name::parse("recorded"), value: Json::parse("1"));
    }
  }

  kotoage fn touch() authorize(Admin) {
    ledger::account::set_metadata(account: context::authority(), key: Name::parse("recorded"), value: Json::parse("2"));
  }
}
"#,
    )
    .expect("compile parameterized multisig contract");
    let code_hash = ivm::contract_code_hash(&program);
    let (_, contract_address) =
        install_trigger_contract(&mut tx, &owner_id, &owner_keypair, program, manifest, 6_062);
    Grant::account_permission(
        iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
            contract: contract_address.clone(),
            permission: "Admin".parse().expect("declared contract permission"),
        },
        multisig_id.clone(),
    )
    .execute(&owner_id, &mut tx)
    .expect("grant exact contract permission to the multisig authority");
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &multisig_id,
            &Grant::account_permission(
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: multisig_id.clone(),
                },
                contract_address.subject_id(),
            )
            .into(),
        )
        .expect("multisig owner delegates its exact metadata permission to the contract subject");
    let mut instructions = build_multisig_contract_call(
        &multisig_id,
        &contract_address,
        &"multisig_call_arguments::universal"
            .parse()
            .expect("contract alias"),
        entrypoint,
        &Json::new(payload),
        &code_hash,
        NonZeroU64::new(1).expect("positive attempt"),
    )
    .expect("canonical multisig contract call")
    .instructions;
    let Some(RegisterBox::Trigger(register)) =
        instructions[0].as_any().downcast_ref::<RegisterBox>()
    else {
        panic!("the canonical envelope registers the callback trigger")
    };
    let trigger = register.object();
    let Executable::ContractCall(invocation) = trigger.action().executable() else {
        panic!("the callback trigger executes one contract invocation")
    };
    assert!(
        invocation.arguments.is_none(),
        "the canonical envelope carries the arguments only as the firing event payload"
    );
    if fixed_record {
        let mut invocation = invocation.clone();
        invocation.arguments =
            Some(ContractArgumentRecord::try_new(vec![1]).expect("bounded record"));
        let action = Action::new(
            Executable::ContractCall(invocation),
            Repeats::Exactly(1),
            multisig_id.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger.id().clone()),
        )
        .expect("fixed-record action fixture")
        .with_metadata(trigger.action().metadata().clone());
        instructions[0] = Register::trigger(Trigger::new(trigger.id().clone(), action)).into();
    }
    let instructions_hash = HashOf::new(&instructions);
    execute_propose(
        &mut tx,
        &signer_id,
        &MultisigPropose::new(
            multisig_id.clone(),
            test_nonce(&instructions_hash),
            instructions,
            None,
        ),
    )
    .expect("signatory propose is admitted");
    execute_approve(
        &mut tx,
        &signer_id,
        &MultisigApprove::new(
            multisig_id.clone(),
            test_id(&network_id, &multisig_id, &instructions_hash),
            instructions_hash,
        ),
    )?;
    Ok(tx
        .world
        .account(&multisig_id)
        .expect("multisig account")
        .metadata()
        .get(&Name::from_str("recorded").expect("metadata key"))
        .map(|recorded| {
            recorded
                .clone()
                .try_into_any_norito::<norito::json::Value>()
                .expect("recorded metadata decodes")
        }))
}
/// The canonical envelope executes once the quorum approves: a parameterized
/// callback takes its argument record from the `ExecuteTrigger` payload, and a
/// zero-parameter callback takes the empty object. The same envelope with a fixed
/// record is refused at execution.
#[test]
fn canonical_multisig_contract_call_executes_parameterized_entrypoint_from_event_arguments() {
    assert_eq!(
        approve_canonical_multisig_contract_call(
            "record",
            norito::json!({ "accepted": true }),
            false
        )
        .expect("the quorum-reaching approval executes the parameterized contract call"),
        Some(norito::json!(1)),
        "the entrypoint received its argument from the firing event"
    );
    assert_eq!(
        approve_canonical_multisig_contract_call("touch", norito::json!({}), false)
            .expect("the quorum-reaching approval executes the zero-parameter contract call"),
        Some(norito::json!(2)),
    );
    let refusal = approve_canonical_multisig_contract_call(
        "record",
        norito::json!({ "accepted": true }),
        true,
    )
    .expect_err("a fixed argument record is refused when the proposal executes");
    assert!(
        format!("{refusal:?}").contains("fixed argument records are not allowed"),
        "unexpected refusal: {refusal:?}"
    );
}
