// Same-scope regression coverage extracted to keep the parent source budget bounded.

#[test]
fn validation_fee_activation_delay_enforces_exact_boundary_and_overflow() {
    let enacted_at_height = 40;
    let minimum = enacted_at_height
        + iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
    assert!(
        super::ensure_validation_fee_policy_activation_delay(
            minimum.saturating_sub(1),
            enacted_at_height,
        )
        .is_err()
    );
    assert_eq!(
        super::ensure_validation_fee_policy_activation_delay(minimum, enacted_at_height)
            .expect("exact activation boundary"),
        minimum
    );
    assert!(
        super::ensure_validation_fee_policy_activation_delay(
            minimum.saturating_add(1),
            enacted_at_height,
        )
        .is_err(),
        "late activation must not weaken the exact 120,960-block relation"
    );
    assert!(
        super::ensure_validation_fee_policy_activation_delay(
            u64::MAX,
            u64::MAX
                - iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS
                + 1,
        )
        .is_err()
    );
}

#[test]
fn contract_subject_binding_materializes_missing_account_and_preserves_existing_account() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("seed lifecycle authority");

    let missing_address = ContractAddress::derive(
        &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
        &ALICE_ID,
        41,
        DataSpaceId::UNIVERSAL,
    )
    .expect("missing-subject contract address");
    let missing_subject = missing_address.subject_id();
    assert!(state_transaction.world.account(&missing_subject).is_err());

    let bound_subject = super::ensure_contract_subject_binding(
        &ALICE_ID,
        &mut state_transaction,
        &missing_address,
    )
    .expect("bind and materialize missing contract subject");
    assert_eq!(bound_subject, missing_subject);
    assert!(state_transaction.world.account(&missing_subject).is_ok());
    assert!(crate::smartcontracts::code::is_historical_contract_subject(
        &state_transaction.world,
        &missing_subject,
    ));

    let existing_address = ContractAddress::derive(
        &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
        &ALICE_ID,
        42,
        DataSpaceId::UNIVERSAL,
    )
    .expect("existing-subject contract address");
    let existing_subject = existing_address.subject_id();
    let marker: Name = "contract_subject_marker".parse().expect("metadata key");
    let mut metadata = Metadata::default();
    metadata.insert(marker.clone(), Json::new("preserve-me"));
    Register::account(Account::new(existing_subject.clone()).with_metadata(metadata.clone()))
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("seed existing contract subject account");

    let bound_existing = super::ensure_contract_subject_binding(
        &ALICE_ID,
        &mut state_transaction,
        &existing_address,
    )
    .expect("bind existing contract subject without replacing it");
    assert_eq!(bound_existing, existing_subject);
    assert_eq!(
        state_transaction
            .world
            .account(&existing_subject)
            .expect("existing subject remains registered")
            .metadata()
            .get(&marker),
        metadata.get(&marker),
        "binding must not replace or repair an existing subject account",
    );
}

#[test]
fn upgrade_execute_enforces_capability_at_the_mutation_boundary() {
    use iroha_data_model::permission::Permissions;
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;

    fn invalid_upgrade() -> iroha_data_model::isi::Upgrade {
        iroha_data_model::isi::Upgrade::new(iroha_data_model::executor::Executor::new(
            IvmBytecode::from_compiled(Vec::new()),
        ))
    }

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(
        NonZeroU64::new(2).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();

    let error = invalid_upgrade()
        .execute(&ALICE_ID, &mut state_transaction)
        .expect_err("direct native dispatch must not bypass executor-upgrade authority");
    assert!(
        matches!(error, InstructionExecutionError::InvariantViolation(ref message)
            if message.as_ref().contains("CanUpgradeExecutor")),
        "unexpected upgrade denial: {error:?}"
    );

    state_transaction.world.account_permissions.insert(
        ALICE_ID.clone(),
        Permissions::from([Permission::from(CanUpgradeExecutor)]),
    );
    let error = invalid_upgrade()
        .execute(&ALICE_ID, &mut state_transaction)
        .expect_err("the intentionally empty executor bytecode must fail migration");
    assert!(
        !matches!(error, InstructionExecutionError::InvariantViolation(ref message)
            if message.as_ref().contains("CanUpgradeExecutor")),
        "an exact capability holder must reach migration: {error:?}"
    );
}

#[test]
fn validation_fee_derived_runtime_permission_rejects_preexisting_direct_and_role_holders() {
    use iroha_data_model::permission::Permissions;
    use iroha_executor_data_model::permission::asset::CanTransferAsset;

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    let permission: Permission = CanTransferAsset {
        asset: AssetId::new(asset_definition_id, ALICE_ID.clone()),
    }
    .into();

    super::require_absent_validation_fee_runtime_permission(
        &stx,
        &permission,
        "the wrapper SBD asset transfer effect",
    )
    .expect("an absent effect permission is eligible for protected derivation");

    stx.world
        .account_permissions
        .insert(BOB_ID.clone(), Permissions::from([permission.clone()]));
    let direct_error = super::require_absent_validation_fee_runtime_permission(
        &stx,
        &permission,
        "the wrapper SBD asset transfer effect",
    )
    .expect_err("a caller-made direct effect grant must fail closed");
    assert!(
        format!("{direct_error:?}").contains("absent before enactment"),
        "unexpected direct-holder error: {direct_error:?}"
    );
    stx.world.account_permissions.remove(BOB_ID.clone());

    let role_id: RoleId = "validation_fee_effect_holder".parse().expect("role id");
    let role = Role::new(role_id.clone(), ALICE_ID.clone())
        .add_permission(permission.clone())
        .build(&ALICE_ID);
    stx.world.roles.insert(role_id, role);
    let role_error = super::require_absent_validation_fee_runtime_permission(
        &stx,
        &permission,
        "the wrapper SBD asset transfer effect",
    )
    .expect_err("a role-owned effect grant must fail closed");
    assert!(
        format!("{role_error:?}").contains("forbids role ownership"),
        "unexpected role-holder error: {role_error:?}"
    );
    assert!(
        stx.world
            .account_permissions
            .iter()
            .all(|(_, permissions)| !permissions.contains(&permission)),
        "a failed derivation preflight must leave no direct effect token"
    );
}

#[test]
fn validation_fee_derived_runtime_permissions_roll_back_atomically() {
    use iroha_executor_data_model::permission::asset::CanTransferAsset;
    use iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint;

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    let effect_permission: Permission = CanTransferAsset {
        asset: AssetId::new(asset_definition_id, ALICE_ID.clone()),
    }
    .into();
    let contract_address: iroha_data_model::smart_contract::ContractAddress =
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
            .parse()
            .expect("canonical contract address");
    let wrapper_permission: Permission = CanInvokeContractEntrypoint {
        contract: contract_address.clone(),
        entrypoint: "autonomous_validation_fee_tick".to_owned(),
    }
    .into();
    let pool_permission: Permission = CanInvokeContractEntrypoint {
        contract: contract_address,
        entrypoint: "swap_exact_in_quote_public".to_owned(),
    }
    .into();
    let permissions = vec![
        (
            wrapper_permission.clone(),
            ALICE_ID.clone(),
            "the wrapper payout selector",
        ),
        (
            pool_permission.clone(),
            ALICE_ID.clone(),
            "the pool swap selector",
        ),
        (
            effect_permission.clone(),
            BOB_ID.clone(),
            "the wrapper SBD asset transfer effect",
        ),
    ];

    let error = super::install_derived_validation_fee_runtime_permissions_with_validation(
        permissions,
        &mut stx,
        |_| {
            Err(InstructionExecutionError::InvariantViolation(
                "forced post-install topology failure".into(),
            ))
        },
    )
    .expect_err("post-install validation failure must reject lifecycle derivation");
    assert!(
        format!("{error:?}").contains("forced post-install topology failure"),
        "unexpected rollback error: {error:?}"
    );
    for permission in [wrapper_permission, pool_permission, effect_permission] {
        assert!(
            stx.world
                .account_permissions
                .iter()
                .all(|(_, permissions)| !permissions.contains(&permission)),
            "failed post-install validation must roll back every derived permission"
        );
    }
}

fn fee_sponsor_revision_fixture(
    program_id: iroha_data_model::nexus::FeeSponsorProgramId,
    asset_definition_id: AssetDefinitionId,
    revision: u64,
) -> iroha_data_model::nexus::FeeSponsorProgramRevision {
    use iroha_data_model::nexus::{
        FeeSponsorAssetBudget, FeeSponsorEligibility, FeeSponsorIvmSelector,
        FeeSponsorProgramRevision, FeeSponsorRule, FeeSponsorRuleEffect, FeeSponsorRuleSelector,
    };

    FeeSponsorProgramRevision {
        program_id,
        revision,
        eligibility: FeeSponsorEligibility::EnrolledOnly,
        rules: vec![FeeSponsorRule {
            id: "allow_ivm".parse().expect("rule name"),
            effect: FeeSponsorRuleEffect::Allow,
            selectors: vec![FeeSponsorRuleSelector::Ivm(FeeSponsorIvmSelector {
                code_hash: Hash::new(b"fee-sponsor-global-scope-test"),
            })],
        }],
        asset_budgets: vec![FeeSponsorAssetBudget {
            asset_definition_id,
            per_transaction: Quantity::from(1_u32),
            per_block: Quantity::from(10_u32),
            per_program_epoch: Quantity::from(100_u32),
            per_beneficiary_epoch: Quantity::from(5_u32),
            reserve_floor: Quantity::zero(),
            epoch_length_blocks: NonZeroU64::new(100).expect("nonzero epoch"),
        }],
    }
}

#[test]
fn fee_sponsor_vault_allocation_requires_program_management_authority() {
    use iroha_data_model::{
        isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation,
        nexus::{
            DataSpaceId, FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle,
            FeeSponsorProgramRevisionKey, ProofBlob,
        },
        permission::Permissions,
    };
    use iroha_executor_data_model::permission::nexus::CanManageFeeSponsorProgram;

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.nexus.enabled = true;

    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "allocation_auth".parse().expect("program name"),
    );
    stx.world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1),
    );
    let mut program = FeeSponsorProgram::new(program_id.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    stx.world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);

    let error = RegisterVerifiedFeeSponsorVaultAllocation {
        program_id: program_id.clone(),
        program_revision: 1,
        asset_definition_id: asset_definition_id.clone(),
        verified_allocation: Quantity::from(1_u32),
        source_dataspace_id: DataSpaceId::UNIVERSAL,
        source_height: 1,
        source_state_root: Hash::new(b"allocation-auth-source"),
        expires_at_height: 2,
        lease_id: Hash::new(b"allocation-auth-lease"),
        manifest_root: [1; 32],
        proof_blob: ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        },
    }
    .execute(&BOB_ID, &mut stx)
    .expect_err("ordinary accounts must not reserve a sponsor vault");
    assert!(
        error
            .to_string()
            .contains("cannot manage fee sponsor program")
    );

    let mut permissions = Permissions::new();
    permissions.insert(
        CanManageFeeSponsorProgram {
            sponsor: ALICE_ID.clone(),
        }
        .into(),
    );
    stx.world
        .account_permissions
        .insert(BOB_ID.clone(), permissions);
    ensure_fee_sponsor_program_owner(&BOB_ID, &program_id, &stx)
        .expect("delegated manager must be authorized to register allocations");
}

#[test]
fn fee_sponsor_vault_allocation_rejects_future_source_height() {
    use iroha_data_model::{
        isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation,
        nexus::{
            DataSpaceId, FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle,
            FeeSponsorProgramRevisionKey, FeeSponsorVault, FeeSponsorVaultKey, ProofBlob,
        },
    };

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.nexus.enabled = true;

    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    stx.world.asset_definitions.insert(
        asset_definition_id.clone(),
        AssetDefinition::numeric(
            asset_definition_id.clone(),
            "global fee asset".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID),
    );
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "future_source".parse().expect("program name"),
    );
    stx.world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1),
    );
    let mut program = FeeSponsorProgram::new(program_id.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    stx.world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);
    let vault_key = FeeSponsorVaultKey {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
    };
    stx.world.fee_sponsor_vaults.insert(
        vault_key.clone(),
        FeeSponsorVault {
            key: vault_key,
            balance: Quantity::from(10_u32),
        },
    );

    let error = RegisterVerifiedFeeSponsorVaultAllocation {
        program_id,
        program_revision: 1,
        asset_definition_id,
        verified_allocation: Quantity::from(10_u32),
        source_dataspace_id: DataSpaceId::UNIVERSAL,
        source_height: 2,
        source_state_root: Hash::new(b"future-source-state"),
        expires_at_height: u64::MAX,
        lease_id: Hash::new(b"future-source-lease"),
        manifest_root: [1; 32],
        proof_blob: ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        },
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("a source snapshot cannot come from a future height");
    match error {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("invalid source height"),
            "unexpected error: {message}"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn fee_sponsor_rejects_restricted_assets_at_every_write_boundary() {
    use iroha_data_model::{
        isi::nexus::{
            FundFeeSponsorProgram, RegisterVerifiedFeeSponsorVaultAllocation,
            StageFeeSponsorProgramRevision,
        },
        nexus::{
            DataSpaceId, FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle,
            FeeSponsorProgramRevisionKey, ProofBlob,
        },
    };

    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.nexus.enabled = true;

    let authority = ALICE_ID.clone();
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    let owning_domain = DomainId::try_new("fees", "restricted").expect("fee asset owning domain");
    let definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "restricted fee asset".to_owned(),
        AssetBalancePolicy::DataspaceRestricted,
        Some(owning_domain),
    )
    .build(&authority);
    stx.world
        .asset_definitions
        .insert(asset_definition_id.clone(), definition);

    let program_id = FeeSponsorProgramId::new(
        authority.clone(),
        "restricted_asset".parse().expect("program name"),
    );
    let revision_one =
        fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1);
    stx.world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        revision_one,
    );
    let mut program = FeeSponsorProgram::new(program_id.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    stx.world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);

    let stage_error = StageFeeSponsorProgramRevision {
        revision: fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 2),
    }
    .execute(&authority, &mut stx)
    .expect_err("restricted fee asset revision must fail");
    let is_restricted_asset_error = |error: &Error| {
        matches!(
            error,
            Error::InvalidParameter(InvalidParameterError::SmartContract(message))
                if message.contains("requires global-balance")
        )
    };
    assert!(is_restricted_asset_error(&stage_error));
    assert!(
        stx.world
            .fee_sponsor_program_revisions
            .get(&FeeSponsorProgramRevisionKey::new(program_id.clone(), 2))
            .is_none()
    );

    let fund_error = FundFeeSponsorProgram {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
        amount: Quantity::from(1_u32),
    }
    .execute(&authority, &mut stx)
    .expect_err("restricted fee asset funding must fail");
    assert!(is_restricted_asset_error(&fund_error));

    let allocation_error = RegisterVerifiedFeeSponsorVaultAllocation {
        program_id,
        program_revision: 1,
        asset_definition_id,
        verified_allocation: Quantity::from(1_u32),
        source_dataspace_id: DataSpaceId::new(1),
        source_height: 1,
        source_state_root: Hash::new(b"source-state-root"),
        expires_at_height: 10,
        lease_id: Hash::new(b"restricted-fee-asset-lease"),
        manifest_root: [1; 32],
        proof_blob: ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        },
    }
    .execute(&authority, &mut stx)
    .expect_err("restricted fee asset allocation must fail");
    assert!(is_restricted_asset_error(&allocation_error));
}
