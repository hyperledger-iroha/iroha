//! Integration coverage for validator admission of signed validation-fee policy.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::num::NonZeroU64;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    isi::{SetParameter, Transfer, TransferAssetBatch, TransferAssetBatchEntry},
    parameter::Parameter,
    prelude::*,
    transaction::SignedTransaction,
    validation_fee::{
        SignedValidationFeePolicyV1, VALIDATION_FEE_INITIAL_MINOR_UNITS,
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_SBD_SCALE, VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY,
        ValidationFeeChargingMode, ValidationFeeGovernanceKeyV1, ValidationFeeGovernanceKeysetV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1,
        ValidationFeePolicySignatureV1, ValidationFeePolicyV1,
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};
use mv::storage::StorageReadOnly;

const TEST_VALIDATION_FEE_ASSET_SCALE: u8 = VALIDATION_FEE_SBD_SCALE;
const TEST_VALIDATION_FEE_MINOR_UNITS: u64 = VALIDATION_FEE_INITIAL_MINOR_UNITS;

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

fn test_state() -> (
    State,
    AccountId,
    KeyPair,
    AccountId,
    AccountId,
    AssetDefinitionId,
) {
    let (user, user_key_pair) = account(1);
    let (recipient, _) = account(2);
    let (treasury, _) = account(3);
    let domain_id = DomainId::try_new("fees", "paynet").expect("domain id");
    let domain = Domain::new(domain_id).build(&user);
    let fee_asset = fee_asset_definition_id();
    let asset_definition = AssetDefinition::numeric(fee_asset.clone()).build(&user);
    let user_asset = Asset::new(
        AssetId::new(fee_asset.clone(), user.clone()),
        Numeric::new(100, 0),
    );
    let state = State::new_for_testing(
        World::with_assets(
            [domain],
            [
                Account::new(user.clone()).build(&user),
                Account::new(recipient.clone()).build(&user),
                Account::new(treasury.clone()).build(&user),
            ],
            [asset_definition],
            [user_asset],
            [],
        ),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
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
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: state.chain_id.to_string(),
        genesis_hash,
        policy_version: 1,
        previous_policy_hash: None,
        sbd_asset_id: fee_asset.to_string(),
        sbd_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
        fee_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
        treasury_account_id: treasury.to_string(),
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: 3,
        expires_after_height: Some(100),
        governance_keyset_id: "validation-fee-governance-v1".to_string(),
        exemption_classes: Vec::new(),
    }
}

fn signed_policy(
    policy: ValidationFeePolicyV1,
    key_pairs: &[&KeyPair],
) -> SignedValidationFeePolicyV1 {
    SignedValidationFeePolicyV1 {
        signatures: key_pairs
            .iter()
            .map(|key_pair| ValidationFeePolicySignatureV1 {
                public_key: key_pair.public_key().clone(),
                signature: SignatureOf::try_new(key_pair.private_key(), &policy.signing_payload())
                    .expect("policy signature"),
            })
            .collect(),
        policy,
    }
}

fn policy_registry(policy: &ValidationFeePolicyV1) -> ValidationFeePolicyRegistryV1 {
    let entry = ValidationFeePolicyRegistryEntryV1::from_policy(policy).expect("registry entry");
    ValidationFeePolicyRegistryV1 {
        active_policy_hash: entry.policy_hash,
        active_policy_version: entry.policy_version,
        registered_policies: vec![entry],
    }
}

fn policy_treasury_account(policy: &ValidationFeePolicyV1) -> AccountId {
    AccountId::parse_encoded(policy.treasury_account_id.as_str())
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("policy treasury account id")
}

fn install_validation_fee_policy(
    state: &State,
    authority: &AccountId,
    policy: ValidationFeePolicyV1,
    key_pairs: &[&KeyPair],
) {
    let keyset = ValidationFeeGovernanceKeysetV1 {
        keyset_id: policy.governance_keyset_id.clone(),
        threshold: u16::try_from(key_pairs.len()).expect("threshold fits"),
        keys: key_pairs
            .iter()
            .map(|key_pair| ValidationFeeGovernanceKeyV1 {
                public_key: key_pair.public_key().clone(),
                weight: 1,
            })
            .collect(),
    };
    let signed = signed_policy(policy.clone(), key_pairs);
    let registry = policy_registry(&policy);
    let mut block = state.block(block_header(2, 1_700_000_001_000));
    let mut stx = block.transaction();
    for custom in [
        keyset.into_custom_parameter(),
        registry.into_custom_parameter(),
        signed.into_custom_parameter(),
    ] {
        SetParameter::new(Parameter::Custom(custom))
            .execute(authority, &mut stx)
            .expect("install validation-fee custom parameter");
    }
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
        include_fee.then(|| (policy.fee_amount_numeric(), policy_treasury_account(policy)));
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
    fee_instruction: Option<(Numeric, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    signed_transfer_with_principal_and_fee_instruction(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        Numeric::new(1, 0),
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
    principal_amount: Numeric,
    fee_instruction: Option<(Numeric, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    let principal = Transfer::asset_numeric(
        AssetId::new(fee_asset.clone(), user.clone()),
        principal_amount,
        recipient.clone(),
    );
    let mut instructions: Vec<InstructionBox> = vec![principal.into()];
    if let Some((fee_amount, fee_recipient)) = fee_instruction {
        instructions.push(
            Transfer::asset_numeric(
                AssetId::new(fee_asset.clone(), user.clone()),
                fee_amount,
                fee_recipient,
            )
            .into(),
        );
    }
    TransactionBuilder::new(state.chain_id.clone(), user.clone())
        .with_instructions(instructions)
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
    first_principal_amount: Numeric,
    second_principal_amount: Numeric,
) -> SignedTransaction {
    let batch = TransferAssetBatch::new(vec![
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
            Numeric::new(
                2 * TEST_VALIDATION_FEE_MINOR_UNITS,
                TEST_VALIDATION_FEE_ASSET_SCALE.into(),
            ),
        ),
    ]);
    TransactionBuilder::new(state.chain_id.clone(), user.clone())
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

fn asset_balance(world: &impl WorldReadOnly, asset_id: &AssetId) -> Numeric {
    world
        .assets()
        .get(asset_id)
        .map_or_else(Numeric::zero, |value| value.clone().into_inner())
}

#[test]
fn raw_fee_asset_transfer_is_rejected_without_exact_active_validation_fee() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let gov_1 = key_pair(21);
    let gov_2 = key_pair(22);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury, genesis_hash);
    install_validation_fee_policy(&state, &user, policy.clone(), &[&gov_1, &gov_2]);

    let missing_fee_error = validate_in_block(
        &state,
        3,
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
        4,
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
fn principal_and_fee_commit_atomically_under_active_validation_fee_policy() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let gov_1 = key_pair(21);
    let gov_2 = key_pair(22);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury.clone(), genesis_hash);
    install_validation_fee_policy(&state, &user, policy.clone(), &[&gov_1, &gov_2]);

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
    let mut block = state.block(block_header(3, 1_700_000_003_000));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "missing fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Numeric::zero(),
        "principal transfer must not commit when validation-fee admission fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Numeric::zero(),
        "treasury must not be credited by a transaction rejected before execution"
    );
    drop(view);

    let underpaid_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Numeric::new(1, 0),
        Some((
            Numeric::new(
                TEST_VALIDATION_FEE_MINOR_UNITS - 1,
                TEST_VALIDATION_FEE_ASSET_SCALE.into(),
            ),
            policy_treasury_account(&policy),
        )),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, underpaid_fee_tx);
    let mut block = state.block(block_header(4, 1_700_000_004_000));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "underpaid fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Numeric::zero(),
        "principal transfer must not commit when the fee amount is wrong"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Numeric::zero(),
        "wrong fee amount must not credit the treasury"
    );
    drop(view);

    let fee_then_overdrawn_principal_tx =
        TransactionBuilder::new(state.chain_id.clone(), user.clone())
            .with_instructions([
                InstructionBox::from(Transfer::asset_numeric(
                    AssetId::new(fee_asset.clone(), user.clone()),
                    policy.fee_amount_numeric(),
                    policy_treasury_account(&policy),
                )),
                InstructionBox::from(Transfer::asset_numeric(
                    AssetId::new(fee_asset.clone(), user.clone()),
                    Numeric::new(100, 0),
                    recipient.clone(),
                )),
            ])
            .with_metadata(metadata_for_policy(&policy, 0))
            .sign(user_key_pair.private_key());
    let accepted = accept_transaction(&state, fee_then_overdrawn_principal_tx);
    let mut block = state.block(block_header(5, 1_700_000_005_000));
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
        Numeric::zero(),
        "recipient must not be credited by a rejected transaction"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Numeric::zero(),
        "fee transfer must roll back when the later principal transfer fails"
    );
    drop(view);

    let principal_then_overdrawn_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Numeric::new(9_995, TEST_VALIDATION_FEE_ASSET_SCALE.into()),
        Some((
            policy.fee_amount_numeric(),
            policy_treasury_account(&policy),
        )),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, principal_then_overdrawn_fee_tx);
    let mut block = state.block(block_header(6, 1_700_000_006_000));
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
        Numeric::zero(),
        "principal transfer must roll back when the later fee transfer fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Numeric::zero(),
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
    let mut block = state.block(block_header(7, 1_700_000_007_000));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert_eq!(result, Ok(Vec::new()));
    block
        .commit()
        .expect("commit exact validation-fee transfer");
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Numeric::new(1, 0),
        "principal transfer must commit with the exact fee"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        policy.fee_amount_numeric(),
        "fee transfer must commit with the principal transfer"
    );
}

#[test]
fn fee_instruction_policy_hash_amount_and_treasury_are_covered_by_user_signature() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let gov_1 = key_pair(21);
    let gov_2 = key_pair(22);
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury, genesis_hash);
    install_validation_fee_policy(&state, &user, policy.clone(), &[&gov_1, &gov_2]);

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

    let wrong_principal_amount_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Numeric::new(2, 0),
        Some((
            policy.fee_amount_numeric(),
            policy_treasury_account(&policy),
        )),
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

    let exact_batch_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Numeric::new(1, 0),
        Numeric::new(1, 0),
    );
    let exact_batch_result = validate_in_block(&state, 4, exact_batch_tx.clone());
    assert_eq!(exact_batch_result, "ok");

    let wrong_batch_principal_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Numeric::new(1, 0),
        Numeric::new(2, 0),
    );
    let mut batch_principal_mutation_tx = exact_batch_tx;
    batch_principal_mutation_tx.set_signature(wrong_batch_principal_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_principal_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-principal payload mutation must fail signature admission, got {signature_error}"
    );

    let wrong_fee_amount_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((
            Numeric::new(
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

    let wrong_treasury_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((policy.fee_amount_numeric(), recipient.clone())),
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
