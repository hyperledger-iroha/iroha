fn contract_deployment_permission() -> Permission {
    executor_permission::smart_contract::CanRegisterSmartContractCode.into()
}

fn bundled_default_user_provided_executor() -> super::Executor {
    let raw_executor = data_model_executor::Executor::new(IvmBytecode::from_compiled(
        include_bytes!("../../../defaults/executor.to").to_vec(),
    ));
    super::Executor::UserProvided(
        super::LoadedExecutor::load(raw_executor).expect("load bundled default executor"),
    )
}

fn contract_upload_instruction(code_hash: Hash, chunk_index: u32) -> InstructionBox {
    UploadSmartContractCodeChunk {
        code_hash,
        total_size: if chunk_index == 0 { 1 } else { 65_537 },
        chunk_index,
        chunk_count: if chunk_index == 0 { 1 } else { 2 },
        chunk: vec![0xA5],
    }
    .into()
}

fn contract_deployment_bootstrap_instructions(
    authority: &AccountId,
    account: iroha_data_model::account::NewAccount,
    permission: Permission,
    deployment: InstructionBox,
) -> Vec<InstructionBox> {
    vec![
        Register::account(account).into(),
        Grant::account_permission(permission, authority.clone()).into(),
        deployment,
    ]
}

#[test]
#[allow(clippy::too_many_lines)]
fn contract_deployment_bootstrap_recognizer_is_exact_and_plain_only() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-shape");
    let code_hash = Hash::new(b"contract deployment bootstrap shape");
    let world = World::new();

    let sign = |instructions: Vec<InstructionBox>| {
        TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(instructions.into()))
        .sign(keypair.private_key())
    };
    let exact = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    let exact_transaction = sign(exact.clone());
    assert!(allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &exact_transaction
    ));
    let authorization = ContractDeploymentSelfBootstrapAuthorization::derive(
        &world.view(),
        &authority,
        &exact_transaction,
    )
    .expect("exact signed prefix derives scoped authorization");
    authorization
        .validate_instruction_sequence(&authority, &exact)
        .expect("exact signed sequence remains authorized");
    let mut divergent_overlay = exact.clone();
    divergent_overlay.push(Log::new(Level::INFO, "unsigned divergence".to_owned()).into());
    assert!(
        authorization
            .validate_instruction_sequence(&authority, &divergent_overlay)
            .is_err(),
        "authorization must bind the complete signed instruction sequence"
    );

    let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
        seiyaku_name: None,
        code_hash: Some(code_hash),
        abi_hash: Some(Hash::new(b"contract deployment bootstrap manifest ABI")),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        states: None,
        error_codes: None,
        kotoba: None,
        provenance: None,
    };
    let manifest_bootstrap = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        contract_deployment_permission(),
        RegisterSmartContractCode { manifest }.into(),
    );
    assert!(allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(manifest_bootstrap)
    ));

    let other = checked_account_id();
    assert!(
        ContractDeploymentSelfBootstrapAuthorization::derive(
            &world.view(),
            &other,
            &exact_transaction,
        )
        .is_none(),
        "derivation must bind the signed transaction authority"
    );
    let wrong_account = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(other.clone()),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(wrong_account)
    ));
    let wrong_destination = vec![
        Register::account(Account::new(authority.clone())).into(),
        Grant::account_permission(contract_deployment_permission(), other).into(),
        contract_upload_instruction(code_hash, 0),
    ];
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(wrong_destination)
    ));

    let mut metadata = Metadata::default();
    metadata.insert(
        "bootstrap-note".parse().expect("metadata key"),
        Json::new("x"),
    );
    let decorated = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()).with_metadata(metadata),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(decorated)
    ));

    let malformed_permission = Permission::new(
        "CanRegisterSmartContractCode".to_owned(),
        Json::from(norito::json!({ "unexpected": true })),
    );
    let malformed = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        malformed_permission,
        contract_upload_instruction(code_hash, 0),
    );
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(malformed)
    ));

    let non_initial_chunk = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 1),
    );
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(non_initial_chunk)
    ));

    let mut shifted = exact.clone();
    shifted.insert(
        0,
        Log::new(Level::INFO, "shifted bootstrap".to_owned()).into(),
    );
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &sign(shifted)
    ));

    let mut atomic_deployment = exact.clone();
    atomic_deployment.push(
        iroha_data_model::isi::smart_contract_code::CommitContractDeployment {
            expected_deploy_nonce: 0,
            contract_address: ContractAddress::derive(
                &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
                &authority,
                0,
                DataSpaceId::UNIVERSAL,
            )
            .expect("atomic deployment address"),
            code_hash,
            contract_alias: "payments::universal".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into(),
    );
    assert!(
        !allows_contract_deployment_self_bootstrap(
            &world.view(),
            &authority,
            &sign(atomic_deployment)
        ),
        "atomic deployment must require an authority that existed before the transaction"
    );

    let proved_transaction = TransactionBuilder::new(
        chain,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x00]),
            overlay: exact.into(),
            events_commitment: Hash::new(b"bootstrap proved events"),
            gas_policy_commitment: Hash::new(b"bootstrap proved gas"),
        },
    ))
    .sign(keypair.private_key());
    assert!(!allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &proved_transaction
    ));

    let existing_world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    assert!(!allows_contract_deployment_self_bootstrap(
        &existing_world.view(),
        &authority,
        &exact_transaction
    ));
}

#[test]
fn initial_executor_bootstraps_missing_deployment_authority_and_meters_grant() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-missing");
    let code_hash = Hash::new(b"contract deployment bootstrap missing authority");
    let instructions = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    let expected_gas = crate::gas::meter_instructions(&instructions);
    let transaction = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(instructions.into()))
    .sign(keypair.private_key());
    let world = World::new();
    assert!(allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &transaction
    ));
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        chain,
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut state_transaction = block.transaction();
    let mut ivm_cache = IvmCache::new();
    assert!(
        !(state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()),
        "bootstrap exception must be exercised outside genesis"
    );

    super::Executor::Initial
        .execute_transaction(
            &mut state_transaction,
            &authority,
            transaction,
            &mut ivm_cache,
        )
        .expect("exact missing-authority bootstrap must execute");
    assert_eq!(state_transaction.last_tx_gas_used, expected_gas);
    state_transaction.apply();

    block
        .world
        .account(&authority)
        .expect("bootstrap account must be registered");
    assert!(
        block
            .world
            .account_permissions_iter(&authority)
            .expect("bootstrap account permissions")
            .any(|permission| permission == &contract_deployment_permission())
    );
    let progress = block
        .world
        .contract_code_upload_progress(&authority, &code_hash)
        .expect("first upload chunk must be staged");
    assert_eq!(progress.descriptor.total_size, 1);
    assert_eq!(progress.descriptor.chunk_count, 1);
    assert_eq!(progress.received_chunks, 1);
}

#[test]
fn default_user_provided_executor_bootstraps_missing_deployment_authority() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-user-provided");
    let code_hash = Hash::new(b"default user-provided deployment bootstrap");
    let instructions = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    let transaction = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(instructions.into()))
    .sign(keypair.private_key());
    let world = World::new();
    assert!(allows_contract_deployment_self_bootstrap(
        &world.view(),
        &authority,
        &transaction
    ));

    let executor = bundled_default_user_provided_executor();
    let super::Executor::UserProvided(loaded_executor) = &executor else {
        unreachable!("test constructs a user-provided executor")
    };
    let (runtime_stats_before, _) = loaded_executor.runtime_pool_snapshot();

    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        chain,
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut state_transaction = block.transaction();
    let mut ivm_cache = IvmCache::new();
    assert!(
        !(state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()),
        "user-provided bootstrap must be exercised outside genesis"
    );

    executor
        .execute_transaction(
            &mut state_transaction,
            &authority,
            transaction,
            &mut ivm_cache,
        )
        .expect("bundled default executor must admit exact missing-authority bootstrap");
    let (runtime_stats_after, _) = loaded_executor.runtime_pool_snapshot();
    assert_eq!(
        runtime_stats_after.hits + runtime_stats_after.misses,
        runtime_stats_before.hits + runtime_stats_before.misses + 2,
        "only register and upload may enter the user-provided runtime; the exact grant is applied directly by Core"
    );
    assert_eq!(
        runtime_stats_after.dirty_resets,
        runtime_stats_before.dirty_resets + 2
    );
    state_transaction.apply();

    block
        .world
        .account(&authority)
        .expect("bootstrap account must be registered");
    let expected_permission = contract_deployment_permission();
    assert!(
        block
            .world
            .account_permissions_iter(&authority)
            .expect("bootstrap account permissions")
            .any(|permission| permission == &expected_permission)
    );
    let progress = block
        .world
        .contract_code_upload_progress(&authority, &code_hash)
        .expect("first upload chunk must be staged");
    assert_eq!(progress.descriptor.total_size, 1);
    assert_eq!(progress.descriptor.chunk_count, 1);
    assert_eq!(progress.received_chunks, 1);
}

#[test]
fn default_user_provided_executor_rejects_existing_bootstrap_before_grant_dispatch() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-user-provided-replay");
    let code_hash = Hash::new(b"default user-provided deployment bootstrap replay");
    let transaction = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(
        contract_deployment_bootstrap_instructions(
            &authority,
            Account::new(authority.clone()),
            contract_deployment_permission(),
            contract_upload_instruction(code_hash, 0),
        )
        .into(),
    ))
    .sign(keypair.private_key());
    let account = Account::new(authority.clone()).build(&authority);
    let state = State::new_with_chain(
        World::with([], [account], []),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        chain,
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    assert!(!allows_contract_deployment_self_bootstrap(
        &block.world,
        &authority,
        &transaction
    ));

    let executor = bundled_default_user_provided_executor();
    let super::Executor::UserProvided(loaded_executor) = &executor else {
        unreachable!("test constructs a user-provided executor")
    };
    let (runtime_stats_before, _) = loaded_executor.runtime_pool_snapshot();
    let error = {
        let mut state_transaction = block.transaction();
        let mut ivm_cache = IvmCache::new();
        executor
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction,
                &mut ivm_cache,
            )
            .expect_err("an existing authority cannot replay the bootstrap prefix")
    };
    assert!(matches!(error, ValidationFail::NotPermitted(message) if
            message.contains("CanRegisterSmartContractCode")
                && message.contains("genesis block")));
    let (runtime_stats_after, _) = loaded_executor.runtime_pool_snapshot();
    assert_eq!(
        runtime_stats_after.hits + runtime_stats_after.misses,
        runtime_stats_before.hits + runtime_stats_before.misses + 1,
        "only the idempotent account registration may reach the runtime before Core rejects the grant"
    );

    block
        .world
        .account(&authority)
        .expect("pre-existing account must remain present");
    assert!(
        !block
            .world
            .account_permissions_iter(&authority)
            .expect("pre-existing account permissions")
            .any(|permission| permission.name() == "CanRegisterSmartContractCode")
    );
    assert!(
        block
            .world
            .contract_code_upload_progress(&authority, &code_hash)
            .is_none()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn default_user_provided_executor_rejects_noncanonical_bootstrap_without_committing_state() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-user-provided-adversarial");
    let code_hash = Hash::new(b"default user-provided adversarial deployment bootstrap");

    let mut metadata = Metadata::default();
    metadata.insert(
        "bootstrap-note".parse().expect("metadata key"),
        Json::new("decorated"),
    );
    let decorated = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()).with_metadata(metadata),
        contract_deployment_permission(),
        contract_upload_instruction(code_hash, 0),
    );
    let malformed = contract_deployment_bootstrap_instructions(
        &authority,
        Account::new(authority.clone()),
        Permission::new(
            "CanRegisterSmartContractCode".to_owned(),
            Json::from(norito::json!({ "scope": "malformed" })),
        ),
        contract_upload_instruction(code_hash, 0),
    );
    let reordered = vec![
        Register::account(Account::new(authority.clone())).into(),
        contract_upload_instruction(code_hash, 0),
        Grant::account_permission(contract_deployment_permission(), authority.clone()).into(),
    ];

    for (label, instructions, expected_runtime_checkouts) in [
        ("decorated registration", decorated, 1),
        ("malformed same-name grant", malformed, 1),
        ("reordered prefix", reordered, 2),
    ] {
        let transaction = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(instructions.into()))
        .sign(keypair.private_key());
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
            chain.clone(),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        assert!(
            !allows_contract_deployment_self_bootstrap(&block.world, &authority, &transaction),
            "{label} must not qualify for the bootstrap exception"
        );

        let executor = bundled_default_user_provided_executor();
        let super::Executor::UserProvided(loaded_executor) = &executor else {
            unreachable!("test constructs a user-provided executor")
        };
        let (runtime_stats_before, _) = loaded_executor.runtime_pool_snapshot();
        let error = {
            let mut state_transaction = block.transaction();
            let mut ivm_cache = IvmCache::new();
            executor
                .execute_transaction(
                    &mut state_transaction,
                    &authority,
                    transaction,
                    &mut ivm_cache,
                )
                .expect_err("noncanonical bootstrap must be rejected")
        };
        let error_debug = format!("{error:?}");
        assert!(
            error_debug.contains("CanRegisterSmartContractCode"),
            "unexpected {label} rejection: {error_debug}"
        );
        let (runtime_stats_after, _) = loaded_executor.runtime_pool_snapshot();
        assert_eq!(
            runtime_stats_after.hits + runtime_stats_after.misses,
            runtime_stats_before.hits + runtime_stats_before.misses + expected_runtime_checkouts,
            "unexpected user-provided runtime dispatch count for {label}"
        );

        assert!(
            block.world.account(&authority).is_err(),
            "rejected {label} must not commit its provisional account"
        );
        assert!(
            block.world.account_permissions.get(&authority).is_none(),
            "rejected {label} must not commit a permission"
        );
        assert!(
            block
                .world
                .contract_code_upload_progress(&authority, &code_hash)
                .is_none(),
            "rejected {label} must not commit upload staging"
        );
    }
}

#[test]
fn user_provided_borrowed_overlay_rejects_deployment_permission_before_runtime_dispatch() {
    let authority = checked_account_id();
    let account = Account::new(authority.clone()).build(&authority);
    let state = State::new_for_testing(
        World::with([], [account], []),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut state_transaction = block.transaction();
    let instruction: InstructionBox =
        Grant::account_permission(contract_deployment_permission(), authority.clone()).into();
    let executor = bundled_default_user_provided_executor();
    let super::Executor::UserProvided(loaded_executor) = &executor else {
        unreachable!("test constructs a user-provided executor")
    };
    let (runtime_stats_before, _) = loaded_executor.runtime_pool_snapshot();

    let error = executor
        .execute_borrowed_overlay_instruction(
            &mut state_transaction,
            &authority,
            &instruction,
            None,
        )
        .expect_err("borrowed overlay permission mutation must be consensus-gated");
    assert!(matches!(error, ValidationFail::NotPermitted(message) if
            message.contains("CanRegisterSmartContractCode")
                && message.contains("genesis block")));
    let (runtime_stats_after, _) = loaded_executor.runtime_pool_snapshot();
    assert_eq!(runtime_stats_after, runtime_stats_before);
    assert!(
        !state_transaction
            .world
            .account_permissions_iter(&authority)
            .expect("account permissions")
            .any(|permission| permission.name() == "CanRegisterSmartContractCode")
    );
}

#[test]
fn initial_executor_denies_preexisting_deployment_self_grant_without_state_change() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = ChainId::from("contract-deployment-bootstrap-existing");
    let code_hash = Hash::new(b"contract deployment bootstrap existing authority");
    let account = Account::new(authority.clone()).build(&authority);
    let state = State::new_with_chain(
        World::with([], [account], []),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        chain.clone(),
    );
    let transaction = TransactionBuilder::new(
        chain,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(
        contract_deployment_bootstrap_instructions(
            &authority,
            Account::new(authority.clone()),
            contract_deployment_permission(),
            contract_upload_instruction(code_hash, 0),
        )
        .into(),
    ))
    .sign(keypair.private_key());
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    assert!(!allows_contract_deployment_self_bootstrap(
        &block.world,
        &authority,
        &transaction
    ));
    let mut state_transaction = block.transaction();
    let mut ivm_cache = IvmCache::new();
    assert!(
        !(state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()),
        "bootstrap replay must be exercised outside genesis"
    );

    let error = super::Executor::Initial
        .execute_transaction(
            &mut state_transaction,
            &authority,
            transaction,
            &mut ivm_cache,
        )
        .expect_err("an existing authority cannot replay the bootstrap prefix");
    assert!(matches!(error, ValidationFail::NotPermitted(message) if
            message.contains("only allowed inside the genesis block")));
    assert!(
        !state_transaction
            .world
            .account_permissions_iter(&authority)
            .expect("existing account permissions")
            .any(|permission| permission.name() == "CanRegisterSmartContractCode")
    );
    assert!(
        state_transaction
            .world
            .contract_code_upload_progress(&authority, &code_hash)
            .is_none()
    );
}

#[test]
fn initial_executor_denies_deployment_permission_grant_revoke_and_malformed_payload() {
    let authority = checked_account_id();
    let canonical = contract_deployment_permission();
    let account = Account::new(authority.clone()).build(&authority);
    let mut world = World::with([], [account], []);
    world
        .account_permissions
        .insert(authority.clone(), BTreeSet::from([canonical.clone()]));
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut state_transaction = block.transaction();
    assert!(
        !(state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()),
        "permission parity must be exercised outside genesis"
    );
    let malformed = Permission::new(
        "CanRegisterSmartContractCode".to_owned(),
        Json::from(norito::json!({ "scope": "not-canonical" })),
    );
    let role_id: RoleId = "deployment_bootstrap_role".parse().expect("role id");

    for instruction in [
        Grant::account_permission(canonical.clone(), authority.clone()).into(),
        Grant::account_permission(malformed, authority.clone()).into(),
        Revoke::account_permission(canonical.clone(), authority.clone()).into(),
        Grant::role_permission(canonical.clone(), role_id.clone()).into(),
        Revoke::role_permission(canonical.clone(), role_id.clone()).into(),
        concrete_instruction_box!(
            Grant<Permission, Account>,
            Grant::account_permission(canonical.clone(), authority.clone())
        ),
        concrete_instruction_box!(
            Revoke<Permission, Role>,
            Revoke::role_permission(canonical.clone(), role_id)
        ),
    ] {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction)
            .expect_err("deployment permission mutation must remain genesis-only");
        assert!(matches!(error, ValidationFail::NotPermitted(message) if
                message.contains("only allowed inside the genesis block")));
    }

    let stored: BTreeSet<_> = state_transaction
        .world
        .account_permissions_iter(&authority)
        .expect("account permissions")
        .cloned()
        .collect();
    assert_eq!(stored, BTreeSet::from([canonical]));
}

#[test]
fn initial_executor_denies_post_genesis_governed_offline_self_grants() {
    let authority = checked_account_id();
    let account = Account::new(authority.clone()).build(&authority);
    let state = State::new_for_testing(
        World::with([], [account], []),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut state_transaction = block.transaction();

    for name in [
        "CanManageOfflineEscrow",
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
    ] {
        let permission = Permission::new(name.to_owned(), Json::new(()));
        let instruction = Grant::account_permission(permission.clone(), authority.clone()).into();
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction)
            .expect_err("an unprivileged account must not self-grant governed offline power");
        assert!(
            matches!(error, ValidationFail::NotPermitted(_)),
            "unexpected {name} self-grant rejection: {error:?}",
        );
        assert!(
            !state_transaction
                .world
                .account_permissions_iter(&authority)
                .expect("authority permissions")
                .any(|stored| stored == &permission),
            "rejected {name} self-grant must not mutate world state",
        );
    }
}
