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
fn sorafs_provider_owner_transition_requires_full_parliament_gate() {
    let kind = ProposalKind::SorafsProviderGovernance(
        iroha_data_model::governance::types::SorafsProviderGovernanceProposal {
            action: Box::new(
                iroha_data_model::isi::sorafs::SorafsProviderGovernanceActionV1::Establish(
                    iroha_data_model::isi::sorafs::EstablishSorafsProviderOwnerV1 {
                        provider_id: iroha_data_model::sorafs::capacity::ProviderId::new(
                            [0xA7; 32],
                        ),
                        owner: ALICE_ID.clone(),
                    },
                ),
            ),
        },
    );
    assert_eq!(
        super::required_parliament_bodies(&kind),
        &[
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ]
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
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        41,
        DataSpaceId::UNIVERSAL,
    )
    .expect("missing-subject contract address");
    let missing_subject = missing_address.subject_id();
    assert!(state_transaction.world.account(&missing_subject).is_err());
    let bound_subject =
        super::ensure_contract_subject_binding(&ALICE_ID, &mut state_transaction, &missing_address)
            .expect("bind and materialize missing contract subject");
    assert_eq!(bound_subject, missing_subject);
    assert!(state_transaction.world.account(&missing_subject).is_ok());
    assert!(crate::smartcontracts::code::is_historical_contract_subject(
        &state_transaction.world,
        &missing_subject,
    ));
    let existing_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
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
        "the wrapper DS asset transfer effect",
    )
    .expect("an absent effect permission is eligible for protected derivation");
    stx.world
        .account_permissions
        .insert(BOB_ID.clone(), Permissions::from([permission.clone()]));
    let direct_error = super::require_absent_validation_fee_runtime_permission(
        &stx,
        &permission,
        "the wrapper DS asset transfer effect",
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
        "the wrapper DS asset transfer effect",
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
            "the wrapper DS asset transfer effect",
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
fn verified_fee_sponsor_registration_fixture(
    frozen_manifest_root: Option<[u8; 32]>,
    proof_manifest_root: [u8; 32],
    policy_commitment_manifest_root: [u8; 32],
    da_commitment: Option<[u8; 32]>,
    proof_expiry: u64,
) -> (
    State,
    iroha_data_model::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation,
) {
    use iroha_data_model::nexus::{
        AxtEffectBinding, AxtFastpqBinding, AxtPolicyEntry, FeeSponsorProgram, FeeSponsorProgramId,
        FeeSponsorProgramLifecycle, FeeSponsorProgramRevisionKey, FeeSponsorVault,
        FeeSponsorVaultAllocationClaim, FeeSponsorVaultKey,
        fee_sponsor_vault_allocation_claim_digest, fee_sponsor_vault_policy_commitment,
        fee_sponsor_vault_source_state_root,
    };

    let source_dataspace_id = DataSpaceId::new(7);
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "proof_policy".parse().expect("program name"),
    );
    let verified_allocation = Quantity::from(10_u32);
    let source_height = 1;
    let expires_at_height = 20;
    let source_state_root = fee_sponsor_vault_source_state_root(
        &program_id,
        1,
        &asset_definition_id,
        &verified_allocation,
        source_dataspace_id,
        source_height,
    );
    let lease_id = Hash::new(b"verified-fee-sponsor-policy-lease");
    let claim = FeeSponsorVaultAllocationClaim {
        program_id: program_id.clone(),
        program_revision: 1,
        asset_definition_id: asset_definition_id.clone(),
        verified_allocation: verified_allocation.clone(),
        source_dataspace_id,
        source_height,
        source_state_root,
        expires_at_height,
        lease_id,
    };
    let source_tx_commitment = Hash::new(b"verified-fee-sponsor-policy-source-tx");
    let claim_digest = fee_sponsor_vault_allocation_claim_digest(&claim);
    let binding = AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: source_dataspace_id.as_u64(),
        source_dataspace: "dataspace-7".to_owned(),
        source_receipt_id: "verified-fee-sponsor-policy-receipt".to_owned(),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: "authorization".to_owned(),
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(Hash::new(b"verified-fee-sponsor-policy-witness").as_ref()),
        policy_commitment: hex::encode(
            fee_sponsor_vault_policy_commitment(&policy_commitment_manifest_root).as_ref(),
        ),
        verified_effect_type: "fee_sponsor_vault_allocation".to_owned(),
        corridor: "fee-sponsor-program:proof-policy".to_owned(),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
        effect_binding: Some(AxtEffectBinding {
            destination_domain: None,
            destination_account_id: Some(ALICE_ID.to_string()),
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(asset_definition_id.to_string()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        }),
        remote_spend_intent_commitments: Vec::new(),
    };
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&source_dataspace_id.as_u64().to_le_bytes());
    let mut batch = fastpq_prover::TransitionBatch::new(
        fastpq_prover::AXT_DEFAULT_PARAMETER,
        fastpq_prover::PublicInputs {
            dsid: dsid_bytes,
            slot: source_height,
            old_root: *source_state_root.as_ref(),
            new_root: *source_state_root.as_ref(),
            perm_root: Hash::new(b"verified-fee-sponsor-policy-permissions").into(),
            tx_set_hash: claim_digest.into(),
        },
    );
    batch.push(fastpq_prover::StateTransition::new(
        b"axt/nexus/fee-sponsor-vault-allocation".to_vec(),
        lease_id.as_ref().to_vec(),
        claim_digest.as_ref().to_vec(),
        fastpq_prover::OperationKind::MetaSet,
    ));
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_owned(),
        source_tx_commitment.as_ref().to_vec(),
    );
    fastpq_prover::bind_axt_batch_with_proof_metadata(
        &mut batch,
        &binding,
        proof_manifest_root,
        da_commitment,
        Some(10),
        Some(proof_expiry),
    )
    .expect("bind verified fee sponsor proof metadata");
    let proof = fastpq_prover::Prover::canonical(fastpq_prover::AXT_DEFAULT_PARAMETER)
        .expect("construct FastPQ prover")
        .prove_axt_bound(&batch, &binding)
        .expect("prove verified fee sponsor allocation");
    let proof_blob = fastpq_prover::axt_proof_blob_from_bound_batch(
        &batch,
        proof,
        proof_manifest_root,
        da_commitment,
        Some(proof_expiry),
    )
    .expect("package verified fee sponsor proof");

    let instruction = iroha_data_model::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation {
        program_id: program_id.clone(),
        program_revision: 1,
        asset_definition_id: asset_definition_id.clone(),
        verified_allocation: verified_allocation.clone(),
        source_dataspace_id,
        source_height,
        source_state_root,
        expires_at_height,
        lease_id,
        manifest_root: proof_manifest_root,
        proof_blob,
    };

    let mut world = World::default();
    if let Some(manifest_root) = frozen_manifest_root {
        world.axt_policies.insert(
            source_dataspace_id,
            AxtPolicyEntry {
                manifest_root,
                target_lane: LaneId::SINGLE,
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 1,
            },
        );
    }
    world.asset_definitions.insert(
        asset_definition_id.clone(),
        AssetDefinition::numeric(
            asset_definition_id.clone(),
            "global fee asset".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID),
    );
    world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1),
    );
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);
    let vault_key = FeeSponsorVaultKey {
        program_id,
        asset_definition_id,
    };
    world.fee_sponsor_vaults.insert(
        vault_key.clone(),
        FeeSponsorVault {
            key: vault_key,
            balance: verified_allocation,
        },
    );
    (
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ),
        instruction,
    )
}
fn execute_verified_fee_sponsor_registration(
    state: &State,
    instruction: iroha_data_model::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation,
) -> Result<(), Error> {
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();

    instruction.execute(&ALICE_ID, &mut transaction)
}
#[test]
fn verified_fee_sponsor_registration_accepts_exact_proof_policy_context() {
    let manifest_root = [0x63; 32];
    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        Some(manifest_root),
        manifest_root,
        manifest_root,
        None,
        20,
    );
    execute_verified_fee_sponsor_registration(&state, instruction)
        .expect("exact frozen policy and proof metadata must register");
}
#[test]
fn verified_fee_sponsor_registration_rejects_oversized_proof_before_decode() {
    let manifest_root = [0x63; 32];
    let (state, mut instruction) = verified_fee_sponsor_registration_fixture(
        Some(manifest_root),
        manifest_root,
        manifest_root,
        None,
        20,
    );
    instruction.proof_blob.payload =
        vec![0xA5; iroha_data_model::nexus::MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES + 1];

    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("oversized proof payload must fail before canonical decode");
    assert!(
        error.to_string().contains("decode limit"),
        "unexpected oversized proof rejection: {error:?}"
    );
}
#[test]
fn verified_fee_sponsor_registration_rejects_missing_or_rotated_frozen_policy() {
    let proof_manifest_root = [0x63; 32];
    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        None,
        proof_manifest_root,
        proof_manifest_root,
        None,
        20,
    );
    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("missing frozen source policy must fail");
    assert!(error.to_string().contains("no frozen AXT policy"));

    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        Some([0x64; 32]),
        proof_manifest_root,
        proof_manifest_root,
        None,
        20,
    );
    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("proof under a rotated manifest must fail");
    assert!(error.to_string().contains("frozen AXT policy"));
}
#[test]
fn verified_fee_sponsor_registration_rejects_wrong_policy_da_and_expiry() {
    let manifest_root = [0x63; 32];
    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        Some(manifest_root),
        manifest_root,
        [0x64; 32],
        None,
        20,
    );
    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("owner-selected policy commitment must fail");
    assert!(error.to_string().contains("policy commitment mismatch"));

    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        Some(manifest_root),
        manifest_root,
        manifest_root,
        Some([0x22; 32]),
        20,
    );
    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("fee sponsor proof with DA must fail");
    assert!(error.to_string().contains("must not carry a DA commitment"));

    let (state, instruction) = verified_fee_sponsor_registration_fixture(
        Some(manifest_root),
        manifest_root,
        manifest_root,
        None,
        21,
    );
    let error = execute_verified_fee_sponsor_registration(&state, instruction)
        .expect_err("proof expiry must equal the lease deadline");
    assert!(
        error
            .to_string()
            .contains("must equal the allocation lease expiry")
    );
}
#[test]
fn initial_genesis_authority_can_bootstrap_fee_sponsor_lifecycle() {
    use iroha_data_model::{
        isi::nexus::{
            ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
            EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
        },
        nexus::{
            FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle, FeeSponsorVaultKey,
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
    let custody = stx.nexus.fees.sponsor_vault_custody_account_id.clone();
    for account in [ALICE_ID.clone(), BOB_ID.clone(), custody.clone()] {
        if stx.world.account(&account).is_err() {
            Register::account(Account::new(account))
                .execute(&ALICE_ID, &mut stx)
                .expect("register genesis fee sponsor fixture account");
        }
    }
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    stx.world.asset_definitions.insert(
        asset_definition_id.clone(),
        AssetDefinition::numeric(
            asset_definition_id.clone(),
            "global genesis fee asset".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID),
    );
    let sponsor_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    Mint::asset_quantity(Quantity::from(10_u32), sponsor_asset.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("prefund genesis sponsor");
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "genesis_bootstrap".parse().expect("program name"),
    );

    CreateFeeSponsorProgram {
        program: FeeSponsorProgram::new(program_id.clone(), ALICE_ID.clone()),
    }
    .execute(&BOB_ID, &mut stx)
    .expect("initial genesis authority creates sponsor-owned program");
    StageFeeSponsorProgramRevision {
        revision: fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1),
    }
    .execute(&BOB_ID, &mut stx)
    .expect("initial genesis authority stages sponsor revision");
    EnrollFeeSponsorBeneficiary {
        program_id: program_id.clone(),
        beneficiary: ALICE_ID.clone(),
    }
    .execute(&BOB_ID, &mut stx)
    .expect("initial genesis authority enrolls exact beneficiary");
    FundFeeSponsorProgram {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
        amount: Quantity::from(10_u32),
    }
    .execute(&BOB_ID, &mut stx)
    .expect("initial genesis authority funds from the exact sponsor balance");
    ActivateFeeSponsorProgramRevision {
        program_id: program_id.clone(),
        revision: 1,
        activate_at_height: 1,
    }
    .execute(&BOB_ID, &mut stx)
    .expect("initial genesis authority activates ready sponsor revision");

    let program = stx
        .world
        .fee_sponsor_programs
        .get(&program_id)
        .expect("bootstrapped sponsor program");
    assert_eq!(program.lifecycle, FeeSponsorProgramLifecycle::Active);
    assert_eq!(program.active_revision, Some(1));
    let vault_key = FeeSponsorVaultKey {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
    };
    assert_eq!(
        stx.world
            .fee_sponsor_vaults
            .get(&vault_key)
            .expect("genesis funding creates the isolated sponsor vault")
            .balance,
        Quantity::from(10_u32),
    );
    assert_eq!(
        stx.pending_transfer_transcript_count_for_testing(),
        1,
        "genesis sponsor funding must retain one auditable transfer transcript",
    );
    assert!(
        stx.world.assets.get(&sponsor_asset).is_none(),
        "fully funded sponsor source is removed at zero balance",
    );
    let custody_asset = AssetId::new(asset_definition_id, custody);
    assert_eq!(
        stx.world
            .assets
            .get(&custody_asset)
            .expect("genesis sponsor funding reaches custody")
            .as_ref(),
        &Quantity::from(10_u32),
    );
}
#[test]
fn post_genesis_authority_cannot_bootstrap_another_sponsors_program() {
    use iroha_data_model::{
        isi::nexus::CreateFeeSponsorProgram,
        nexus::{FeeSponsorProgram, FeeSponsorProgramId},
    };
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(2).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "post_genesis_denied".parse().expect("program name"),
    );
    let error = CreateFeeSponsorProgram {
        program: FeeSponsorProgram::new(program_id, ALICE_ID.clone()),
    }
    .execute(&BOB_ID, &mut stx)
    .expect_err("height-two authority must not manage another sponsor's program");
    assert!(
        error
            .to_string()
            .contains("cannot manage fee sponsor program")
    );
}
#[test]
fn replayed_genesis_header_cannot_regain_fee_sponsor_bootstrap_authority() {
    use iroha_data_model::{
        isi::nexus::CreateFeeSponsorProgram,
        nexus::{FeeSponsorProgram, FeeSponsorProgramId},
    };
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut hashes = state.block_hashes.block();
        hashes.push_for_tests(
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new(b"fee-sponsor-genesis-replay-guard"),
            ),
        );
        hashes.commit_for_tests();
    }
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
    assert!(stx._curr_block.is_genesis());
    assert!(!crate::executor::is_initial_genesis_context(&stx));
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "replayed_genesis_denied".parse().expect("program name"),
    );
    let error = CreateFeeSponsorProgram {
        program: FeeSponsorProgram::new(program_id.clone(), ALICE_ID.clone()),
    }
    .execute(&BOB_ID, &mut stx)
    .expect_err("committed history must disable the height-one owner exception");
    assert!(
        error
            .to_string()
            .contains("cannot manage fee sponsor program")
    );
    assert!(stx.world.fee_sponsor_programs.get(&program_id).is_none());
}
#[test]
fn post_genesis_fund_mismatch_preserves_balances_vault_and_transcripts() {
    use iroha_data_model::{
        isi::nexus::FundFeeSponsorProgram,
        nexus::{FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorVaultKey},
    };
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(2).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let custody = stx.nexus.fees.sponsor_vault_custody_account_id.clone();
    for account in [ALICE_ID.clone(), BOB_ID.clone(), custody.clone()] {
        if stx.world.account(&account).is_err() {
            Register::account(Account::new(account))
                .execute(&ALICE_ID, &mut stx)
                .expect("register post-genesis sponsor fixture account");
        }
    }
    let asset_definition_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical asset definition id");
    stx.world.asset_definitions.insert(
        asset_definition_id.clone(),
        AssetDefinition::numeric(
            asset_definition_id.clone(),
            "post-genesis fee asset".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID),
    );
    let sponsor_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    Mint::asset_quantity(Quantity::from(10_u32), sponsor_asset.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("prefund post-genesis sponsor");
    let custody_asset = AssetId::new(asset_definition_id.clone(), custody);
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "fund_mismatch".parse().expect("program name"),
    );
    stx.world.fee_sponsor_programs.insert(
        program_id.clone(),
        FeeSponsorProgram::new(program_id.clone(), ALICE_ID.clone()),
    );
    let vault_key = FeeSponsorVaultKey {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
    };
    let sponsor_before = stx
        .world
        .assets
        .get(&sponsor_asset)
        .expect("prefunded sponsor asset")
        .as_ref()
        .clone();
    let custody_before = stx.world.assets.get(&custody_asset).cloned();

    let error = FundFeeSponsorProgram {
        program_id,
        asset_definition_id,
        amount: Quantity::from(3_u32),
    }
    .execute(&BOB_ID, &mut stx)
    .expect_err("height-two non-owner funding must fail before moving assets");

    assert!(
        error
            .to_string()
            .contains("cannot manage fee sponsor program")
    );
    assert_eq!(
        stx.world
            .assets
            .get(&sponsor_asset)
            .expect("rejected funding preserves sponsor asset")
            .as_ref(),
        &sponsor_before,
    );
    assert_eq!(
        stx.world.assets.get(&custody_asset).cloned(),
        custody_before
    );
    assert!(stx.world.fee_sponsor_vaults.get(&vault_key).is_none());
    assert_eq!(stx.pending_transfer_transcript_count_for_testing(), 0);
}
#[test]
fn fee_sponsor_program_rejects_unregistered_payout_account() {
    use iroha_data_model::{
        isi::nexus::CreateFeeSponsorProgram,
        nexus::{FeeSponsorProgram, FeeSponsorProgramId},
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
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register sponsor");
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "closed_payout".parse().expect("program name"),
    );
    let create = CreateFeeSponsorProgram {
        program: FeeSponsorProgram::new(program_id.clone(), BOB_ID.clone()),
    };
    let error = create
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("an unregistered payout account must fail closed");
    assert!(
        error
            .to_string()
            .contains("unknown fee sponsor payout account")
    );
    assert!(stx.world.fee_sponsor_programs.get(&program_id).is_none());
    Register::account(Account::new(BOB_ID.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register payout account");
    create
        .execute(&ALICE_ID, &mut stx)
        .expect("registered immutable payout account must be accepted");
    assert_eq!(
        stx.world
            .fee_sponsor_programs
            .get(&program_id)
            .expect("created sponsor program")
            .payout_account,
        *BOB_ID
    );
    let error = Unregister::account(BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("a live program's immutable payout account must remain registered");
    assert!(error.to_string().contains("immutable payout account"));
}
#[test]
fn fee_sponsor_withdrawal_is_owner_only_and_pays_registered_account() {
    use iroha_data_model::{
        isi::nexus::WithdrawFeeSponsorProgram,
        nexus::{
            FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle, FeeSponsorVault,
            FeeSponsorVaultKey,
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
    let custody = stx.nexus.fees.sponsor_vault_custody_account_id.clone();
    for account in [ALICE_ID.clone(), BOB_ID.clone(), custody.clone()] {
        if stx.world.account(&account).is_err() {
            Register::account(Account::new(account))
                .execute(&ALICE_ID, &mut stx)
                .expect("register sponsor withdrawal fixture account");
        }
    }
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
    let custody_asset = AssetId::new(asset_definition_id.clone(), custody);
    Mint::asset_quantity(Quantity::from(10_u32), custody_asset.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("fund sponsor custody");
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "owner_payout".parse().expect("program name"),
    );
    let mut program = FeeSponsorProgram::new(program_id.clone(), BOB_ID.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Paused;
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
            key: vault_key.clone(),
            balance: Quantity::from(10_u32),
        },
    );
    stx.world.account_permissions.insert(
        BOB_ID.clone(),
        Permissions::from([CanManageFeeSponsorProgram {
            sponsor: ALICE_ID.clone(),
        }
        .into()]),
    );
    let withdrawal = WithdrawFeeSponsorProgram {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
        amount: Quantity::from(3_u32),
    };
    let error = withdrawal
        .clone()
        .execute(&BOB_ID, &mut stx)
        .expect_err("a delegated manager must not withdraw sponsor funds");
    assert!(error.to_string().contains("only sponsor"));
    assert_eq!(
        stx.world
            .fee_sponsor_vaults
            .get(&vault_key)
            .expect("rejected withdrawal preserves vault")
            .balance,
        Quantity::from(10_u32)
    );
    withdrawal
        .execute(&ALICE_ID, &mut stx)
        .expect("exact sponsor may withdraw to the registered payout account");
    let payout_asset = AssetId::new(asset_definition_id, BOB_ID.clone());
    assert_eq!(
        stx.world
            .assets
            .get(&payout_asset)
            .expect("registered payout receives withdrawal")
            .as_ref(),
        &Quantity::from(3_u32)
    );
    assert_eq!(
        stx.world
            .assets
            .get(&custody_asset)
            .expect("custody retains remaining balance")
            .as_ref(),
        &Quantity::from(7_u32)
    );
    assert_eq!(
        stx.world
            .fee_sponsor_vaults
            .get(&vault_key)
            .expect("nonempty vault remains")
            .balance,
        Quantity::from(7_u32)
    );
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
        NonZeroU64::new(2).expect("nonzero height"),
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
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "allocation_auth".parse().expect("program name"),
    );
    stx.world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        fee_sponsor_revision_fixture(program_id.clone(), asset_definition_id.clone(), 1),
    );
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
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
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
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
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
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
