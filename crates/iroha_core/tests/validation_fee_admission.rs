//! Integration coverage for validator admission of Parliament-enacted validation-fee policy.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{num::NonZeroU64, sync::Arc};

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateTransaction, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{
    Algorithm, Hash, KeyPair,
    blake2::{Blake2b512, Digest as _},
};
use iroha_data_model::{
    account::AccountId,
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    governance::types::{
        GovernanceFinalizationEvidence, ParliamentBodies, ParliamentBody, ParliamentRoster,
        ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    },
    isi::{
        SetParameter, Transfer, TransferAssetBatch, TransferAssetBatchEntry,
        governance::{AtWindow, EnactReferendum, VotingMode},
    },
    nexus::DataSpaceId,
    parameter::Parameter,
    prelude::*,
    smart_contract::ContractAddress,
    transaction::{Executable, IvmBytecode, IvmProved, SignedTransaction},
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
        VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY,
        VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS, ValidationFeeChargingMode,
        ValidationFeeFinalizationEvidenceV1, ValidationFeeGovernanceVotingModeV1,
        ValidationFeeGovernanceWindowV1, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePayoutLifecycleReferenceV1, ValidationFeePolicyRegistryEntryV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicyV1, ValidationFeeTreasuryPayoutBindingV1,
        ValidationFeeTreasuryPayoutRecipientV1,
    },
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec},
};
use mv::storage::StorageReadOnly;
use sha2::Sha256;

const TEST_VALIDATION_FEE_ASSET_SCALE: u8 = VALIDATION_FEE_DS_SCALE;
const TEST_VALIDATION_FEE_MINOR_UNITS: u64 = 10;
const TEST_POLICY_ENACTMENT_HEIGHT: u64 = 2;
const TEST_POLICY_EFFECTIVE_HEIGHT: u64 =
    TEST_POLICY_ENACTMENT_HEIGHT + VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;

fn quantity(mantissa: u64, scale: u32) -> Quantity {
    Quantity::try_from_numeric(Numeric::new(mantissa, scale))
        .expect("non-negative validation-fee fixture quantity")
}

fn block_header(height: u64, timestamp_ms: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("height"),
        None,
        None,
        None,
        timestamp_ms,
        0,
    )
}

fn key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair")
}

fn account(seed: u8) -> (AccountId, KeyPair) {
    let key_pair = key_pair(seed);
    (AccountId::new(key_pair.public_key().clone()), key_pair)
}

fn fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "fee_token".parse().expect("asset name"),
    )
}

fn xor_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "xor".parse().expect("asset name"),
    )
}

fn payout_contract_address() -> ContractAddress {
    ContractAddress::derive(
        iroha_config::parameters::defaults::common::chain_discriminant(),
        &account(1).0,
        42,
        DataSpaceId::UNIVERSAL,
    )
    .expect("payout contract address")
}

fn payout_contract_artifact() -> (
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
    let entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
        name: "autonomous_validation_fee_tick".to_owned(),
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        permission: Some("CanInvokeValidationFeePayout".to_owned()),
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "ValidationFeePayout".to_owned(),
        compiler_fingerprint: "validation-fee-admission-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
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
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let verified =
        ivm::verify_contract_artifact(&artifact).expect("valid payout contract artifact");
    (artifact, verified.manifest)
}

fn payout_binding(fee_asset: &AssetDefinitionId) -> ValidationFeeTreasuryPayoutBindingV1 {
    let contract_address = payout_contract_address();
    let (contract_artifact, _) = payout_contract_artifact();
    ValidationFeeTreasuryPayoutBindingV1 {
        treasury_account_id: contract_address.subject_id(),
        contract_address,
        code_hash: <[u8; 32]>::from(Sha256::digest(contract_artifact)),
        entrypoint: "autonomous_validation_fee_tick"
            .parse()
            .expect("payout entrypoint"),
        sbd_asset_id: fee_asset.clone(),
        xor_asset_id: xor_asset_definition_id(),
        pool_vault_account_id: account(2).0,
        batch_sbd: iroha_data_model::validation_fee::validation_fee_payout_batch_sbd(),
        min_xor_out: iroha_data_model::validation_fee::validation_fee_payout_min_xor(),
        max_xor_out: iroha_data_model::validation_fee::validation_fee_payout_max_xor(),
        recipients: (3..=6)
            .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                account_id: account(seed).0,
                share: iroha_data_model::validation_fee::validation_fee_payout_recipient_share(),
            })
            .collect(),
    }
}

fn test_state() -> (
    State,
    AccountId,
    KeyPair,
    AccountId,
    AccountId,
    AssetDefinitionId,
) {
    let (user, user_key_pair) = account(1);
    let (recipient, _) = account(8);
    let domain_id = DomainId::try_new("fees", "paynet").expect("domain id");
    let domain = Domain::new(domain_id).build(&user);
    let fee_asset = fee_asset_definition_id();
    let treasury = payout_contract_address().subject_id();
    let asset_definition = AssetDefinition::new(
        fee_asset.clone(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
    )
    .build(&user);
    let xor_asset_definition = AssetDefinition::new(
        xor_asset_definition_id(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
    )
    .build(&user);
    let user_asset = Asset::new(
        AssetId::new(fee_asset.clone(), user.clone()),
        Quantity::from(100_u64),
    );
    let mut accounts = vec![
        Account::new(user.clone()).build(&user),
        Account::new(recipient.clone()).build(&user),
        Account::new(treasury.clone()).build(&user),
    ];
    accounts.extend((2..=6).map(|seed| Account::new(account(seed).0).build(&user)));
    let state = State::new_for_testing(
        World::with_assets(
            [domain],
            accounts,
            [asset_definition, xor_asset_definition],
            [user_asset],
            [],
        ),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    (state, user, user_key_pair, recipient, treasury, fee_asset)
}

fn accept_transaction(state: &State, tx: SignedTransaction) -> AcceptedTransaction<'static> {
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();
    AcceptedTransaction::accept(
        tx,
        &state.chain_id,
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    )
    .expect("transaction admission should pass stateless checks")
}

fn commit_empty_genesis_like_block(state: &State) -> [u8; 32] {
    let block_signer = key_pair(240);
    let new_block = BlockBuilder::new(Vec::new())
        .chain(0, None)
        .sign(block_signer.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(new_block.header());
    let valid_block =
        ValidBlock::validate_unchecked(new_block.into(), &mut state_block).unpack(|_| {});
    let committed_block = valid_block.commit_unchecked().unpack(|_| {});
    let genesis_hash = committed_block.as_ref().hash();
    let _events = state_block.apply_without_execution(&committed_block, Vec::new());
    state_block.commit().expect("commit initial block hash");
    *genesis_hash.as_ref()
}

fn validation_fee_policy(
    state: &State,
    fee_asset: AssetDefinitionId,
    treasury: AccountId,
    genesis_hash: [u8; 32],
) -> ValidationFeePolicyV1 {
    let payout_binding = payout_binding(&fee_asset);
    assert_eq!(treasury, payout_binding.treasury_account_id);
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        chain_id: state.chain_id.clone(),
        genesis_hash,
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: fee_asset,
        ds_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
        fee: iroha_data_model::validation_fee::initial_validation_fee_amount(),
        treasury_account_id: treasury,
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: TEST_POLICY_EFFECTIVE_HEIGHT,
        expires_after_height: Some(TEST_POLICY_EFFECTIVE_HEIGHT + 100),
        exemption_classes: vec![VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_owned()],
        treasury_payout_binding: Some(payout_binding),
    }
}

fn test_parliament_bodies() -> ParliamentBodies {
    let member = account(250).0;
    let rosters = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ]
    .into_iter()
    .map(|body| {
        (
            body,
            ParliamentRoster {
                body,
                epoch: 1,
                members: vec![member.clone()],
                alternates: Vec::new(),
                verified: 1,
                candidate_count: 1,
                derived_by: Default::default(),
            },
        )
    })
    .collect();
    ParliamentBodies {
        selection_epoch: 1,
        rosters,
    }
}

fn test_roster_root() -> [u8; 32] {
    let encoded = norito::to_bytes(&test_parliament_bodies()).expect("encode Parliament bodies");
    let digest = Blake2b512::digest(encoded);
    let mut root = [0; 32];
    root.copy_from_slice(&digest[..32]);
    root
}

fn test_parliament_authorization(proposal_id: [u8; 32]) -> ValidationFeeParliamentAuthorizationV1 {
    ValidationFeeParliamentAuthorizationV1 {
        proposal_id,
        proposal_fingerprint: proposal_id,
        proposal_time_roster_root: test_roster_root(),
        referendum_window: ValidationFeeGovernanceWindowV1 {
            lower: 1,
            upper: TEST_POLICY_ENACTMENT_HEIGHT,
        },
        finalization: ValidationFeeFinalizationEvidenceV1 {
            referendum_id: proposal_id,
            finalized_at_height: 1,
            mode: ValidationFeeGovernanceVotingModeV1::Plain,
            approve: 1,
            reject: 0,
            abstain: 0,
            min_turnout: 1,
            approval_threshold_numerator: 1,
            approval_threshold_denominator: 2,
            approved: true,
        },
        enacted_at_height: TEST_POLICY_ENACTMENT_HEIGHT,
    }
}

fn policy_treasury_account(policy: &ValidationFeePolicyV1) -> AccountId {
    policy.treasury_account_id.clone()
}

fn payout_lifecycle_proposal(policy: &ValidationFeePolicyV1) -> ProposalKind {
    ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
        payout_binding: policy
            .treasury_payout_binding
            .clone()
            .expect("enabled policy must carry its exact payout binding"),
    })
}

fn payout_lifecycle_proposal_id(policy: &ValidationFeePolicyV1) -> [u8; 32] {
    payout_lifecycle_proposal(policy).fingerprint()
}

fn policy_proposal(policy: &ValidationFeePolicyV1) -> ProposalKind {
    ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        policy: policy.clone(),
        payout_lifecycle_proposal_id: Some(payout_lifecycle_proposal_id(policy)),
    })
}

fn policy_registry(policy: &ValidationFeePolicyV1) -> ValidationFeePolicyRegistryV1 {
    let lifecycle_id = payout_lifecycle_proposal_id(policy);
    let lifecycle_seal = policy
        .treasury_payout_binding
        .as_ref()
        .expect("enabled policy must carry its exact payout binding")
        .lifecycle_seal()
        .expect("derive payout lifecycle seal");
    let proposal = policy_proposal(policy);
    let proposal_id = proposal.fingerprint();
    let entry = ValidationFeePolicyRegistryEntryV1::from_enactment(
        policy.clone(),
        test_parliament_authorization(proposal_id),
        Some(ValidationFeePayoutLifecycleReferenceV1 {
            lifecycle_seal,
            parliament_authorization: test_parliament_authorization(lifecycle_id),
        }),
    )
    .expect("registry entry");
    ValidationFeePolicyRegistryV1 {
        registered_policies: vec![entry],
    }
}

fn seed_approved_proposal(
    kind: ProposalKind,
    proposer: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> ([u8; 32], AtWindow) {
    let proposal_id = kind.fingerprint();
    let referendum_id = hex::encode(proposal_id);
    let window = AtWindow {
        lower: 1,
        upper: TEST_POLICY_ENACTMENT_HEIGHT,
    };
    let bodies = test_parliament_bodies();
    state_transaction.world.governance_proposals_mut().insert(
        proposal_id,
        iroha_core::state::GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind,
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Approved,
            pipeline: iroha_core::state::GovernancePipeline::default(),
            parliament_snapshot: Some(iroha_core::state::GovernanceParliamentSnapshot {
                selection_epoch: 1,
                beacon: [0x44; 32],
                roster_root: test_roster_root(),
                bodies,
            }),
            finalization_evidence: Some(GovernanceFinalizationEvidence {
                proposal_id,
                referendum_id: proposal_id,
                finalized_at_height: 1,
                mode: VotingMode::Plain,
                approve: 1,
                reject: 0,
                abstain: 0,
                min_turnout: 1,
                approval_threshold_numerator: 1,
                approval_threshold_denominator: 2,
                approved: true,
            }),
            enacted_at_height: None,
        },
    );
    state_transaction.world.governance_referenda_mut().insert(
        referendum_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: window.lower,
            h_end: window.upper,
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
    let mut approvals = iroha_core::state::GovernanceStageApprovals::default();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        approvals
            .ensure_stage(body, 1, 1, 10_000)
            .record(account(250).0);
    }
    state_transaction
        .world
        .governance_stage_approvals_mut()
        .insert(referendum_id, approvals);
    (proposal_id, window)
}

fn install_validation_fee_policy(
    state: &State,
    authority: &AccountId,
    authority_key_pair: &KeyPair,
    policy: ValidationFeePolicyV1,
) {
    let mut block = state.block(block_header(
        TEST_POLICY_ENACTMENT_HEIGHT,
        1_700_000_001_000,
    ));
    let mut stx = block.transaction();
    let register_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(register_permission, authority.clone())
        .execute(authority, &mut stx)
        .expect("grant payout-contract registration authority");
    let (contract_artifact, contract_manifest) = payout_contract_artifact();
    let registered_code_hash = iroha_core::smartcontracts::code::register_code_bytes(
        authority,
        contract_artifact,
        &mut stx,
    )
    .expect("register payout-contract bytes");
    iroha_core::smartcontracts::code::register_manifest(
        authority,
        contract_manifest.signed(authority_key_pair),
        &mut stx,
    )
    .expect("register signed payout-contract manifest");
    iroha_core::smartcontracts::code::activate_instance(
        authority,
        payout_contract_address(),
        registered_code_hash,
        &mut stx,
    )
    .expect("activate immutable payout-contract subject");
    let (lifecycle_id, lifecycle_window) =
        seed_approved_proposal(payout_lifecycle_proposal(&policy), authority, &mut stx);
    EnactReferendum {
        referendum_id: lifecycle_id,
        preimage_hash: lifecycle_id,
        at_window: lifecycle_window,
    }
    .execute(authority, &mut stx)
    .expect("enact Parliament-approved validation-fee payout lifecycle");
    let (proposal_id, window) =
        seed_approved_proposal(policy_proposal(&policy), authority, &mut stx);
    EnactReferendum {
        referendum_id: proposal_id,
        preimage_hash: proposal_id,
        at_window: window,
    }
    .execute(authority, &mut stx)
    .expect("enact Parliament-approved validation-fee policy");
    stx.apply();
    block.commit().expect("commit validation-fee policy");
}

fn metadata_for_policy(policy: &ValidationFeePolicyV1, fee_instruction_index: usize) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(policy.policy_version),
    );
    metadata.insert(
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(hex::encode(policy.policy_hash().expect("policy hash"))),
    );
    metadata.insert(
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(u64::try_from(fee_instruction_index).expect("instruction index fits")),
    );
    metadata
}

fn metadata_for_batch_policy(
    policy: &ValidationFeePolicyV1,
    fee_instruction_index: usize,
    fee_entry_index: usize,
) -> Metadata {
    let mut metadata = metadata_for_policy(policy, fee_instruction_index);
    metadata.insert(
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(u64::try_from(fee_entry_index).expect("entry index fits")),
    );
    metadata
}

fn signed_transfer(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    include_fee: bool,
) -> SignedTransaction {
    let metadata = if include_fee {
        metadata_for_policy(policy, 1)
    } else {
        Metadata::default()
    };
    signed_transfer_with_metadata(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        policy,
        include_fee,
        metadata,
    )
}

fn signed_transfer_with_metadata(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    include_fee: bool,
    metadata: Metadata,
) -> SignedTransaction {
    let fee_instruction =
        include_fee.then(|| (policy.fee.clone(), policy_treasury_account(policy)));
    signed_transfer_with_fee_instruction(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        fee_instruction,
        metadata,
    )
}

fn signed_transfer_with_fee_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    fee_instruction: Option<(Quantity, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    signed_transfer_with_principal_and_fee_instruction(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        Quantity::from(1_u32),
        fee_instruction,
        metadata,
    )
}

fn signed_transfer_with_principal_and_fee_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    principal_amount: Quantity,
    fee_instruction: Option<(Quantity, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    let principal = Transfer::asset_quantity(
        AssetId::new(fee_asset.clone(), user.clone()),
        principal_amount,
        recipient.clone(),
    );
    let mut instructions: Vec<InstructionBox> = vec![principal.into()];
    if let Some((fee_amount, fee_recipient)) = fee_instruction {
        instructions.push(
            Transfer::asset_quantity(
                AssetId::new(fee_asset.clone(), user.clone()),
                fee_amount,
                fee_recipient,
            )
            .into(),
        );
    }
    TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}

fn signed_ivm_proved_overlay(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    overlay: Vec<InstructionBox>,
    metadata: Metadata,
) -> SignedTransaction {
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

    TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1_000)),
    )
    .with_executable(Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(program),
        overlay: overlay.into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas-policy"),
    }))
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}

fn signed_transfer_with_explicit_fee_asset_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    principal_asset: &AssetDefinitionId,
    fee_asset: &AssetDefinitionId,
    fee_amount: Quantity,
    fee_recipient: AccountId,
    metadata: Metadata,
) -> SignedTransaction {
    TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(principal_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            fee_amount,
            fee_recipient,
        )),
    ])
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}

fn signed_transfer_with_explicit_fee_source_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    fee_source: &AccountId,
    fee_amount: Quantity,
    fee_recipient: AccountId,
    metadata: Metadata,
) -> SignedTransaction {
    TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), fee_source.clone()),
            fee_amount,
            fee_recipient,
        )),
    ])
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}

fn signed_batch_transfer_with_principal_amounts(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    first_principal_amount: Quantity,
    second_principal_amount: Quantity,
) -> SignedTransaction {
    signed_batch_transfer_with_entries(
        state,
        user,
        user_key_pair,
        policy,
        vec![
            TransferAssetBatchEntry::new(
                user.clone(),
                recipient.clone(),
                fee_asset.clone(),
                first_principal_amount,
            ),
            TransferAssetBatchEntry::new(
                user.clone(),
                recipient.clone(),
                fee_asset.clone(),
                second_principal_amount,
            ),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(policy),
                fee_asset.clone(),
                quantity(
                    2 * TEST_VALIDATION_FEE_MINOR_UNITS,
                    TEST_VALIDATION_FEE_ASSET_SCALE.into(),
                ),
            ),
        ],
    )
}

fn signed_batch_transfer_with_entries(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    policy: &ValidationFeePolicyV1,
    entries: Vec<TransferAssetBatchEntry>,
) -> SignedTransaction {
    let batch = TransferAssetBatch::new(entries);
    TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(batch)])
    .with_metadata(metadata_for_batch_policy(policy, 0, 2))
    .sign(user_key_pair.private_key())
}

fn validate_in_block(state: &State, height: u64, tx: SignedTransaction) -> String {
    let accepted = accept_transaction(state, tx);
    let mut block = state.block(block_header(height, 1_700_000_002_000 + height));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("{error:?}"),
    }
}

fn accept_transaction_error(state: &State, tx: SignedTransaction) -> String {
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();
    match AcceptedTransaction::accept(
        tx,
        &state.chain_id,
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    ) {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("{error:?}"),
    }
}

fn asset_balance(world: &impl WorldReadOnly, asset_id: &AssetId) -> Quantity {
    world
        .assets()
        .get(asset_id)
        .map_or_else(Quantity::zero, |value| value.clone().into_inner())
}

#[test]
fn raw_fee_asset_transfer_is_rejected_without_exact_active_validation_fee() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury, genesis_hash);
    install_validation_fee_policy(&state, &user, &user_key_pair, policy.clone());

    let missing_fee_error = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &fee_asset,
            &policy,
            false,
        ),
    );
    assert!(
        missing_fee_error.contains("missing validation-fee transfer of 10 minor units"),
        "unexpected missing-fee rejection: {missing_fee_error}"
    );

    let exact_fee_result = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &fee_asset,
            &policy,
            true,
        ),
    );
    assert_eq!(exact_fee_result, "ok");
}

#[test]
fn validation_fee_registry_cannot_be_installed_through_generic_parameter_path() {
    let (state, user, _, _, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let policy = validation_fee_policy(&state, fee_asset, treasury, genesis_hash);
    let custom = policy_registry(&policy).into_custom_parameter();
    let mut block = state.block(block_header(
        TEST_POLICY_ENACTMENT_HEIGHT,
        1_700_000_001_000,
    ));
    let mut state_transaction = block.transaction();
    let error = SetParameter::new(Parameter::Custom(custom))
        .execute(&user, &mut state_transaction)
        .expect_err("generic parameter writes must not bypass Parliament");
    let error_debug = format!("{error:?}");
    assert!(
        error_debug.contains("can only be changed by an enacted SORA Parliament proposal"),
        "unexpected protected-registry rejection: {error_debug}"
    );
}

#[test]
fn ivm_proved_overlay_reaches_active_validation_fee_admission() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury.clone(), genesis_hash);
    install_validation_fee_policy(&state, &user, &user_key_pair, policy.clone());

    let principal = || {
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        ))
    };
    let fee = || {
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            policy.fee.clone(),
            treasury.clone(),
        ))
    };

    let missing_fee_error = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT,
        signed_ivm_proved_overlay(
            &state,
            &user,
            &user_key_pair,
            vec![principal()],
            Metadata::default(),
        ),
    );
    assert!(
        missing_fee_error.contains("missing validation-fee transfer of 10 minor units"),
        "unexpected proved-IVM missing-fee rejection: {missing_fee_error}"
    );

    let exact_fee_result = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        signed_ivm_proved_overlay(
            &state,
            &user,
            &user_key_pair,
            vec![principal(), fee()],
            metadata_for_policy(&policy, 1),
        ),
    );
    assert!(
        !exact_fee_result.contains("validation-fee admission rejected transaction")
            && !exact_fee_result.contains("UnsupportedExecutable"),
        "exact proved-IVM overlay fee must pass validation-fee admission: {exact_fee_result}"
    );
}

#[test]
fn principal_and_fee_commit_atomically_under_active_validation_fee_policy() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury.clone(), genesis_hash);
    install_validation_fee_policy(&state, &user, &user_key_pair, policy.clone());

    let recipient_asset = AssetId::new(fee_asset.clone(), recipient.clone());
    let treasury_asset = AssetId::new(fee_asset.clone(), treasury.clone());
    let missing_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        false,
    );
    let accepted = accept_transaction(&state, missing_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT,
        1_700_000_003_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "missing fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must not commit when validation-fee admission fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "treasury must not be credited by a transaction rejected before execution"
    );
    drop(view);

    let underpaid_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Quantity::from(1_u32),
        Some((
            quantity(
                TEST_VALIDATION_FEE_MINOR_UNITS - 1,
                TEST_VALIDATION_FEE_ASSET_SCALE.into(),
            ),
            policy_treasury_account(&policy),
        )),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, underpaid_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        1_700_000_004_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "underpaid fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must not commit when the fee amount is wrong"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "wrong fee amount must not credit the treasury"
    );
    drop(view);

    let fee_then_overdrawn_principal_tx = TransactionBuilder::new(
        state.chain_id.clone(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            policy.fee.clone(),
            policy_treasury_account(&policy),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            100_u32,
            recipient.clone(),
        )),
    ])
    .with_metadata(metadata_for_policy(&policy, 0))
    .sign(user_key_pair.private_key());
    let accepted = accept_transaction(&state, fee_then_overdrawn_principal_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 2,
        1_700_000_005_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_err(),
        "overdrawn principal after fee execution must reject"
    );
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "recipient must not be credited by a rejected transaction"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "fee transfer must roll back when the later principal transfer fails"
    );
    drop(view);

    let principal_then_overdrawn_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        quantity(9_995, TEST_VALIDATION_FEE_ASSET_SCALE.into()),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, principal_then_overdrawn_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 3,
        1_700_000_006_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_err(),
        "overdrawn fee after principal execution must reject"
    );
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must roll back when the later fee transfer fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "treasury must not be credited by a rejected transaction"
    );
    drop(view);

    let exact_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
    );
    let accepted = accept_transaction(&state, exact_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 4,
        1_700_000_007_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert_eq!(result, Ok(Vec::new()));
    block
        .commit()
        .expect("commit exact validation-fee transfer");
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::from(1_u64),
        "principal transfer must commit with the exact fee"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        policy.fee.clone(),
        "fee transfer must commit with the principal transfer"
    );
}

#[test]
fn fee_instruction_policy_hash_amount_and_treasury_are_covered_by_user_signature() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury, genesis_hash);
    install_validation_fee_policy(&state, &user, &user_key_pair, policy.clone());

    let mut exact_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
    );
    let exact_fee_result = validate_in_block(&state, 3, exact_fee_tx.clone());
    assert_eq!(exact_fee_result, "ok");

    let mut wrong_policy_hash_metadata = metadata_for_policy(&policy, 1);
    wrong_policy_hash_metadata.insert(
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(hex::encode([0x55u8; 32])),
    );
    let wrong_policy_hash_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        wrong_policy_hash_metadata,
    );

    let mut policy_hash_mutation_tx = exact_fee_tx.clone();
    policy_hash_mutation_tx.set_signature(wrong_policy_hash_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, policy_hash_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "policy-hash payload mutation must fail signature admission, got {signature_error}"
    );

    let mut wrong_policy_version_metadata = metadata_for_policy(&policy, 1);
    wrong_policy_version_metadata.insert(
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(policy.policy_version + 1),
    );
    let wrong_policy_version_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        wrong_policy_version_metadata,
    );
    let mut policy_version_mutation_tx = exact_fee_tx.clone();
    policy_version_mutation_tx.set_signature(wrong_policy_version_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, policy_version_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "policy-version payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_fee_coordinate_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        metadata_for_policy(&policy, 0),
    );
    let mut fee_coordinate_mutation_tx = exact_fee_tx.clone();
    fee_coordinate_mutation_tx.set_signature(wrong_fee_coordinate_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_coordinate_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-coordinate payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_principal_amount_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Quantity::from(2_u32),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let mut principal_amount_mutation_tx = exact_fee_tx.clone();
    principal_amount_mutation_tx.set_signature(wrong_principal_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, principal_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "principal-amount payload mutation must fail signature admission, got {signature_error}"
    );

    let (alternate_recipient, _) = account(4);
    let wrong_principal_recipient_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &alternate_recipient,
        &fee_asset,
        Quantity::from(1_u32),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let mut principal_recipient_mutation_tx = exact_fee_tx.clone();
    principal_recipient_mutation_tx.set_signature(wrong_principal_recipient_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, principal_recipient_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "principal-recipient payload mutation must fail signature admission, got {signature_error}"
    );

    let exact_batch_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(1_u64),
    );
    let exact_batch_result = accept_transaction_error(&state, exact_batch_tx.clone());
    assert_eq!(exact_batch_result, "ok");

    let wrong_batch_principal_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(2_u64),
    );
    let mut batch_principal_mutation_tx = exact_batch_tx.clone();
    batch_principal_mutation_tx.set_signature(wrong_batch_principal_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_principal_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-principal payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_batch_source_tx = signed_batch_transfer_with_entries(
        &state,
        &user,
        &user_key_pair,
        &policy,
        vec![
            TransferAssetBatchEntry::new(
                recipient.clone(),
                recipient.clone(),
                fee_asset.clone(),
                1_u32,
            ),
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), fee_asset.clone(), 1_u32),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(&policy),
                fee_asset.clone(),
                quantity(
                    2 * TEST_VALIDATION_FEE_MINOR_UNITS,
                    TEST_VALIDATION_FEE_ASSET_SCALE.into(),
                ),
            ),
        ],
    );
    let mut batch_source_mutation_tx = exact_batch_tx.clone();
    batch_source_mutation_tx.set_signature(wrong_batch_source_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_source_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-source payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_batch_amount_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(2_u64),
        Quantity::from(1_u64),
    );
    let mut batch_amount_mutation_tx = exact_batch_tx.clone();
    batch_amount_mutation_tx.set_signature(wrong_batch_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-amount payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_batch_asset = AssetDefinitionId::new(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "wrong_batch_token".parse().expect("asset name"),
    );
    let wrong_batch_asset_tx = signed_batch_transfer_with_entries(
        &state,
        &user,
        &user_key_pair,
        &policy,
        vec![
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), wrong_batch_asset, 1_u32),
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), fee_asset.clone(), 1_u32),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(&policy),
                fee_asset.clone(),
                quantity(
                    2 * TEST_VALIDATION_FEE_MINOR_UNITS,
                    TEST_VALIDATION_FEE_ASSET_SCALE.into(),
                ),
            ),
        ],
    );
    let mut batch_asset_mutation_tx = exact_batch_tx.clone();
    batch_asset_mutation_tx.set_signature(wrong_batch_asset_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_asset_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-asset payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_batch_recipient_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &alternate_recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(1_u64),
    );
    let mut batch_recipient_mutation_tx = exact_batch_tx;
    batch_recipient_mutation_tx.set_signature(wrong_batch_recipient_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_recipient_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-recipient payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_fee_amount_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((
            quantity(
                TEST_VALIDATION_FEE_MINOR_UNITS + 1,
                TEST_VALIDATION_FEE_ASSET_SCALE.into(),
            ),
            policy_treasury_account(&policy),
        )),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_amount_mutation_tx = exact_fee_tx.clone();
    fee_amount_mutation_tx.set_signature(wrong_fee_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-amount payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_fee_asset = AssetDefinitionId::new(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "wrong_fee_token".parse().expect("asset name"),
    );
    let wrong_fee_asset_tx = signed_transfer_with_explicit_fee_asset_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &wrong_fee_asset,
        policy.fee.clone(),
        policy_treasury_account(&policy),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_asset_mutation_tx = exact_fee_tx.clone();
    fee_asset_mutation_tx.set_signature(wrong_fee_asset_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_asset_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-asset payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_fee_source_tx = signed_transfer_with_explicit_fee_source_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &recipient,
        policy.fee.clone(),
        policy_treasury_account(&policy),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_source_mutation_tx = exact_fee_tx.clone();
    fee_source_mutation_tx.set_signature(wrong_fee_source_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_source_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-source payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_treasury_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((policy.fee.clone(), recipient.clone())),
        metadata_for_policy(&policy, 1),
    );
    exact_fee_tx.set_signature(wrong_treasury_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, exact_fee_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-treasury payload mutation must fail signature admission, got {signature_error}"
    );
}
