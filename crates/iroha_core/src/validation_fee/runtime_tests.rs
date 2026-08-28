#[test]
fn treasury_payout_is_exempt_when_enacted_policy_lists_class() {
    use iroha_data_model::{
        block::BlockHeader,
        nexus::DataSpaceId,
        prelude::{Account, AssetDefinition, Domain},
        smart_contract::ContractAddress,
    };
    let deployer_key = key_pair(55);
    let deployer = AccountId::new(deployer_key.public_key().clone());
    let domain_id = DomainId::try_new("contracts", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&deployer);
    let fee_domain = Domain::new(DomainId::try_new("fees", "paynet").expect("fee-asset domain id"))
        .build(&deployer);
    let mut accounts = vec![Account::new(deployer.clone()).build(&deployer)];
    accounts.extend((2..=7).map(|seed| Account::new(account(seed)).build(&deployer)));
    let fee_definition = AssetDefinition::new(
        fee_asset(),
        "fee_token".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&deployer);
    let xor_definition = AssetDefinition::new(
        xor_asset(),
        "xor".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&deployer);
    let world = crate::state::World::with(
        [domain, fee_domain],
        accounts,
        [fee_definition, xor_definition],
    );
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(TEST_POLICY_EFFECTIVE_HEIGHT)
            .expect("test policy effective height is non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    let deployment_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    crate::smartcontracts::Execute::execute(
        iroha_data_model::isi::Grant::account_permission(deployment_permission, deployer.clone()),
        &deployer,
        &mut state_tx,
    )
    .expect("grant contract lifecycle authority");
    let (code, manifest) = minimal_bound_contract_artifact();
    let code_hash =
        crate::smartcontracts::code::register_code_bytes(&deployer, code.clone(), &mut state_tx)
            .expect("register contract bytes");
    crate::smartcontracts::code::register_manifest(
        &deployer,
        manifest.signed(&deployer_key),
        &mut state_tx,
    )
    .expect("register signed contract manifest");
    let contract_address =
        ContractAddress::derive(&state_tx.network_id, &deployer, 0, DataSpaceId::UNIVERSAL)
            .expect("contract address");
    crate::smartcontracts::code::activate_instance(
        &deployer,
        contract_address.clone(),
        code_hash,
        &mut state_tx,
    )
    .expect("activate contract instance");
    let binding = treasury_payout_binding(contract_address.clone(), &code);
    let payout_trigger_id = register_bound_payout_time_trigger(
        &mut state_tx,
        &binding,
        code_hash,
        "validation_fee_payout_tick",
    );
    let treasury = binding.treasury_account_id.clone();
    let lifecycle_seal = binding
        .lifecycle_seal()
        .expect("derive test payout lifecycle seal");
    let policy = policy_with_treasury_payout_lifecycle(binding.clone());
    let mut wrong_code_binding = binding.clone();
    wrong_code_binding.code_hash[0] ^= 0xff;
    let wrong_code_policy = policy_with_treasury_payout_lifecycle(wrong_code_binding);
    let wrong_code_registry = policy_registry(std::slice::from_ref(&wrong_code_policy));
    install_policy_registry_fixture(&wrong_code_registry, &mut state_tx);
    let wrong_code_error = active_policy(&state_tx)
        .expect_err("the governed binding cannot name another SHA-256 artifact");
    assert!(
        matches!(wrong_code_error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("deployed code hash differs from the enacted binding")),
        "unexpected governed code-hash rejection: {wrong_code_error:?}",
    );
    let registry = policy_registry(std::slice::from_ref(&policy));
    install_policy_registry_fixture(&registry, &mut state_tx);
    active_policy(&state_tx)
        .expect("read active policy")
        .expect("policy is active");
    let runtime = crate::executor::ContractRuntimeExecutionContext {
        contract_address: contract_address.clone(),
        contract_subject: treasury.clone(),
        contract_alias: None,
        entrypoint: binding.entrypoint.to_string(),
    };
    let instructions = canonical_treasury_payout_plan(&binding, Quantity::from(20_u64));
    let ordered = ordered_treasury_payout_plan(&binding, &instructions);
    let groups = std::collections::BTreeMap::from([(treasury.clone(), instructions.clone())]);
    for rejected_origin in [
        None,
        Some(crate::executor::ContractRuntimeExecutionContext {
            contract_address: runtime.contract_address.clone(),
            contract_subject: runtime.contract_subject.clone(),
            contract_alias: None,
            entrypoint: "swap_quote_for_base".to_owned(),
        }),
        Some(crate::executor::ContractRuntimeExecutionContext {
            contract_address: test_contract_address(),
            contract_subject: test_contract_address().subject_id(),
            contract_alias: None,
            entrypoint: binding.entrypoint.to_string(),
        }),
    ] {
        let origin = rejected_origin
            .as_ref()
            .map(|context| OpaqueDeferredRuntimeOrigin::new(context, &code));
        let error =
            enforce_opaque_deferred_instruction_groups(&groups, &ordered, &mut state_tx, origin)
                .expect_err(
                    "direct execution, a wrong entrypoint, and another address must not use credit",
                );
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(ref message)
                ) if message.contains("opaque deferred executable derived a policy fee-asset transfer")),
            "unexpected unbound-runtime rejection: {error:?}",
        );
    }
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &std::collections::BTreeMap::new(),
            &[],
            &mut state_tx,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("a bound pool may report no payout when no batch is available"),
        OpaqueDeferredValidationOutcome::NoOp,
    );
    let payout_credit_minor_units =
        quantity_to_minor_units(&binding.batch_ds, policy.ds_scale, usize::MAX)
            .expect("payout batch must fit the policy minor-unit domain");
    let payout_credit = ValidationFeeCredit::from_policy_minor_units(
        treasury.clone(),
        lifecycle_seal,
        policy_fee_asset(&policy),
        policy.ds_scale,
        payout_credit_minor_units,
    )
    .expect("convert payout credit into nominal quantity");
    commit_validation_fee_credit(&mut state_tx, Some(&payout_credit))
        .expect("seed consensus validation-fee credit");
    let (_, asset_binding_key) = validation_fee_credit_state_keys(&state_tx, &payout_credit)
        .expect("resolve treasury credit paths");
    let valid_asset_binding = state_tx
        .world
        .smart_contract_state
        .get(&asset_binding_key)
        .expect("credit commit must bind its fee asset")
        .clone();
    state_tx
        .world
        .smart_contract_state
        .remove(asset_binding_key.clone());
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditAssetBinding { .. })
    ));
    state_tx
        .world
        .smart_contract_state
        .insert(asset_binding_key.clone(), vec![0xFF]);
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditAssetBinding { .. })
    ));
    state_tx
        .world
        .smart_contract_state
        .insert(asset_binding_key, valid_asset_binding);
    let wrong_asset_credit = ValidationFeeCredit::from_policy_minor_units(
        treasury.clone(),
        lifecycle_seal,
        asset_definition("unrelated_ds_successor"),
        policy.ds_scale,
        1,
    )
    .expect("convert wrong-asset fixture credit");
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &wrong_asset_credit),
        Err(ValidationFeeAdmissionError::CreditAssetBindingMismatch { .. })
    ));
    let expected_credit_key = validation_fee_credit_lifecycle_state_key_for_address(
        &runtime.contract_address,
        lifecycle_seal,
        VALIDATION_FEE_CREDIT_STATE_LEAF,
    );
    let projected_credit_key =
        validation_fee_credit_state_key_for_address(&runtime.contract_address);
    assert_eq!(
        validation_fee_credit_state_keys(&state_tx, &payout_credit)
            .expect("resolve treasury credit path")
            .0,
        expected_credit_key,
        "native credit must use the immutable contract and lifecycle-seal scope"
    );
    assert!(
        expected_credit_key
            .as_ref()
            .ends_with("/AvailableValidationFeeCredit")
    );
    let retired_key: StatePath = expected_credit_key
        .as_ref()
        .replace(
            "AvailableValidationFeeCredit",
            "AvailableValidationFeeMinorUnits",
        )
        .parse()
        .expect("retired credit path remains a syntactically valid state path");
    assert!(
        !is_validation_fee_credit_state_key(&retired_key),
        "the first release must not reserve or decode the retired fixed-width leaf"
    );
    let canonical_credit_state = state_tx
        .world
        .smart_contract_state
        .get(&expected_credit_key)
        .expect("credit commit must write its state value")
        .clone();
    assert_eq!(
        state_tx
            .world
            .smart_contract_state
            .get(&projected_credit_key),
        Some(&canonical_credit_state),
        "the fixed contract-visible leaf must project the active sealed balance"
    );
    let projection_seal_key =
        validation_fee_credit_lifecycle_seal_state_key_for_address(&runtime.contract_address);
    assert_eq!(
        state_tx
            .world
            .smart_contract_state
            .get(&projection_seal_key)
            .and_then(|bytes| norito::decode_from_bytes::<[u8; 32]>(bytes).ok()),
        Some(lifecycle_seal),
        "the contract-visible projection must advertise its immutable lifecycle seal"
    );
    let record: StateValueRecordV1 = norito::decode_from_bytes(&canonical_credit_state)
        .expect("credit must use a state-value record");
    assert_eq!(record.atoms.len(), 1);
    assert_eq!(
        decode_validation_fee_credit_state_value(&canonical_credit_state)
            .expect("decode canonical nominal credit"),
        payout_credit.amount
    );
    state_tx.world.smart_contract_state.insert(
        expected_credit_key.clone(),
        norito::to_bytes(&100_i64).expect("encode retired primitive state value"),
    );
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
    ));
    state_tx
        .world
        .smart_contract_state
        .remove(expected_credit_key.clone());
    state_tx.world.smart_contract_state.insert(
        retired_key.clone(),
        norito::to_bytes(&100_i64).expect("encode retired fixed-width credit leaf"),
    );
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
    ));
    state_tx.world.smart_contract_state.remove(retired_key);
    let mut noncanonical_record = canonical_credit_state.clone();
    noncanonical_record.push(0);
    state_tx
        .world
        .smart_contract_state
        .insert(expected_credit_key.clone(), noncanonical_record);
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
    ));
    let mut wrong_schema_record = record.clone();
    wrong_schema_record.schema_hash[0] ^= 1;
    state_tx.world.smart_contract_state.insert(
        expected_credit_key.clone(),
        norito::to_bytes(&wrong_schema_record).expect("encode wrong-schema state record"),
    );
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
    ));
    let wrong_scale: Quantity = "0.001".parse().expect("canonical scale-three quantity");
    state_tx.world.smart_contract_state.insert(
        expected_credit_key.clone(),
        encode_validation_fee_credit_state_value(&wrong_scale)
            .expect("encode schema-bound wrong-scale credit"),
    );
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit),
        Err(ValidationFeeAdmissionError::CreditAmountOutsideAssetSpec {
            amount,
            allowed_scale: 2,
            ..
        }) if amount == wrong_scale
    ));
    let wrong_policy_scale = ValidationFeeCredit::from_policy_minor_units(
        treasury.clone(),
        lifecycle_seal,
        policy_fee_asset(&policy),
        policy.ds_scale - 1,
        10,
    )
    .expect("construct mismatched policy-scale fixture");
    state_tx
        .world
        .smart_contract_state
        .insert(expected_credit_key.clone(), canonical_credit_state.clone());
    assert!(matches!(
        read_validation_fee_credit_balance(&state_tx, &wrong_policy_scale),
        Err(
            ValidationFeeAdmissionError::CreditAssetNumericSpecMismatch {
                expected_scale: 1,
                observed_scale: Some(2),
                ..
            }
        )
    ));
    let excessive_debit_minor_units = payout_credit_minor_units
        .checked_mul(2)
        .expect("fixture debit must fit minor-unit domain");
    let excessive_debit = ValidationFeeCredit::from_policy_minor_units(
        treasury.clone(),
        lifecycle_seal,
        policy_fee_asset(&policy),
        policy.ds_scale,
        excessive_debit_minor_units,
    )
    .expect("construct underflow fixture");
    let excessive_debit_amount = payout_credit
        .amount
        .checked_add(&payout_credit.amount)
        .expect("fixture debit must fit nominal quantity");
    assert!(matches!(
        consume_validation_fee_credit(&mut state_tx, &excessive_debit),
        Err(ValidationFeeAdmissionError::InsufficientCreditBalance {
            available,
            requested,
        }) if available == payout_credit.amount && requested == excessive_debit_amount
    ));
    let wide: Quantity = "18446744073709551616"
        .parse()
        .expect("canonical credit above u64::MAX");
    state_tx.world.smart_contract_state.insert(
        expected_credit_key.clone(),
        encode_validation_fee_credit_state_value(&wide).expect("encode wide nominal credit"),
    );
    let one_tenth_credit = ValidationFeeCredit::from_policy_minor_units(
        treasury.clone(),
        lifecycle_seal,
        policy_fee_asset(&policy),
        policy.ds_scale,
        TEST_VALIDATION_FEE_MINOR_UNITS,
    )
    .expect("construct one-tenth credit");
    commit_validation_fee_credit(&mut state_tx, Some(&one_tenth_credit))
        .expect("accumulated nominal credit may exceed the u64 policy-scalar domain");
    assert_eq!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit)
            .expect("read accumulated wide credit"),
        "18446744073709551616.1"
            .parse::<Quantity>()
            .expect("canonical accumulated wide credit")
    );
    let mut maximum_bytes = vec![0xff_u8; iroha_primitives::numeric::MAX_MANTISSA_BYTES];
    *maximum_bytes.last_mut().expect("non-empty mantissa") = 0x7f;
    let maximum_mantissa = iroha_primitives::bigint::BigInt::from_twos_bytes(&maximum_bytes)
        .expect("maximum signed 512-bit mantissa");
    let maximum: Quantity = maximum_mantissa
        .to_string()
        .parse()
        .expect("maximum non-negative quantity");
    state_tx.world.smart_contract_state.insert(
        expected_credit_key.clone(),
        encode_validation_fee_credit_state_value(&maximum).expect("encode maximum credit"),
    );
    let overflow = commit_validation_fee_credit(&mut state_tx, Some(&one_tenth_credit))
        .expect_err("credit addition must reject Quantity overflow");
    assert!(
        matches!(overflow, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains(&maximum.to_string()) && message.contains("additional 0.1")),
        "overflow diagnostics must retain exact quantities: {overflow:?}"
    );
    assert_eq!(
        read_validation_fee_credit_balance(&state_tx, &payout_credit)
            .expect("failed addition must leave maximum credit unchanged"),
        maximum
    );
    state_tx
        .world
        .smart_contract_state
        .insert(expected_credit_key, canonical_credit_state.clone());
    state_tx
        .world
        .smart_contract_state
        .insert(projected_credit_key, canonical_credit_state);
    state_tx.apply();
    {
        let mut failed_signed_transaction = block.transaction();
        let exact_fee_credit = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            lifecycle_seal,
            policy_fee_asset(&policy),
            policy.ds_scale,
            TEST_VALIDATION_FEE_MINOR_UNITS,
        )
        .expect("convert exact policy fee into nominal quantity");
        commit_validation_fee_credit(&mut failed_signed_transaction, Some(&exact_fee_credit))
            .expect("stage exact transaction-bound fee credit");
        let staged_credit = payout_credit
            .amount
            .checked_add(&exact_fee_credit.amount)
            .expect("staged fixture credit must fit nominal quantity");
        assert_eq!(
            read_validation_fee_credit_balance(&failed_signed_transaction, &payout_credit,)
                .expect("read staged fee credit"),
            staged_credit
        );
        // Simulate a later transaction/data-trigger failure: no staged credit is applied.
    }
    {
        let mut failed_trigger_transaction = block.transaction();
        assert_eq!(
            read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit)
                .expect("read credit after failed signed transaction"),
            payout_credit.amount,
            "a failed signed transaction must not create fee credit"
        );
        assert_eq!(
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &ordered,
                &mut failed_trigger_transaction,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &runtime,
                    &code,
                    &payout_trigger_id,
                )),
            )
            .expect("matching runtime may stage an exactly credited payout"),
            OpaqueDeferredValidationOutcome::Apply
        );
        assert_eq!(
            read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit,)
                .expect("read staged debit"),
            Quantity::zero(),
            "the validator stages the debit in the trigger subtransaction"
        );
        failed_trigger_transaction
            .world
            .smart_contract_state
            .insert(
                "ValidationFeeFinalLegRollbackSentinel"
                    .parse()
                    .expect("rollback sentinel key"),
                vec![1],
            );
        // Simulate failure of the sixth (final validator) transfer: dropping this
        // subtransaction must roll back the pool-state artifact and native credit debit.
    }
    let mut successful_trigger_transaction = block.transaction();
    assert_eq!(
        read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
            .expect("read rolled-back debit"),
        payout_credit.amount,
        "a failed trigger subtransaction must roll its staged credit debit back"
    );
    assert!(
        successful_trigger_transaction
            .world
            .smart_contract_state
            .get(
                &"ValidationFeeFinalLegRollbackSentinel"
                    .parse::<StatePath>()
                    .expect("rollback sentinel key")
            )
            .is_none(),
        "a final-leg failure must roll back staged pool state as well as credit",
    );
    let altered_code = [code.as_slice(), &[0_u8]].concat();
    let error = enforce_opaque_deferred_instruction_groups(
        &groups,
        &ordered,
        &mut successful_trigger_transaction,
        Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
            &runtime,
            &altered_code,
            &payout_trigger_id,
        )),
    )
    .expect_err("altered runtime code must not receive the payout exception");
    assert!(
        matches!(error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("scheduled trigger or executed runtime identity matches")
                && message.contains("pair is not exact")),
        "unexpected altered-code rejection: {error:?}",
    );
    let wrong_runtime = crate::executor::ContractRuntimeExecutionContext {
        contract_address: contract_address.clone(),
        contract_subject: deployer,
        contract_alias: None,
        entrypoint: binding.entrypoint.to_string(),
    };
    assert!(
        enforce_opaque_deferred_instruction_groups(
            &groups,
            &ordered,
            &mut successful_trigger_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &wrong_runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .is_err(),
        "a signable runtime authority must not inherit the contract-subject exception"
    );
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &groups,
            &ordered,
            &mut successful_trigger_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("matching active contract runtime may make its direct treasury payout"),
        OpaqueDeferredValidationOutcome::Apply
    );
    assert_eq!(
        read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
            .expect("read consumed validation-fee credit"),
        Quantity::zero(),
        "matching payout consumes exactly its policy-minor-unit debit"
    );
    successful_trigger_transaction.apply();
    let mut exhausted_credit_transaction = block.transaction();
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &groups,
            &ordered,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("insufficient reserved credit is a legitimate atomic no-op"),
        OpaqueDeferredValidationOutcome::NoOp
    );
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &std::collections::BTreeMap::new(),
            &[],
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("an empty bound tick is a legitimate no-op"),
        OpaqueDeferredValidationOutcome::NoOp
    );
    let partial_credit = payout_credit.with_amount(
        "3.33"
            .parse()
            .expect("partial lifecycle credit is canonical"),
    );
    commit_validation_fee_credit(&mut exhausted_credit_transaction, Some(&partial_credit))
        .expect("seed partial lifecycle credit");
    let mut zero_quote = vec![
        transfer(
            &treasury,
            &binding.ds_asset_id,
            partial_credit.amount.clone(),
            &binding.pool_vault_account_id,
        ),
        transfer(
            &binding.pool_vault_account_id,
            &binding.xor_asset_id,
            Quantity::zero(),
            &treasury,
        ),
    ];
    let mut canonical_recipients = binding
        .recipients
        .iter()
        .map(|recipient| recipient.account_id.clone())
        .collect::<Vec<_>>();
    canonical_recipients.sort();
    zero_quote.extend(canonical_recipients.into_iter().map(|recipient| {
        transfer(
            &treasury,
            &binding.xor_asset_id,
            Quantity::zero(),
            &recipient,
        )
    }));
    let zero_quote_ordered = ordered_treasury_payout_plan(&binding, &zero_quote);
    let zero_quote_groups = std::collections::BTreeMap::from([(treasury.clone(), zero_quote)]);
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &zero_quote_groups,
            &zero_quote_ordered,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("a zero deterministic quote is an atomic no-op"),
        OpaqueDeferredValidationOutcome::NoOp
    );
    assert_eq!(
        read_validation_fee_credit_balance(&exhausted_credit_transaction, &partial_credit)
            .expect("read credit after zero quote"),
        partial_credit.amount,
        "a zero quote must not debit lifecycle credit"
    );
    let out_of_range = treasury_payout_plan(
        &binding,
        partial_credit.amount.clone(),
        "33.31".parse().expect("out-of-range partial quote"),
    );
    let out_of_range_ordered = ordered_treasury_payout_plan(&binding, &out_of_range);
    let out_of_range_groups = std::collections::BTreeMap::from([(treasury.clone(), out_of_range)]);
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &out_of_range_groups,
            &out_of_range_ordered,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("an out-of-range deterministic quote is an atomic no-op"),
        OpaqueDeferredValidationOutcome::NoOp
    );
    assert_eq!(
        read_validation_fee_credit_balance(&exhausted_credit_transaction, &partial_credit)
            .expect("read credit after invalid quote"),
        partial_credit.amount,
        "an invalid quote must not debit lifecycle credit"
    );
    let partial_plan = treasury_payout_plan(
        &binding,
        partial_credit.amount.clone(),
        "20.03".parse().expect("valid partial quote"),
    );
    let partial_ordered = ordered_treasury_payout_plan(&binding, &partial_plan);
    let partial_groups = std::collections::BTreeMap::from([(treasury.clone(), partial_plan)]);
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &partial_groups,
            &partial_ordered,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("a proportional partial-credit payout is valid"),
        OpaqueDeferredValidationOutcome::Apply
    );
    assert_eq!(
        read_validation_fee_credit_balance(&exhausted_credit_transaction, &partial_credit)
            .expect("read drained partial credit"),
        Quantity::zero()
    );
    let mut successor_seal = lifecycle_seal;
    successor_seal[0] ^= 1;
    let successor_credit = ValidationFeeCredit {
        treasury_account_id: treasury.clone(),
        lifecycle_seal: successor_seal,
        fee_asset_definition_id: policy_fee_asset(&policy),
        asset_scale: policy.ds_scale,
        amount: Quantity::from(1_u64),
    };
    commit_validation_fee_credit(&mut exhausted_credit_transaction, Some(&successor_credit))
        .expect("seed successor lifecycle credit");
    assert_eq!(
        enforce_opaque_deferred_instruction_groups(
            &groups,
            &ordered,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &code,
                &payout_trigger_id,
            )),
        )
        .expect("an old lifecycle cannot consume successor credit"),
        OpaqueDeferredValidationOutcome::NoOp
    );
    assert_eq!(
        read_validation_fee_credit_balance(&exhausted_credit_transaction, &successor_credit)
            .expect("read isolated successor credit"),
        successor_credit.amount
    );
    consume_validation_fee_credit(&mut exhausted_credit_transaction, &successor_credit)
        .expect("drain successor fixture");
    assert_eq!(
        read_validation_fee_credit_balance(&exhausted_credit_transaction, &successor_credit)
            .expect("read retained successor lifecycle credit"),
        Quantity::zero(),
        "a drained first-release lifecycle must retain a canonical zero balance"
    );
    let (successor_balance_key, successor_asset_key) =
        validation_fee_credit_state_keys(&exhausted_credit_transaction, &successor_credit)
            .expect("resolve retained successor lifecycle state");
    assert!(
        exhausted_credit_transaction
            .world
            .smart_contract_state
            .get(&successor_balance_key)
            .is_some(),
        "a drained first-release lifecycle must retain its canonical zero balance"
    );
    assert!(
        exhausted_credit_transaction
            .world
            .smart_contract_state
            .get(&successor_asset_key)
            .is_some(),
        "a drained first-release lifecycle must retain its immutable asset binding"
    );
}
#[test]
fn disabled_successor_old_tick_debits_predecessor_lifecycle_credit() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let predecessor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "predecessor_disabled_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(predecessor.binding.clone());
        let mut disabled = successor_policy(&first);
        disabled.charging_mode = ValidationFeeChargingMode::Disabled;
        disabled.fee = Quantity::zero();
        disabled.exemption_classes.clear();
        disabled.treasury_payout_binding = None;
        install_policy_registry_fixture(&policy_registry(&[first.clone(), disabled]), state_tx);
        let credit = lifecycle_credit(&first);
        commit_validation_fee_credit(state_tx, Some(&credit))
            .expect("seed predecessor lifecycle credit");
        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &predecessor,
                code,
                canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64)),
            )
            .expect("retained predecessor payout remains valid after Disabled cutover"),
            OpaqueDeferredValidationOutcome::Apply,
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &credit)
                .expect("read drained predecessor credit"),
            Quantity::zero(),
        );
        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &predecessor,
                code,
                canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64),),
            )
            .expect("a retained lifecycle with zero credit is an atomic no-op"),
            OpaqueDeferredValidationOutcome::NoOp,
        );
    });
}
#[test]
fn different_asset_successor_keeps_predecessor_tick_credit_bound() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let predecessor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "different_asset_predecessor_tick",
        );
        let successor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            1,
            successor_fee_asset(),
            "different_asset_successor_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(predecessor.binding.clone());
        let mut next = successor_policy(&first);
        next.ds_asset_id = successor.binding.ds_asset_id.clone();
        next.treasury_account_id = successor.binding.treasury_account_id.clone();
        next.treasury_payout_binding = Some(successor.binding.clone());
        install_policy_registry_fixture(&policy_registry(&[first.clone(), next.clone()]), state_tx);
        let predecessor_credit = lifecycle_credit(&first);
        let successor_credit = lifecycle_credit(&next);
        commit_validation_fee_credit(state_tx, Some(&predecessor_credit))
            .expect("seed predecessor credit");
        commit_validation_fee_credit(state_tx, Some(&successor_credit))
            .expect("seed successor credit");
        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &predecessor,
                code,
                canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64)),
            )
            .expect("predecessor tick resolves independently of active successor asset"),
            OpaqueDeferredValidationOutcome::Apply,
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &predecessor_credit)
                .expect("read predecessor credit"),
            Quantity::zero(),
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &successor_credit)
                .expect("read isolated successor credit"),
            successor_credit.amount,
        );
    });
}
#[test]
fn same_asset_successor_does_not_strand_predecessor_credit() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let predecessor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "same_asset_predecessor_tick",
        );
        let successor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            1,
            fee_asset(),
            "same_asset_successor_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(predecessor.binding.clone());
        let mut next = successor_policy(&first);
        next.treasury_account_id = successor.binding.treasury_account_id.clone();
        next.treasury_payout_binding = Some(successor.binding.clone());
        install_policy_registry_fixture(&policy_registry(&[first.clone(), next.clone()]), state_tx);
        let predecessor_credit = lifecycle_credit(&first);
        let successor_credit = lifecycle_credit(&next);
        commit_validation_fee_credit(state_tx, Some(&predecessor_credit))
            .expect("seed predecessor credit");
        commit_validation_fee_credit(state_tx, Some(&successor_credit))
            .expect("seed successor credit");
        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &predecessor,
                code,
                canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64)),
            )
            .expect("same-asset predecessor remains bound to its own lifecycle seal"),
            OpaqueDeferredValidationOutcome::Apply,
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &predecessor_credit)
                .expect("read predecessor credit"),
            Quantity::zero(),
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &successor_credit)
                .expect("read successor credit"),
            successor_credit.amount,
        );
    });
}
#[test]
fn predecessor_tick_cannot_spend_beyond_retained_credit_after_cutover() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let predecessor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "bounded_predecessor_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(predecessor.binding.clone());
        let mut disabled = successor_policy(&first);
        disabled.charging_mode = ValidationFeeChargingMode::Disabled;
        disabled.fee = Quantity::zero();
        disabled.exemption_classes.clear();
        disabled.treasury_payout_binding = None;
        install_policy_registry_fixture(&policy_registry(&[first.clone(), disabled]), state_tx);
        let retained = lifecycle_credit(&first)
            .with_amount("3.33".parse().expect("partial retained predecessor credit"));
        commit_validation_fee_credit(state_tx, Some(&retained))
            .expect("seed partial predecessor credit");
        let error = enforce_bound_payout_tick(
            state_tx,
            &predecessor,
            code,
            canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64)),
        )
        .expect_err("a full batch cannot spend beyond retained predecessor credit");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(ref message)
                    ) if message.contains("effect plan") && message.contains("exact bound DS")),
            "unexpected over-credit rejection: {error:?}",
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &retained)
                .expect("read unchanged partial predecessor credit"),
            retained.amount,
        );
    });
}
#[test]
fn future_payout_lifecycle_cannot_preempt_effective_predecessor() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 50;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let runtime = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "future_payout_cutover_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(runtime.binding.clone());
        let mut future_binding = runtime.binding.clone();
        future_binding.pool_vault_account_id = account(7);
        let mut future = successor_policy(&first);
        future.treasury_payout_binding = Some(future_binding);
        assert!(future.effective_from_height > height);
        install_policy_registry_fixture(&policy_registry(&[first.clone(), future]), state_tx);
        let predecessor_credit = lifecycle_credit(&first);
        commit_validation_fee_credit(state_tx, Some(&predecessor_credit))
            .expect("seed effective predecessor credit");

        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &runtime,
                code,
                canonical_treasury_payout_plan(&runtime.binding, Quantity::from(20_u64),),
            )
            .expect("a future lifecycle must not authorize or make its predecessor ambiguous"),
            OpaqueDeferredValidationOutcome::Apply,
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &predecessor_credit)
                .expect("read drained predecessor credit"),
            Quantity::zero(),
        );
    });
}
#[test]
fn ambiguous_payout_runtime_identity_fails_closed() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let runtime = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "ambiguous_payout_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(runtime.binding.clone());
        let mut rebound_binding = runtime.binding.clone();
        rebound_binding.pool_vault_account_id = account(7);
        let mut next = successor_policy(&first);
        next.treasury_payout_binding = Some(rebound_binding);
        install_policy_registry_fixture(&policy_registry(&[first.clone(), next]), state_tx);
        let credit = lifecycle_credit(&first);
        commit_validation_fee_credit(state_tx, Some(&credit))
            .expect("seed unambiguous predecessor credit");
        let error = enforce_bound_payout_tick(
            state_tx,
            &runtime,
            code,
            canonical_treasury_payout_plan(&runtime.binding, Quantity::from(20_u64)),
        )
        .expect_err("one scheduled runtime must not select between two lifecycle seals");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(ref message)
                    ) if message.contains("matches multiple retained lifecycle identities")),
            "unexpected ambiguous-runtime rejection: {error:?}",
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &credit)
                .expect("read unchanged credit after ambiguous runtime"),
            credit.amount,
        );
    });
}
#[test]
fn unrelated_trigger_still_applies_when_fee_policy_disabled() {
    let height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    with_validation_fee_payout_state_at_height(height, |state_tx, deployer, code, code_hash| {
        let predecessor = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            0,
            fee_asset(),
            "disabled_predecessor_tick",
        );
        let unrelated = activate_bound_payout_runtime(
            state_tx,
            deployer,
            code,
            code_hash,
            1,
            fee_asset(),
            "disabled_unrelated_tick",
        );
        let first = policy_with_treasury_payout_lifecycle(predecessor.binding.clone());
        let mut disabled = successor_policy(&first);
        disabled.charging_mode = ValidationFeeChargingMode::Disabled;
        disabled.fee = Quantity::zero();
        disabled.exemption_classes.clear();
        disabled.treasury_payout_binding = None;
        install_policy_registry_fixture(&policy_registry(&[first.clone(), disabled]), state_tx);
        let predecessor_credit = lifecycle_credit(&first);
        commit_validation_fee_credit(state_tx, Some(&predecessor_credit))
            .expect("seed retained predecessor credit");
        assert_eq!(
            enforce_bound_payout_tick(
                state_tx,
                &unrelated,
                code,
                canonical_treasury_payout_plan(&unrelated.binding, Quantity::from(20_u64)),
            )
            .expect("Disabled policy leaves unrelated scheduled runtimes generic"),
            OpaqueDeferredValidationOutcome::Apply,
        );
        let predecessor_plan =
            canonical_treasury_payout_plan(&predecessor.binding, Quantity::from(20_u64));
        let predecessor_ordered =
            ordered_treasury_payout_plan(&predecessor.binding, &predecessor_plan);
        let predecessor_groups = std::collections::BTreeMap::from([(
            predecessor.binding.treasury_account_id.clone(),
            predecessor_plan,
        )]);
        let error = enforce_opaque_deferred_instruction_groups(
            &predecessor_groups,
            &predecessor_ordered,
            state_tx,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &predecessor.runtime,
                code,
                &unrelated.trigger_id,
            )),
        )
        .expect_err("a retained payout runtime paired with another trigger must fail closed");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(ref message)
                    ) if message.contains("scheduled trigger or executed runtime identity matches")
                        && message.contains("pair is not exact")),
            "unexpected cross-wired payout rejection: {error:?}",
        );
        assert_eq!(
            read_validation_fee_credit_balance(state_tx, &predecessor_credit)
                .expect("read untouched predecessor credit"),
            predecessor_credit.amount,
        );
    });
}
#[test]
fn active_policy_admission_rejects_completed_ivm_proved_axt() {
    use iroha_data_model::block::BlockHeader;
    let deployer_key = key_pair(55);
    let deployer = AccountId::new(deployer_key.public_key().clone());
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        validation_fee_payout_world(&deployer),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(TEST_POLICY_EFFECTIVE_HEIGHT)
            .expect("test policy effective height is non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    let policy =
        install_active_bound_validation_fee_policy(&mut state_tx, &deployer, &deployer_key);
    assert_eq!(
        active_policy(&state_tx)
            .expect("active policy lookup succeeds")
            .expect("bound policy is active"),
        policy
    );
    let error = enforce_ivm_proved_completed_axt_admission(1, &state_tx)
        .expect_err("active policy must reject opaque IvmProved AXT effects");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("proof-carrying AXT is disabled")
                    && message.contains("not represented in the signed overlay")),
        "unexpected active-policy AXT rejection: {error:?}"
    );
}
#[test]
fn fee_bearing_transaction_requires_signed_fee_instruction_coordinate() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate)
    );
}
#[test]
fn dangling_fee_batch_entry_coordinate_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let mut metadata = metadata_for(&policy);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
        Json::new(0u64),
    );
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata,
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
    );
}
#[test]
fn non_authority_source_transfer_requires_context_authority_fee() {
    let authority = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let delegated_source = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let missing_fee_tx = tx(
        1,
        vec![transfer(
            &delegated_source,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&missing_fee_tx, &policy),
        Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: 10,
        })
    );
    let exact_fee_tx = tx(
        1,
        vec![
            transfer(
                &delegated_source,
                &fee_asset,
                Quantity::from(1_u64),
                &recipient,
            ),
            transfer(&authority, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&exact_fee_tx, &policy)
        .expect("context authority-paid aggregate fee should validate");
}
#[test]
fn unrelated_treasury_inflow_does_not_inflate_transaction_bound_fee_credit() {
    let user = account(1);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let transaction = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &treasury),
            transfer(
                &user,
                &fee_asset,
                minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                &treasury,
            ),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy_with_credit(&transaction, &policy)
            .expect("principal inflow plus exact coordinate must validate"),
        TEST_VALIDATION_FEE_MINOR_UNITS,
        "only the exact signed fee coordinate becomes spendable fee credit"
    );
}
#[test]
fn underpayment_and_overpayment_are_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    for (observed, expected_error) in [
        (
            9,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units: 9,
            },
        ),
        (
            11,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units: 11,
            },
        ),
    ] {
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(&user, &fee_asset, minor_units(observed), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
    }
}
#[test]
fn duplicate_fee_instructions_are_rejected_as_ambiguous() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(5), &treasury),
            transfer(&user, &fee_asset, minor_units(5), &treasury),
        ],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::DuplicateFeeInstructions { count: 2 })
    );
}
#[test]
fn signed_fee_coordinate_treats_additional_treasury_transfer_as_qualifying() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
            transfer(&user, &fee_asset, Quantity::from(1_u64), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 20,
            observed_minor_units: 10,
        })
    );
}
#[test]
fn wrong_treasury_or_wrong_asset_fee_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let wrong_treasury = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let wrong_treasury_tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &wrong_treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&wrong_treasury_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
            instruction_index: 1,
            entry_index: None,
            expected_account_id: treasury.to_string(),
            observed_account_id: wrong_treasury.to_string(),
        })
    );
    let wrong_asset_tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &xor, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&wrong_asset_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAsset {
            instruction_index: 1,
            entry_index: None,
        })
    );
}
#[test]
fn signed_fee_coordinate_rejects_fee_not_paid_by_transaction_authority() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let sponsor = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&sponsor, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeSource {
            instruction_index: 1,
            entry_index: None,
        })
    );
}
#[test]
fn fee_transfer_is_not_recursively_charged() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let exact_fee_tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&exact_fee_tx, &policy).expect("fee instruction is not recursively charged");
    let recursively_charged_tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(20), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&recursively_charged_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 10,
            observed_minor_units: 20,
        })
    );
}
#[test]
fn retail_transfer_to_treasury_requires_separate_signed_fee() {
    let user = account(1);
    let treasury = account(2);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &treasury),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&tx, &policy)
        .expect("treasury-destination principal requires a separate signed fee instruction");
}
#[test]
fn single_treasury_transfer_cannot_be_signed_as_standalone_fee() {
    let user = account(1);
    let treasury = account(2);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![transfer(&user, &fee_asset, minor_units(10), &treasury)],
        metadata_for_fee_instruction(&policy, 0),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 0,
            observed_minor_units: 10,
        })
    );
}
#[test]
fn treasury_payout_requires_enacted_payout_lifecycle() {
    let user = account(1);
    let treasury = account(2);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let treasury_payout = tx(
        2,
        vec![transfer(
            &treasury,
            &fee_asset,
            Quantity::from(1_u64),
            &user,
        )],
        Metadata::default(),
    );
    assert_eq!(
        enforce_policy(&treasury_payout, &policy),
        Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS
        })
    );
}
#[test]
fn non_exempt_treasury_payout_is_accepted_with_exact_fee() {
    let user = account(1);
    let treasury = account(2);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let treasury_payout = tx(
        2,
        vec![
            transfer(&treasury, &fee_asset, Quantity::from(1_u64), &user),
            transfer(&treasury, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&treasury_payout, &policy)
        .expect("non-exempt treasury payout can pay the exact protocol fee");
}
#[test]
fn ordinary_ds_transfer_from_bound_treasury_remains_fee_bearing() {
    let user = account(1);
    let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
    let treasury = binding.treasury_account_id.clone();
    let policy = policy_with_treasury_payout_lifecycle(binding);
    let fee_asset = policy_fee_asset(&policy);
    let treasury_payout = tx(
        2,
        vec![transfer(
            &treasury,
            &fee_asset,
            Quantity::from(1_u64),
            &user,
        )],
        Metadata::default(),
    );
    assert_eq!(
        enforce_policy(&treasury_payout, &policy),
        Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
        }),
        "the exemption is available only to the exact bound opaque runtime plan",
    );
}
#[test]
fn sub_minor_fee_amount_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(
                &user,
                &fee_asset,
                "0.00001".parse().expect("canonical quantity"),
                &treasury,
            ),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::NonMinorUnitAmount {
            instruction_index: 1,
            scale: 5,
            policy_scale: TEST_VALIDATION_FEE_ASSET_SCALE
        })
    );
}
#[test]
fn policy_version_metadata_is_required_and_exact() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let instructions = || {
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ]
    };
    let missing_metadata_tx = tx(
        1,
        instructions(),
        metadata_for_fee_instruction_coordinate(1),
    );
    assert_eq!(
        enforce_policy(&missing_metadata_tx, &policy),
        Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
    );
    let mut wrong_version = metadata_for_fee_instruction(&policy, 1);
    wrong_version.insert(
        Name::from_str(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY).expect("metadata key"),
        Json::new(policy.policy_version + 1),
    );
    let wrong_version_tx = tx(1, instructions(), wrong_version);
    assert_eq!(
        enforce_policy(&wrong_version_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongPolicyVersionMetadata {
            expected_version: 1,
            observed_version: 2
        })
    );
}
#[test]
fn wrong_policy_hash_metadata_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let mut metadata = metadata_for_fee_instruction(&policy, 1);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
        Json::new(hex::encode([9u8; 32])),
    );
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata,
    );
    assert!(matches!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongPolicyHashMetadata { .. })
    ));
}
#[test]
fn zero_qualifying_transaction_rejects_mismatched_validation_fee_policy_metadata() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let non_fee_asset = asset_definition("xor");
    let mut metadata = metadata_for(&policy);
    let observed_hash_hex = hex::encode([9u8; 32]);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
        Json::new(observed_hash_hex.clone()),
    );
    let tx = tx(
        1,
        vec![transfer(
            &user,
            &non_fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata,
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongPolicyHashMetadata {
            expected_hash_hex: hex::encode(policy.policy_hash().expect("policy hash")),
            observed_hash_hex,
        })
    );
}
#[test]
fn zero_qualifying_transaction_with_fee_coordinate_requires_policy_metadata() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let non_fee_asset = asset_definition("xor");
    let tx = tx(
        1,
        vec![transfer(
            &user,
            &non_fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata_for_fee_instruction_coordinate(0),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
    );
}
#[test]
fn zero_qualifying_transaction_rejects_dangling_fee_entry_coordinate() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let non_fee_asset = asset_definition("xor");
    let mut metadata = metadata_for(&policy);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
        Json::new(0u64),
    );
    let tx = tx(
        1,
        vec![transfer(
            &user,
            &non_fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata,
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
    );
}
#[test]
fn batch_entries_are_charged_per_entry() {
    let user = account(1);
    let recipient_a = account(2);
    let recipient_b = account(3);
    let treasury = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            TransferAssetBatch::new(vec![
                TransferAssetBatchEntry::new(user.clone(), recipient_a, fee_asset.clone(), 1_u64),
                TransferAssetBatchEntry::new(user.clone(), recipient_b, fee_asset.clone(), 1_u64),
                TransferAssetBatchEntry::new(user, treasury, fee_asset, minor_units(20)),
            ])
            .into(),
        ],
        metadata_for_fee_batch_entry(&policy, 0, 2),
    );
    assert_eq!(
        enforce_policy_with_credit(&tx, &policy).expect("batch aggregate fee validates"),
        20,
        "a signed batch credits exactly its aggregate protocol fee"
    );
}
#[test]
fn batch_entries_reject_underpayment_and_overpayment() {
    let user = account(1);
    let recipient_a = account(2);
    let recipient_b = account(3);
    let treasury = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    for (observed, expected_error) in [
        (
            10,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 20,
                observed_minor_units: 10,
            },
        ),
        (
            30,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 20,
                observed_minor_units: 30,
            },
        ),
    ] {
        let tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_a.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        treasury.clone(),
                        fee_asset.clone(),
                        minor_units(observed),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 2),
        );
        assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
    }
}
#[test]
fn batch_fee_coordinate_pointing_at_principal_entry_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            TransferAssetBatch::new(vec![
                TransferAssetBatchEntry::new(
                    user.clone(),
                    recipient.clone(),
                    fee_asset.clone(),
                    1_u64,
                ),
                TransferAssetBatchEntry::new(user, treasury.clone(), fee_asset, minor_units(10)),
            ])
            .into(),
        ],
        metadata_for_fee_batch_entry(&policy, 0, 0),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
            instruction_index: 0,
            entry_index: Some(0),
            expected_account_id: treasury.to_string(),
            observed_account_id: recipient.to_string(),
        })
    );
}
#[test]
fn batch_fee_entry_rejects_wrong_treasury_asset_and_source() {
    let user = account(1);
    let recipient_a = account(2);
    let recipient_b = account(3);
    let treasury = account(4);
    let wrong_treasury = account(5);
    let sponsor = account(6);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let wrong_treasury_tx = tx(
        1,
        vec![
            TransferAssetBatch::new(vec![
                TransferAssetBatchEntry::new(
                    user.clone(),
                    recipient_a.clone(),
                    fee_asset.clone(),
                    1_u64,
                ),
                TransferAssetBatchEntry::new(
                    user.clone(),
                    recipient_b.clone(),
                    fee_asset.clone(),
                    1_u64,
                ),
                TransferAssetBatchEntry::new(
                    user.clone(),
                    wrong_treasury.clone(),
                    fee_asset.clone(),
                    minor_units(20),
                ),
            ])
            .into(),
        ],
        metadata_for_fee_batch_entry(&policy, 0, 2),
    );
    assert_eq!(
        enforce_policy(&wrong_treasury_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
            instruction_index: 0,
            entry_index: Some(2),
            expected_account_id: treasury.to_string(),
            observed_account_id: wrong_treasury.to_string(),
        })
    );
    let wrong_asset_tx = tx(
        1,
        vec![
            TransferAssetBatch::new(vec![
                TransferAssetBatchEntry::new(
                    user.clone(),
                    recipient_a.clone(),
                    fee_asset.clone(),
                    1_u64,
                ),
                TransferAssetBatchEntry::new(
                    user.clone(),
                    recipient_b.clone(),
                    fee_asset.clone(),
                    1_u64,
                ),
                TransferAssetBatchEntry::new(user.clone(), treasury.clone(), xor, minor_units(20)),
            ])
            .into(),
        ],
        metadata_for_fee_batch_entry(&policy, 0, 2),
    );
    assert_eq!(
        enforce_policy(&wrong_asset_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAsset {
            instruction_index: 0,
            entry_index: Some(2),
        })
    );
    let wrong_source_tx = tx(
        1,
        vec![
            TransferAssetBatch::new(vec![
                TransferAssetBatchEntry::new(user.clone(), recipient_a, fee_asset.clone(), 1_u64),
                TransferAssetBatchEntry::new(user, recipient_b, fee_asset.clone(), 1_u64),
                TransferAssetBatchEntry::new(sponsor, treasury.clone(), fee_asset, minor_units(20)),
            ])
            .into(),
        ],
        metadata_for_fee_batch_entry(&policy, 0, 2),
    );
    assert_eq!(
        enforce_policy(&wrong_source_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeSource {
            instruction_index: 0,
            entry_index: Some(2),
        })
    );
}
#[test]
fn multisig_proposal_fee_asset_transfer_requires_context_fee() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let proposal = MultisigPropose::new(
        multisig.clone(),
        vec![transfer(
            &multisig,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        None,
    );
    let missing_fee_tx = tx(1, vec![proposal.into()], metadata_for(&policy));
    assert_eq!(
        enforce_policy(&missing_fee_tx, &policy),
        Err(ValidationFeeAdmissionError::MissingMultisigFeeMarker { context_index: 1 })
    );
    let top_level_fee = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
            Log::new(Level::INFO, "outer index spacer".to_owned()).into(),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 2),
    );
    assert_eq!(
        enforce_policy(&top_level_fee, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 0,
            observed_minor_units: 10
        })
    );
}
#[test]
fn multisig_proposal_context_fee_validates() {
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
        ],
        metadata_for(&policy),
    );
    enforce_policy(&tx, &policy).expect("multisig proposal context fee validates");
}
#[test]
fn multisig_fee_credits_only_when_deferred_instructions_execute() {
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let deferred_instructions = with_multisig_fee_marker(
        &policy,
        vec![
            transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(
                &multisig,
                &fee_asset,
                minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                &treasury,
            ),
        ],
        1,
        None,
    );
    let proposal_transaction = tx(
        1,
        vec![MultisigPropose::new(multisig.clone(), deferred_instructions.clone(), None).into()],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy_with_credit(&proposal_transaction, &policy)
            .expect("signed proposal must validate"),
        0,
        "registering a proposal cannot credit DS that has not moved"
    );
    assert_eq!(
        enforce_deferred_policy_with_credit(&multisig, &deferred_instructions, &policy)
            .expect("executing the stored multisig instructions must validate"),
        TEST_VALIDATION_FEE_MINOR_UNITS,
        "the exact marker-bound fee credits when the proposal actually executes"
    );
}
#[test]
fn multisig_proposal_signed_fee_coordinate_resolves_unique_nested_context() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let nested_proposal = || {
        MultisigPropose::new(
            multisig.clone(),
            with_multisig_fee_marker(
                &policy,
                vec![
                    transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                    transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                ],
                1,
                None,
            ),
            None,
        )
    };
    let exact = tx(
        1,
        vec![nested_proposal().into()],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&exact, &policy)
        .expect("signed fee coordinate should resolve the unique nested proposal context");
    let wrong = tx(
        1,
        vec![nested_proposal().into()],
        metadata_for_fee_instruction(&policy, 0),
    );
    assert_eq!(
        enforce_policy(&wrong, &policy),
        Err(ValidationFeeAdmissionError::ConflictingMultisigFeeCoordinate { context_index: 1 })
    );
    let ambiguous = tx(
        1,
        vec![
            nested_proposal().into(),
            transfer(
                &user,
                &asset_definition("xor"),
                Quantity::from(1_u64),
                &recipient,
            ),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&ambiguous, &policy),
        Err(
            ValidationFeeAdmissionError::AmbiguousFeeInstructionCoordinate {
                instruction_index: 1,
                entry_index: None,
            }
        )
    );
}
#[test]
fn nested_fee_coordinate_does_not_implicitly_designate_top_level_treasury_inflow() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let nested_proposal = MultisigPropose::new(
        multisig.clone(),
        with_multisig_fee_marker(
            &policy,
            vec![
                transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(&multisig, &fee_asset, minor_units(10), &treasury),
            ],
            1,
            None,
        ),
        None,
    );
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            nested_proposal.into(),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate)
    );
}
#[test]
fn multisig_proposal_context_fee_requires_policy_coordinates() {
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
        ],
        Metadata::default(),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
    );
}
#[test]
fn multisig_proposal_context_fee_rejects_wrong_amounts() {
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    for (observed, expected_error) in [
        (
            9,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units: 9,
            },
        ),
        (
            11,
            ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units: 11,
            },
        ),
    ] {
        let tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                            transfer(&multisig, &fee_asset, minor_units(observed), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );
        assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
    }
}
#[test]
fn multisig_proposal_context_fee_rejects_wrong_treasury_asset_and_source() {
    let recipient = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let wrong_treasury = account(5);
    let sponsor = account(6);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let wrong_treasury_tx = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &wrong_treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
        ],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&wrong_treasury_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
            instruction_index: 1,
            entry_index: None,
            expected_account_id: treasury.to_string(),
            observed_account_id: wrong_treasury.to_string(),
        })
    );
    let wrong_asset_tx = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&multisig, &xor, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
        ],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&wrong_asset_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAsset {
            instruction_index: 1,
            entry_index: None,
        })
    );
    let wrong_source_tx = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
                        transfer(&sponsor, &fee_asset, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
            .into(),
        ],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&wrong_source_tx, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeSource {
            instruction_index: 1,
            entry_index: None,
        })
    );
}
#[test]
fn multisig_proposal_batch_entries_are_charged_per_entry() {
    let recipient_a = account(2);
    let recipient_b = account(3);
    let treasury = account(4);
    let multisig = account(5);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let proposal = MultisigPropose::new(
        multisig.clone(),
        with_multisig_fee_marker(
            &policy,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        multisig.clone(),
                        recipient_a,
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        multisig.clone(),
                        recipient_b,
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(multisig, treasury, fee_asset, minor_units(20)),
                ])
                .into(),
            ],
            0,
            Some(2),
        ),
        None,
    );
    let tx = tx(1, vec![proposal.into()], metadata_for(&policy));
    enforce_policy(&tx, &policy).expect("multisig batch aggregate fee validates");
}
