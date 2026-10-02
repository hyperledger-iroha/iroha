// Original signed contract custody at the native fee-reader boundary.
fn signed_fee_registry_fixture() -> (
    crate::sumeragi::test_chain::CertifiedTestChain,
    ValidationFeeTreasuryPayoutBindingV1,
    crate::executor::ContractRuntimeExecutionContext,
    Vec<u8>,
) {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{
        isi::smart_contract_code::{
            CommitContractDeployment, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
        smart_contract::{ContractAddress, ContractAlias, ContractArtifactId},
    };
    use iroha_model_base::topology::DataSpaceId;
    let signer = key_pair(55);
    let authority = AccountId::new(signer.public_key().clone());
    let (code, manifest) = minimal_bound_contract_artifact();
    let code_hash = manifest
        .code_hash
        .expect("verified artifact has a code hash");
    let artifact_id = ContractArtifactId::new(DataSpaceId::UNIVERSAL, code_hash);
    let mut config = TestChainConfig::new(validation_fee_payout_world(&authority), 1_000);
    config.genesis_key = signer.clone();
    config.genesis_instructions = vec![
        RegisterSmartContractBytes {
            artifact_id,
            code: code.clone(),
        }
        .into(),
        RegisterSmartContractCode {
            artifact_id,
            manifest: manifest.signed(&signer),
        }
        .into(),
    ];
    for scope in [
        iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
            DataSpaceId::UNIVERSAL,
        ),
        iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(
            DomainId::try_new("contracts", "universal").unwrap(),
        ),
    ] {
        let permission: iroha_data_model::permission::Permission =
            iroha_executor_data_model::permission::account::CanManageAccountAlias { scope }.into();
        config.genesis_instructions.push(
            iroha_data_model::isi::Grant::account_permission(permission, authority.clone()).into(),
        );
    }
    let mut chain = CertifiedTestChain::start(config).expect("original signed root genesis");
    let address =
        ContractAddress::derive(&chain.network_id(), &authority, 0, DataSpaceId::UNIVERSAL)
            .unwrap();
    let alias =
        ContractAlias::from_components("original-payout", Some("contracts"), "universal").unwrap();
    let deploy = chain.sign(
        &signer,
        [CommitContractDeployment {
            expected_deploy_nonce: 0,
            contract_address: address.clone(),
            code_hash,
            contract_alias: alias.clone(),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into()],
        2_000,
    );
    assert_eq!(
        chain.commit(vec![deploy]),
        vec![true],
        "actual signed deployment must publish successfully: {:?}",
        chain.committed(2).block().execution_outputs()
    );
    let binding = treasury_payout_binding(address.clone(), &code);
    let runtime = crate::executor::ContractRuntimeExecutionContext {
        contract_address: address,
        contract_subject: binding.treasury_account_id.clone(),
        contract_alias: Some(alias),
        entrypoint: binding.entrypoint.to_string(),
    };
    (chain, binding, runtime, code)
}

#[test]
fn signed_fee_subject_read_keeps_scope_refusal_distinct_from_inactive_treasury() {
    let (chain, binding, _, _) = signed_fee_registry_fixture();
    let view = chain.state().view();
    let run = || {
        validate_treasury_payout_binding_contract_subject(
            &binding,
            TEST_VALIDATION_FEE_ASSET_SCALE,
            &view,
        )
    };
    run().expect("the exact original signed deployment owns the active treasury subject");
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
        run,
    );
    assert!(
        matches!(refused, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(ref reason)) if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
        "local scope refusal became a completed inactive-subject verdict: {refused:?}"
    );
    run().expect("the original treasury record retries without changing its binding");
    assert_eq!(view.height(), 2);
    assert_eq!(chain.kura().blocks_count(), 2);
}

#[test]
fn signed_fee_runtime_read_does_not_turn_scope_refusal_into_a_nonmatching_origin() {
    let (chain, binding, runtime, code) = signed_fee_registry_fixture();
    let state = std::sync::Arc::clone(chain.state());
    let previous = state.view().latest_block_hash();
    let mut block = state.block(BlockHeader::new(
        3_u64.try_into().unwrap(),
        previous,
        None,
        3_000,
        0,
    ));
    let transaction = block.transaction();
    let origin = OpaqueDeferredRuntimeOrigin::new(&runtime, &code);
    let run = || runtime_origin_matches_payout_binding(&binding, &transaction, &origin);
    assert!(
        run().expect("complete original scope read"),
        "original deployed code and runtime match the exact payout binding"
    );
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
        run,
    );
    assert!(
        matches!(refused, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(ref reason)) if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
        "local scope refusal became a completed nonmatching runtime verdict"
    );
    assert!(
        run().expect("complete original scope read"),
        "the original runtime retries without replacing code or subject"
    );
    assert_eq!(state.view().height(), 2);
    assert_eq!(chain.kura().blocks_count(), 2);
}

/// Two genuinely signed deployments used by the Parliament reader regression.
pub(crate) fn signed_payout_lifecycle_registry_fixture() -> (
    crate::sumeragi::test_chain::CertifiedTestChain,
    ValidationFeeTreasuryPayoutBindingV1,
) {
    use iroha_data_model::{
        isi::smart_contract_code::CommitContractDeployment,
        smart_contract::{ContractAddress, ContractAlias},
    };
    let (mut chain, mut binding, _, code) = signed_fee_registry_fixture();
    let signer = key_pair(55);
    let authority = AccountId::new(signer.public_key().clone());
    let pool = ContractAddress::derive(
        &chain.network_id(),
        &authority,
        1,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    let deploy = chain.sign(
        &signer,
        [CommitContractDeployment {
            expected_deploy_nonce: 1,
            contract_address: pool.clone(),
            code_hash: ivm::contract_code_hash(&code),
            contract_alias: ContractAlias::from_components(
                "original-pool",
                Some("contracts"),
                "universal",
            )
            .unwrap(),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into()],
        3_000,
    );
    assert_eq!(chain.commit(vec![deploy]), vec![true]);
    binding.pool_vault_account_id = pool.subject_id();
    binding.min_xor_out = iroha_data_model::validation_fee::validation_fee_payout_min_xor();
    binding.max_xor_out = iroha_data_model::validation_fee::validation_fee_payout_max_xor();
    assert_eq!(binding.invariant_error(), None);
    (chain, binding)
}

#[test]
fn original_retained_fee_registry_does_not_publish_local_decode_refusal_as_malformed() {
    let (chain, binding, _, _) = signed_fee_registry_fixture();
    let mut policy = policy_with_treasury_payout_lifecycle(binding);
    policy.network_id = chain.network_id();
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = std::sync::Arc::clone(chain.state());
    let mut block = state.block(BlockHeader::new(
        3_u64.try_into().unwrap(),
        state.view().latest_block_hash(),
        None,
        3_000,
        0,
    ));
    let mut transaction = block.transaction();
    install_policy_registry_fixture(&registry, &mut transaction);
    validated_policy_registry_in_world(&transaction.world)
        .expect("exact retained registry and governance owners");
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
        || validated_policy_registry_in_world(&transaction.world),
    );
    assert!(
        matches!(refused, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(ref reason))
        if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
        "original registry decoder refusal must remain local: {refused:?}"
    );
    let restore_refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
        || {
            validate_persisted_policy_registry_runtime_v1(
                &transaction,
                policy.effective_from_height,
            )
        },
    );
    assert!(
        matches!(restore_refused, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(ref reason))
        if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity)
    );
    validated_policy_registry_in_world(&transaction.world)
        .expect("retry original retained registry");
    validate_persisted_policy_registry_runtime_v1(&transaction, policy.effective_from_height)
        .expect("original active runtime and exact native network");
}

#[test]
fn retained_registry_refusal_blocks_fee_effect_publication_and_original_retry() {
    let (chain, binding, _, _) = signed_fee_registry_fixture();
    let mut policy = policy_with_treasury_payout_lifecycle(binding);
    policy.network_id = chain.network_id();
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = std::sync::Arc::clone(chain.state());
    let header = BlockHeader::new(
        3_u64.try_into().unwrap(),
        state.view().latest_block_hash(),
        None,
        3_000,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    install_policy_registry_fixture(&registry, &mut transaction);
    let groups = std::collections::BTreeMap::new();
    enforce_opaque_deferred_instruction_groups(&groups, &[], &mut transaction, None)
        .expect("the original future policy completes admission without charging");
    assert!(transaction.execution_deferral().is_none());
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
        || enforce_opaque_deferred_instruction_groups(&groups, &[], &mut transaction, None),
    );
    assert!(refused.is_err());
    assert_eq!(
        transaction.execution_deferral().unwrap().reason(),
        ivm::error::ExecutionDeferral::ActiveMemoryCapacity
    );
    transaction.apply();
    assert!(
        validated_policy_registry_in_world(&block.world)
            .unwrap()
            .is_none(),
        "the refused transaction must not apply its staged registry"
    );
    // A declined transaction application poisons its output plan. Use the actual
    // publication seam; the World-only fixture installer intentionally has no output plan.
    assert!(matches!(
        block.commit(),
        Err(crate::state::storage_transactions::TransactionsBlockError::ExecutionOutputCapacity)
    ));
    assert_eq!(state.view().height(), 2);
    assert_eq!(chain.kura().blocks_count(), 2);
    let mut block = state.block(header);
    let mut retry = block.transaction();
    install_policy_registry_fixture(&registry, &mut retry);
    enforce_opaque_deferred_instruction_groups(&groups, &[], &mut retry, None).unwrap();
    assert!(retry.execution_deferral().is_none());
}
