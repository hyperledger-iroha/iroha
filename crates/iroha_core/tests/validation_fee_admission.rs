//! Integration coverage for validator admission of signed SBD validation-fee policy.
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
    isi::{SetParameter, Transfer},
    parameter::Parameter,
    prelude::*,
    transaction::SignedTransaction,
    validation_fee::{
        SignedValidationFeePolicyV1, VALIDATION_FEE_INITIAL_MINOR_UNITS,
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_SBD_SCALE, ValidationFeeChargingMode, ValidationFeeGovernanceKeyV1,
        ValidationFeeGovernanceKeysetV1, ValidationFeePolicyRegistryEntryV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicySignatureV1, ValidationFeePolicyV1,
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};

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

fn sbd_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("cbsi", "universal").expect("domain id"),
        "sbd".parse().expect("asset name"),
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
    let domain_id = DomainId::try_new("cbsi", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&user);
    let sbd = sbd_asset_definition_id();
    let asset_definition = AssetDefinition::numeric(sbd.clone()).build(&user);
    let user_asset = Asset::new(
        AssetId::new(sbd.clone(), user.clone()),
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
    (state, user, user_key_pair, recipient, treasury, sbd)
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
    sbd: AssetDefinitionId,
    treasury: AccountId,
    genesis_hash: [u8; 32],
) -> ValidationFeePolicyV1 {
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: state.chain_id.to_string(),
        genesis_hash,
        policy_version: 1,
        previous_policy_hash: None,
        sbd_asset_definition_id: sbd,
        sbd_scale: VALIDATION_FEE_SBD_SCALE,
        fee_minor_units: VALIDATION_FEE_INITIAL_MINOR_UNITS,
        treasury_account_id: treasury,
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

fn signed_transfer(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    sbd: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    include_fee: bool,
) -> SignedTransaction {
    let principal = Transfer::asset_numeric(
        AssetId::new(sbd.clone(), user.clone()),
        Numeric::new(1, 0),
        recipient.clone(),
    );
    let mut instructions: Vec<InstructionBox> = vec![principal.into()];
    if include_fee {
        instructions.push(
            Transfer::asset_numeric(
                AssetId::new(sbd.clone(), user.clone()),
                Numeric::new(
                    VALIDATION_FEE_INITIAL_MINOR_UNITS,
                    u32::from(VALIDATION_FEE_SBD_SCALE),
                ),
                policy.treasury_account_id.clone(),
            )
            .into(),
        );
    }
    let metadata = if include_fee {
        metadata_for_policy(policy, 1)
    } else {
        Metadata::default()
    };
    TransactionBuilder::new(state.chain_id.clone(), user.clone())
        .with_instructions(instructions)
        .with_metadata(metadata)
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

#[test]
fn raw_sbd_transfer_is_rejected_without_exact_active_validation_fee() {
    let (state, user, user_key_pair, recipient, treasury, sbd) = test_state();
    let genesis_hash = commit_empty_genesis_like_block(&state);
    let gov_1 = key_pair(21);
    let gov_2 = key_pair(22);
    let policy = validation_fee_policy(&state, sbd.clone(), treasury, genesis_hash);
    install_validation_fee_policy(&state, &user, policy.clone(), &[&gov_1, &gov_2]);

    let missing_fee_error = validate_in_block(
        &state,
        3,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &sbd,
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
            &sbd,
            &policy,
            true,
        ),
    );
    assert_eq!(exact_fee_result, "ok");
}
