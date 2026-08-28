// Shared imports and fixtures for the validation-fee test shards.
use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId, NetworkId,
    asset::{AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    events::{
        execute_trigger::ExecuteTriggerEventFilter,
        time::{ExecutionTime, Schedule, TimeEventFilter},
    },
    governance::types::{GovernanceAttemptId, GovernanceCertificateId},
    hijiri::{FeeMultiplierBand, HijiriAccountRiskV1, HijiriFeePolicy, Q16},
    isi::{
        InstructionBox, Transfer, TransferAssetBatchEntry,
        offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
        repo::RepoMarginCallIsi,
        settlement::{SettlementLeg, SettlementPlan},
    },
    offline::{
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND, KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        KagemushaAndroidKeyMintHardwareAssertionV1, KagemushaOnlineHardwareAssertionV1,
        KagemushaPastaCycleParityV1, KagemushaPastaCycleProofEnvelopeV4,
        KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendBranchClaimV2,
        KagemushaRecursiveSpendBundleV4, KagemushaRecursiveSpendOperationVectorV4,
        KagemushaRecursiveSpendProofV4, KagemushaRecursiveSpendPublicStatementV4,
        KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendRedeemUnsignedV4,
        KagemushaRecursiveSpendRedemptionIntentV4, KagemushaRecursiveSpendStateBoundaryV5,
        KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaRecursiveSpendTopUpRequestV4,
        KagemushaRecursiveSpendTopUpUnsignedV4, KagemushaRequestAuthorizationV2,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        KagemushaUnshieldPublicInputsBindingV2, kagemusha_confidential_amount_encoding_v2,
        kagemusha_recursive_spend_verifier_key_id_v4,
    },
    prelude::Register,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    repo::{RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    transaction::{
        Executable, IvmBytecode, IvmProved, TransactionBuilder, executable::ContractInvocation,
    },
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
    validation_fee::{
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePayoutLifecycleReferenceV1, ValidationFeePolicyRegistryEntryV1,
        ValidationFeePolicyRegistryV1, ValidationFeeTreasuryPayoutRecipientV1,
    },
};
use iroha_executor_data_model::isi::multisig::{MultisigApprove, MultisigPropose};
use iroha_primitives::json::Json;
use std::str::FromStr as _;
const TEST_VALIDATION_FEE_ASSET_SCALE: u8 =
    iroha_data_model::validation_fee::VALIDATION_FEE_DS_SCALE;
const TEST_VALIDATION_FEE_MINOR_UNITS: u64 = 10;
const TEST_REFERENDUM_START_HEIGHT: u64 = 10;
const TEST_POLICY_EFFECTIVE_HEIGHT: u64 = TEST_REFERENDUM_START_HEIGHT
    + 3_600
    + iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
fn key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair")
}
fn account(seed: u8) -> AccountId {
    let key_pair = key_pair(seed);
    AccountId::new(key_pair.public_key().clone())
}
fn asset_definition(name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        Name::from_str(name).expect("asset name"),
    )
}
fn fee_asset() -> AssetDefinitionId {
    asset_definition("fee_token")
}
fn successor_fee_asset() -> AssetDefinitionId {
    asset_definition("successor_fee_token")
}
fn validation_fee_test_network_id() -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([7; 32]),
    ))
}
fn policy(treasury: &AccountId) -> ValidationFeePolicyV1 {
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: validation_fee_test_network_id(),
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: fee_asset(),
        ds_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
        fee: iroha_data_model::validation_fee::initial_validation_fee_amount(),
        treasury_account_id: treasury.clone(),
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: TEST_POLICY_EFFECTIVE_HEIGHT,
        expires_after_height: TEST_POLICY_EFFECTIVE_HEIGHT.checked_add(100),
        exemption_classes: Vec::new(),
        treasury_payout_binding: None,
    }
}
fn hijiri_parameters(default_account_risk: Q16) -> HijiriParametersV1 {
    let fee_policy = HijiriFeePolicy::new(
        vec![
            FeeMultiplierBand::new(Q16::from_parts(0, 0x8000), Q16::ONE)
                .expect("low-risk fee band"),
            FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(2, 0)).expect("high-risk fee band"),
        ],
        Q16::from_parts(2, 0),
    )
    .expect("Hijiri fee policy");
    HijiriParametersV1::try_new(1, None, fee_policy, default_account_risk)
        .expect("Hijiri parameters")
}
fn xor_asset() -> AssetDefinitionId {
    asset_definition("xor")
}
fn test_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(
        &validation_fee_test_network_id(),
        &account(9),
        42,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .expect("test contract address")
}
fn treasury_payout_binding(
    contract_address: iroha_data_model::smart_contract::ContractAddress,
    code: &[u8],
) -> ValidationFeeTreasuryPayoutBindingV1 {
    let treasury = contract_address.subject_id();
    ValidationFeeTreasuryPayoutBindingV1 {
        contract_address,
        code_hash: <[u8; 32]>::from(Sha256::digest(code)),
        entrypoint: "autonomous_validation_fee_tick"
            .parse()
            .expect("payout entrypoint"),
        treasury_account_id: treasury,
        ds_asset_id: fee_asset(),
        xor_asset_id: xor_asset(),
        pool_vault_account_id: account(2),
        batch_ds: iroha_data_model::validation_fee::validation_fee_payout_batch_ds(),
        min_xor_out: Quantity::from(4_u64),
        max_xor_out: Quantity::from(100_u64),
        recipients: (3..=6)
            .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                account_id: account(seed),
                share: "0.25".parse().expect("validator share"),
            })
            .collect(),
    }
}
fn policy_with_treasury_payout_lifecycle(
    binding: ValidationFeeTreasuryPayoutBindingV1,
) -> ValidationFeePolicyV1 {
    let mut policy = policy(&binding.treasury_account_id);
    policy
        .exemption_classes
        .push(VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_string());
    policy.treasury_payout_binding = Some(binding);
    policy
}
fn policy_fee_asset(policy: &ValidationFeePolicyV1) -> AssetDefinitionId {
    policy.ds_asset_id.clone()
}
fn successor_policy(previous: &ValidationFeePolicyV1) -> ValidationFeePolicyV1 {
    let mut policy = previous.clone();
    policy.policy_version += 1;
    policy.previous_policy_hash = Some(previous.policy_hash().expect("previous policy hash"));
    policy.effective_from_height += 100;
    policy.expires_after_height = Some(policy.effective_from_height + 100);
    policy
}
fn test_parliament_candidates() -> Vec<AccountId> {
    (220_u8..=243).map(account).collect()
}
fn test_authorization(
    proposal: &iroha_data_model::governance::types::ProposalKind,
    policy_effective_height: u64,
) -> ValidationFeeParliamentAuthorizationV1 {
    let enacted_at_height = policy_effective_height
        .checked_sub(
            iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        )
        .expect("test policy leaves the full activation delay");
    let attempt = crate::governance::parliament::enacted_parliament_attempt_for_testing(
        proposal,
        test_parliament_candidates(),
        &validation_fee_test_network_id(),
        enacted_at_height,
    );
    let governance_certificate = attempt
        .certificate()
        .cloned()
        .expect("test Parliament attempt retains its enacted certificate");
    let proposal_operator = match proposal {
        iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(proposal) => {
            proposal.proposal_operator.clone()
        }
        iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
            proposal,
        ) => proposal.proposal_operator.clone(),
        _ => panic!("validation-fee authorization fixture requires a validation-fee proposal"),
    };
    let governance_certificate_id = GovernanceCertificateId::derive_v1(&governance_certificate);
    ValidationFeeParliamentAuthorizationV1 {
        proposal_operator,
        proposal_fingerprint: proposal.fingerprint(),
        governance_certificate_id,
        governance_certificate,
        enacted_at_height,
    }
}
fn policy_registry(policies: &[ValidationFeePolicyV1]) -> ValidationFeePolicyRegistryV1 {
    let registered_policies = policies
        .iter()
        .map(|policy| {
            use iroha_data_model::governance::types::{
                ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
            };
            let payout_lifecycle = policy.treasury_payout_binding.as_ref().map(|binding| {
                let lifecycle_seal = binding
                    .lifecycle_seal()
                    .expect("derive payout lifecycle seal");
                let lifecycle_kind = ProposalKind::ValidationFeePayoutLifecycle(
                    ValidationFeePayoutLifecycleProposal {
                        proposal_operator: account(250),
                        payout_binding: binding.clone(),
                    },
                );
                ValidationFeePayoutLifecycleReferenceV1 {
                    lifecycle_seal,
                    parliament_authorization: test_authorization(
                        &lifecycle_kind,
                        policy.effective_from_height,
                    ),
                }
            });
            let lifecycle_id = payout_lifecycle
                .as_ref()
                .map(|reference| reference.parliament_authorization.proposal_fingerprint);
            let kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                proposal_operator: account(250),
                policy: policy.clone(),
                payout_lifecycle_proposal_id: lifecycle_id,
            });
            ValidationFeePolicyRegistryEntryV1::from_enactment(
                policy.clone(),
                test_authorization(&kind, policy.effective_from_height),
                payout_lifecycle,
            )
            .expect("registry entry")
        })
        .collect::<Vec<_>>();
    ValidationFeePolicyRegistryV1 {
        registered_policies,
    }
}
fn seed_authorized_proposal(
    kind: iroha_data_model::governance::types::ProposalKind,
    authorization: ValidationFeeParliamentAuthorizationV1,
    state_tx: &mut StateTransaction<'_, '_>,
) {
    let proposal_id = authorization.proposal_fingerprint;
    assert_eq!(kind.fingerprint(), proposal_id);
    assert_eq!(authorization.invariant_error(), None);
    let proposal_operator = authorization.proposal_operator.clone();
    let attempt = crate::governance::parliament::enacted_parliament_attempt_for_testing(
        &kind,
        test_parliament_candidates(),
        &validation_fee_test_network_id(),
        authorization.enacted_at_height,
    );
    assert_eq!(
        attempt.certificate(),
        Some(&authorization.governance_certificate),
        "authorization must retain the exact certificate produced by its Parliament attempt"
    );
    let attempt_id = attempt.attempt().id;
    state_tx
        .world
        .put_governance_proposal(
            proposal_id,
            crate::state::GovernanceProposalRecord {
                proposer: proposal_operator,
                kind,
                created_height: 1,
                status: crate::state::GovernanceProposalStatus::Enacted,
            },
        )
        .expect("validation-fee test proposal must satisfy first-release JSON bounds");
    state_tx
        .world
        .put_parliament_attempt_for_testing(attempt_id, attempt)
        .expect("persist exact enacted validation-fee Parliament attempt");
}
fn install_policy_registry_fixture(
    registry: &ValidationFeePolicyRegistryV1,
    state_tx: &mut StateTransaction<'_, '_>,
) {
    use iroha_data_model::governance::types::{
        ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    };
    for entry in &registry.registered_policies {
        if let (Some(binding), Some(reference)) = (
            entry.policy.treasury_payout_binding.as_ref(),
            entry.payout_lifecycle.as_ref(),
        ) {
            seed_authorized_proposal(
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: reference.parliament_authorization.proposal_operator.clone(),
                    payout_binding: binding.clone(),
                }),
                reference.parliament_authorization.clone(),
                state_tx,
            );
        }
        seed_authorized_proposal(
            ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                proposal_operator: entry.parliament_authorization.proposal_operator.clone(),
                policy: entry.policy.clone(),
                payout_lifecycle_proposal_id: entry
                    .payout_lifecycle
                    .as_ref()
                    .map(|reference| reference.parliament_authorization.proposal_fingerprint),
            }),
            entry.parliament_authorization.clone(),
            state_tx,
        );
    }
    state_tx
        .world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(registry.clone().into_custom_parameter()));
}
fn block_hash(bytes: [u8; 32]) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
}
fn minimal_bound_contract_artifact() -> (
    Vec<u8>,
    iroha_data_model::smart_contract::manifest::ContractManifest,
) {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    };
    let wrapper_entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
        name: "autonomous_validation_fee_tick".to_owned(),
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        permission: Some(VALIDATION_FEE_PAYOUT_WRAPPER_ENTRYPOINT_PERMISSION.to_owned()),
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
    };
    let pool_entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
        name: VALIDATION_FEE_POOL_SWAP_ENTRYPOINT.to_owned(),
        ..wrapper_entrypoint.clone()
    };
    let entrypoints = [wrapper_entrypoint, pool_entrypoint];
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "ValidationFeePayout".to_owned(),
        compiler_fingerprint: "validation-fee-bound-contract-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: entrypoints
            .iter()
            .enumerate()
            .map(|(index, entrypoint)| ivm::EmbeddedEntrypointDescriptor {
                name: entrypoint.name.clone(),
                kind: entrypoint.kind,
                params: entrypoint.params.clone(),
                argument_schema: entrypoint.argument_schema.clone(),
                return_type: entrypoint.return_type.clone(),
                return_schema: entrypoint.return_schema.clone(),
                permission: entrypoint.permission.clone(),
                read_keys: entrypoint.read_keys.clone(),
                write_keys: entrypoint.write_keys.clone(),
                access_hints_complete: entrypoint.access_hints_complete,
                access_hints_skipped: entrypoint.access_hints_skipped.clone(),
                triggers: entrypoint.triggers.clone(),
                entry_pc: u64::try_from(index).expect("fixture entrypoint index fits u64") * 4,
            })
            .collect(),
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut instructions = Vec::new();
    for _ in &entrypoints {
        instructions.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    }
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&instructions);
    let verified = ivm::verify_contract_artifact(&artifact).expect("valid bound contract artifact");
    (artifact, verified.manifest)
}
fn validation_fee_payout_world(deployer: &AccountId) -> crate::state::World {
    use iroha_data_model::prelude::{Account, AssetDefinition, Domain};
    let contract_domain =
        Domain::new(DomainId::try_new("contracts", "universal").expect("contract domain id"))
            .build(deployer);
    let fee_domain = Domain::new(DomainId::try_new("fees", "paynet").expect("fee-asset domain id"))
        .build(deployer);
    let mut accounts = vec![Account::new(deployer.clone()).build(deployer)];
    accounts.extend((2..=7).map(|seed| Account::new(account(seed)).build(deployer)));
    let fee_definition = AssetDefinition::new(
        fee_asset(),
        "fee_token".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(deployer);
    let xor_definition = AssetDefinition::new(
        xor_asset(),
        "xor".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(deployer);
    let successor_fee_definition = AssetDefinition::new(
        successor_fee_asset(),
        "successor_fee_token".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(deployer);
    crate::state::World::with(
        [contract_domain, fee_domain],
        accounts,
        [fee_definition, successor_fee_definition, xor_definition],
    )
}
fn register_bound_payout_time_trigger(
    state_tx: &mut StateTransaction<'_, '_>,
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    expected_code_hash: Hash,
    trigger_id: &str,
) -> iroha_data_model::trigger::TriggerId {
    let trigger_id: iroha_data_model::trigger::TriggerId =
        trigger_id.parse().expect("payout trigger id");
    let block_cadence = std::time::Duration::from_millis(
        state_tx
            .world
            .parameters
            .sumeragi()
            .block_cadence_ms()
            .get(),
    );
    let action = Action::new(
        Executable::ContractCall(ContractInvocation {
            contract_address: binding.contract_address.clone(),
            expected_code_hash,
            entrypoint: binding.entrypoint.to_string(),
            arguments: None,
        }),
        Repeats::Indefinitely,
        binding.treasury_account_id.clone(),
        TimeEventFilter::new(ExecutionTime::Schedule(
            Schedule::starting_at(std::time::Duration::from_millis(1)).with_period(block_cadence),
        )),
    )
    .expect("bound payout trigger action");
    let trigger = Trigger::new(trigger_id.clone(), action);
    crate::smartcontracts::isi::triggers::isi::register_trigger_internal(
        &binding.treasury_account_id,
        state_tx,
        trigger,
        None,
    )
    .expect("register exact bound payout trigger");
    trigger_id
}
pub(crate) struct BoundPayoutRuntimeFixture {
    pub(crate) binding: ValidationFeeTreasuryPayoutBindingV1,
    runtime: crate::executor::ContractRuntimeExecutionContext,
    trigger_id: iroha_data_model::trigger::TriggerId,
}
pub(crate) fn with_validation_fee_payout_state_at_height(
    height: u64,
    test: impl FnOnce(&mut StateTransaction<'_, '_>, &AccountId, &[u8], Hash),
) {
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
        std::num::NonZeroU64::new(height).expect("test height is non-zero"),
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
            .expect("register payout contract bytes");
    crate::smartcontracts::code::register_manifest(
        &deployer,
        manifest.signed(&deployer_key),
        &mut state_tx,
    )
    .expect("register payout contract manifest");
    test(&mut state_tx, &deployer, &code, code_hash);
}
pub(crate) fn activate_bound_payout_runtime(
    state_tx: &mut StateTransaction<'_, '_>,
    deployer: &AccountId,
    code: &[u8],
    code_hash: Hash,
    nonce: u64,
    ds_asset_id: AssetDefinitionId,
    trigger_id: &str,
) -> BoundPayoutRuntimeFixture {
    use iroha_data_model::{nexus::DataSpaceId, smart_contract::ContractAddress};
    let contract_address = ContractAddress::derive(
        &state_tx.network_id,
        deployer,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive payout contract address");
    crate::smartcontracts::code::activate_instance(
        deployer,
        contract_address.clone(),
        code_hash,
        state_tx,
    )
    .expect("activate payout contract instance");
    let mut binding = treasury_payout_binding(contract_address.clone(), code);
    binding.ds_asset_id = ds_asset_id;
    let trigger_id = register_bound_payout_time_trigger(state_tx, &binding, code_hash, trigger_id);
    let runtime = crate::executor::ContractRuntimeExecutionContext {
        contract_address,
        contract_subject: binding.treasury_account_id.clone(),
        contract_alias: None,
        entrypoint: binding.entrypoint.to_string(),
    };
    BoundPayoutRuntimeFixture {
        binding,
        runtime,
        trigger_id,
    }
}
fn lifecycle_credit(policy: &ValidationFeePolicyV1) -> ValidationFeeCredit {
    let binding = policy
        .treasury_payout_binding
        .as_ref()
        .expect("payout policy binding");
    ValidationFeeCredit {
        treasury_account_id: binding.treasury_account_id.clone(),
        lifecycle_seal: binding.lifecycle_seal().expect("payout lifecycle seal"),
        fee_asset_definition_id: binding.ds_asset_id.clone(),
        asset_scale: policy.ds_scale,
        amount: binding.batch_ds.clone(),
    }
}
fn enforce_bound_payout_tick(
    state_tx: &mut StateTransaction<'_, '_>,
    fixture: &BoundPayoutRuntimeFixture,
    code: &[u8],
    instructions: Vec<InstructionBox>,
) -> Result<OpaqueDeferredValidationOutcome, TransactionRejectionReason> {
    let ordered = ordered_treasury_payout_plan(&fixture.binding, &instructions);
    let groups = std::collections::BTreeMap::from([(
        fixture.binding.treasury_account_id.clone(),
        instructions,
    )]);
    enforce_opaque_deferred_instruction_groups(
        &groups,
        &ordered,
        state_tx,
        Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
            &fixture.runtime,
            code,
            &fixture.trigger_id,
        )),
    )
}
fn install_active_bound_validation_fee_policy(
    state_tx: &mut StateTransaction<'_, '_>,
    deployer: &AccountId,
    deployer_key: &KeyPair,
) -> ValidationFeePolicyV1 {
    use iroha_data_model::{nexus::DataSpaceId, smart_contract::ContractAddress};
    let deployment_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    crate::smartcontracts::Execute::execute(
        iroha_data_model::isi::Grant::account_permission(deployment_permission, deployer.clone()),
        deployer,
        state_tx,
    )
    .expect("grant contract lifecycle authority");
    let (code, manifest) = minimal_bound_contract_artifact();
    let code_hash =
        crate::smartcontracts::code::register_code_bytes(deployer, code.clone(), state_tx)
            .expect("register contract bytes");
    crate::smartcontracts::code::register_manifest(
        deployer,
        manifest.signed(deployer_key),
        state_tx,
    )
    .expect("register signed contract manifest");
    let contract_address =
        ContractAddress::derive(&state_tx.network_id, deployer, 0, DataSpaceId::UNIVERSAL)
            .expect("contract address");
    crate::smartcontracts::code::activate_instance(
        deployer,
        contract_address.clone(),
        code_hash,
        state_tx,
    )
    .expect("activate contract instance");
    let binding = treasury_payout_binding(contract_address, &code);
    let policy = policy_with_treasury_payout_lifecycle(binding);
    install_policy_registry_fixture(&policy_registry(std::slice::from_ref(&policy)), state_tx);
    policy
}
fn minor_units(value: u64) -> Quantity {
    quantity_from_policy_minor_units(value, TEST_VALIDATION_FEE_ASSET_SCALE)
        .expect("validation-fee fixture minor units fit Quantity")
}
fn transfer(
    from: &AccountId,
    asset_definition: &AssetDefinitionId,
    amount: Quantity,
    to: &AccountId,
) -> InstructionBox {
    Transfer::asset_quantity(
        AssetId::new(asset_definition.clone(), from.clone()),
        amount,
        to.clone(),
    )
    .into()
}
fn canonical_treasury_payout_plan(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    xor_out: Quantity,
) -> Vec<InstructionBox> {
    treasury_payout_plan(binding, binding.batch_ds.clone(), xor_out)
}
fn treasury_payout_plan(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    debit_ds: Quantity,
    xor_out: Quantity,
) -> Vec<InstructionBox> {
    let xor_scale = u32::from(TEST_VALIDATION_FEE_ASSET_SCALE);
    let payouts = canonical_validation_fee_payouts(binding, &xor_out, xor_scale)
        .expect("test XOR payout arithmetic is valid")
        .expect("test XOR output funds every recipient");
    let mut instructions = vec![
        transfer(
            &binding.treasury_account_id,
            &binding.ds_asset_id,
            debit_ds,
            &binding.pool_vault_account_id,
        ),
        transfer(
            &binding.pool_vault_account_id,
            &binding.xor_asset_id,
            xor_out.clone(),
            &binding.treasury_account_id,
        ),
    ];
    instructions.extend(payouts.into_iter().map(|(recipient, amount)| {
        transfer(
            &binding.treasury_account_id,
            &binding.xor_asset_id,
            amount,
            &recipient,
        )
    }));
    instructions
}
fn ordered_treasury_payout_plan(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    instructions: &[InstructionBox],
) -> Vec<(AccountId, InstructionBox)> {
    instructions
        .iter()
        .cloned()
        .map(|instruction| (binding.treasury_account_id.clone(), instruction))
        .collect()
}
fn assert_treasury_payout_plan_mismatch(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    ordered: &[(AccountId, InstructionBox)],
) {
    let terms = validation_fee_payout_terms(
        &binding.batch_ds,
        binding,
        TEST_VALIDATION_FEE_ASSET_SCALE,
        u32::from(TEST_VALIDATION_FEE_ASSET_SCALE),
    )
    .expect("derive full-batch test payout terms")
    .expect("full-batch test payout has a nonempty range");
    assert!(matches!(
        validate_treasury_payout_effect_plan(groups, ordered, binding, &terms),
        Ok(false)
            | Err(ValidationFeeAdmissionError::TreasuryPayoutEffectPlanMismatch { .. })
            | Err(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)
    ));
}
fn kagemusha_artifact_binding() -> KagemushaRecursiveSpendArtifactBindingV4 {
    KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "validation-fee-kagemusha-v4".to_owned(),
        manifest_sha256: [0xA1; 32],
    }
}
fn kagemusha_authorization(
    authority: AccountId,
    asset_definition_id: AssetDefinitionId,
    operation_id: [u8; 32],
    payload_digest: [u8; 32],
) -> KagemushaRequestAuthorizationV2 {
    KagemushaRequestAuthorizationV2 {
        authority,
        device_id: "validation-fee-kagemusha-device".to_owned(),
        asset_definition_id,
        operation_id,
        issued_at_ms: 1,
        expires_at_ms: 2,
        nonce: [0xA2; 32],
        payload_digest,
        registration_hash: [0xA3; 32],
        hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
            KagemushaAndroidKeyMintHardwareAssertionV1 {
                signature: iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                    &[1; 64],
                )
                .expect("canonical low-S fixture signature"),
            },
        ),
    }
}
fn kagemusha_top_up_request(
    asset_definition_id: &AssetDefinitionId,
) -> KagemushaRecursiveSpendTopUpRequestV4 {
    let payer = account(1);
    let network_id = NetworkId::from_genesis_hash(block_hash([0xA0; 32]));
    let amount = KagemushaScaledAmountV2::new(500, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
        .expect("positive top-up amount");
    let operation_id = [0xA4; 32];
    let mut shield_proof = ProofAttachment::new_ref(
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
        ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xA5]),
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            "validation-fee-topup-shield",
        ),
    );
    shield_proof.vk_commitment = Some([0xA6; 32]);
    let unsigned = KagemushaRecursiveSpendTopUpUnsignedV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        asset: AssetId::new(asset_definition_id.clone(), payer.clone()),
        amount,
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id,
            asset: asset_definition_id.clone(),
            note_commitment: [0xA7; 32],
            spend_nullifier: [0xA8; 32],
            amount,
        },
        shield_evidence: iroha_data_model::offline::KagemushaTopUpShieldEvidenceV2 {
            initial_root: [0xA9; 32],
            finalized_root: [0xAA; 32],
            leaf_index: 0,
            proof: shield_proof,
        },
        artifact_binding: kagemusha_artifact_binding(),
        operation_id,
    };
    let payload_digest = unsigned.digest().expect("valid top-up payload");
    unsigned
        .into_request(kagemusha_authorization(
            payer,
            asset_definition_id.clone(),
            operation_id,
            payload_digest,
        ))
        .expect("valid top-up request")
}
fn kagemusha_redeem_request(
    asset_definition_id: &AssetDefinitionId,
) -> KagemushaRecursiveSpendRedeemRequestV4 {
    let recipient = account(1);
    let network_id = NetworkId::from_genesis_hash(block_hash([0xA0; 32]));
    let amount = KagemushaScaledAmountV2::new(500, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
        .expect("positive redemption amount");
    let operation_id = [0xB1; 32];
    let topup_anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: [0xB2; 32],
        anchor_digest: [0xB3; 32],
    };
    let branch_claim = KagemushaRecursiveSpendBranchClaimV2::root(topup_anchor_ref.anchor_digest)
        .expect("canonical root branch claim");
    let note = KagemushaSpendableNoteDescriptorV2 {
        network_id,
        asset: asset_definition_id.clone(),
        note_commitment: [0xB4; 32],
        spend_nullifier: [0xB5; 32],
        amount,
    };
    let binding = kagemusha_artifact_binding();
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        binding.manifest_sha256,
    );
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        network_id,
        asset: asset_definition_id.clone(),
        asset_scale: u32::from(TEST_VALIDATION_FEE_ASSET_SCALE),
        final_root: [0xB6; 32],
        next_zero_leaf_index: 1,
        topup_anchor_refs: vec![topup_anchor_ref.clone()],
        proof_step_count: 1,
        peer_hop_count: 0,
        current_note: note.clone(),
        branch_claims: vec![branch_claim.clone()],
        transition: None,
        artifact_binding: binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest().expect("valid public statement");
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = 1;
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: KagemushaRecursiveSpendOperationVectorV4 {
            limbs: operation_limbs,
        },
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope: KagemushaPastaCycleProofEnvelopeV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
                step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
                step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
                artifact_generation: binding.generation.clone(),
                manifest_sha256: binding.manifest_sha256,
                step_eq_parameter_generation: "validation-fee-eq-params".to_owned(),
                step_ep_parameter_generation: "validation-fee-ep-params".to_owned(),
                step_eq_circuit_params_sha256: [0xB7; 32],
                step_ep_circuit_params_sha256: [0xB8; 32],
                step_eq_verifier_key_sha256: [0xB9; 32],
                step_ep_verifier_key_sha256: [0xBA; 32],
                state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state_limbs)
                    .expect("valid state boundary"),
                proof: ProofBox::new(
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                    vec![0xBB],
                ),
            },
        },
    };
    let bundle_digest = bundle.digest().expect("valid recursive bundle");
    let unshield_public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
        input_commitment_0: note.note_commitment,
        input_commitment_1: [0; 32],
        nullifier_0: note.spend_nullifier,
        nullifier_1: [0; 32],
        change_output_commitment: [0; 32],
        root: [0xB6; 32],
        public_amount: kagemusha_confidential_amount_encoding_v2(amount.atomic_units),
        asset_tag: [0xBC; 32],
        network_tag: [0xBD; 32],
    };
    let redemption = KagemushaRecursiveSpendRedemptionIntentV4 {
        network_id,
        asset: asset_definition_id.clone(),
        input_note: note,
        parent_branch_claims: vec![branch_claim],
        parent_topup_anchor_refs: vec![topup_anchor_ref],
        parent_proof_step_count: 1,
        parent_peer_hop_count: 0,
        parent_bundle_digest: bundle_digest,
        input_root: [0xB6; 32],
        recipient: recipient.clone(),
        public_amount: amount,
        change_output: None,
        change_artifact_binding: None,
        unshield_public_inputs_digest: unshield_public_inputs
            .digest()
            .expect("valid unshield public inputs"),
        unshield_public_inputs,
        operation_id,
    };
    let mut redeem_proof = ProofAttachment::new_ref(
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
        ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xBE]),
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            "validation-fee-unshield",
        ),
    );
    redeem_proof.vk_commitment = Some([0xBF; 32]);
    let unsigned = KagemushaRecursiveSpendRedeemUnsignedV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        bundle,
        recipient: recipient.clone(),
        amount,
        redeem_proof,
        redemption,
        offline_change: None,
        block_height: 10,
        operation_id,
    };
    let payload_digest = unsigned.digest().expect("valid redemption payload");
    unsigned
        .into_request(kagemusha_authorization(
            recipient,
            asset_definition_id.clone(),
            operation_id,
            payload_digest,
        ))
        .expect("valid redemption request")
}
fn repo_initiate(
    agreement: &str,
    initiator: &AccountId,
    counterparty: &AccountId,
    cash_asset: &AssetDefinitionId,
    collateral_asset: &AssetDefinitionId,
) -> RepoIsi {
    RepoIsi::new(
        agreement.parse().expect("repo agreement id"),
        initiator.clone(),
        counterparty.clone(),
        None,
        RepoCashLeg {
            asset_definition_id: cash_asset.clone(),
            quantity: Quantity::from(1_u64),
        },
        RepoCollateralLeg::new(collateral_asset.clone(), Quantity::from(1_u64)),
        0,
        1_000,
        RepoGovernance::with_defaults(0, 0),
    )
}
fn repo_reverse(agreement: &str) -> ReverseRepoIsi {
    ReverseRepoIsi::new(agreement.parse().expect("repo agreement id"))
}
fn settlement_leg(
    asset_definition_id: &AssetDefinitionId,
    from: &AccountId,
    to: &AccountId,
) -> SettlementLeg {
    SettlementLeg::new(asset_definition_id.clone(), 1_u64, from.clone(), to.clone())
}
fn tx(
    authority_seed: u8,
    instructions: Vec<InstructionBox>,
    metadata: Metadata,
) -> SignedTransaction {
    let key_pair = key_pair(authority_seed);
    TransactionBuilder::new(
        validation_fee_test_network_id(),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .with_metadata(metadata)
    .sign(key_pair.private_key())
}
fn contract_call_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
    let key_pair = key_pair(authority_seed);
    TransactionBuilder::new(
        validation_fee_test_network_id(),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::ContractCall(ContractInvocation {
        contract_address: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .parse()
            .expect("contract address"),
        expected_code_hash: iroha_crypto::Hash::new(b"validation-fee-contract-code"),
        entrypoint: "send_transfer".to_owned(),
        arguments: None,
    }))
    .with_metadata(metadata)
    .sign(key_pair.private_key())
}
fn ivm_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
    let key_pair = key_pair(authority_seed);
    TransactionBuilder::new(
        validation_fee_test_network_id(),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![0x00])))
    .with_metadata(metadata)
    .sign(key_pair.private_key())
}
fn ivm_proved_tx(
    authority_seed: u8,
    overlay: Vec<InstructionBox>,
    metadata: Metadata,
) -> SignedTransaction {
    let key_pair = key_pair(authority_seed);
    TransactionBuilder::new(
        validation_fee_test_network_id(),
        AccountId::new(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(vec![0x00]),
        overlay: overlay.into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas-policy"),
    }))
    .with_metadata(metadata)
    .sign(key_pair.private_key())
}
fn metadata_for(policy: &ValidationFeePolicyV1) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY).expect("metadata key"),
        Json::new(policy.policy_version),
    );
    metadata.insert(
        Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
        Json::new(hex::encode(policy.policy_hash().expect("policy hash"))),
    );
    metadata
}
fn metadata_for_fee_instruction(
    policy: &ValidationFeePolicyV1,
    instruction_index: usize,
) -> Metadata {
    let mut metadata = metadata_for(policy);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY).expect("metadata key"),
        Json::new(u64::try_from(instruction_index).expect("instruction index fits u64")),
    );
    metadata
}
fn metadata_for_hijiri_fee_instruction(
    policy: &ValidationFeePolicyV1,
    hijiri_fee_quote_hash: [u8; 32],
    instruction_index: usize,
) -> Metadata {
    let mut metadata = metadata_for_fee_instruction(policy, instruction_index);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY).expect("metadata key"),
        Json::new(hex::encode(hijiri_fee_quote_hash)),
    );
    metadata
}
fn metadata_for_fee_instruction_coordinate(instruction_index: usize) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY).expect("metadata key"),
        Json::new(u64::try_from(instruction_index).expect("instruction index fits u64")),
    );
    metadata
}
fn metadata_for_fee_batch_entry(
    policy: &ValidationFeePolicyV1,
    instruction_index: usize,
    entry_index: usize,
) -> Metadata {
    let mut metadata = metadata_for_fee_instruction(policy, instruction_index);
    metadata.insert(
        Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
        Json::new(u64::try_from(entry_index).expect("entry index fits u64")),
    );
    metadata
}
fn with_multisig_fee_marker(
    policy: &ValidationFeePolicyV1,
    instructions: Vec<InstructionBox>,
    fee_instruction_index: usize,
    fee_entry_index: Option<usize>,
) -> Vec<InstructionBox> {
    with_multisig_fee_marker_and_hijiri(
        policy,
        None,
        instructions,
        fee_instruction_index,
        fee_entry_index,
    )
}
fn with_multisig_fee_marker_and_hijiri(
    policy: &ValidationFeePolicyV1,
    hijiri_fee_quote_hash: Option<[u8; 32]>,
    mut instructions: Vec<InstructionBox>,
    fee_instruction_index: usize,
    fee_entry_index: Option<usize>,
) -> Vec<InstructionBox> {
    instructions.push(
        ValidationFeeMultisigMarkerV1::new(
            policy.policy_version,
            policy.policy_hash().expect("policy hash"),
            hijiri_fee_quote_hash,
            u64::try_from(fee_instruction_index).expect("instruction index fits u64"),
            fee_entry_index.map(|index| u64::try_from(index).expect("entry index fits u64")),
        )
        .into_instruction(),
    );
    instructions
}
