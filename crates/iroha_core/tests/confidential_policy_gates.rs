//! Tests covering confidential policy gating for transparent asset instructions.

use crate::native_genesis::{certified_state, certified_state_with};
use iroha_config::parameters::defaults;
use iroha_core::{
    smartcontracts::Execute,
    state::{State, StateTransaction, World, WorldReadOnly},
    sumeragi::test_chain::TestChainConfig,
    tx::ValidationFail,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    asset::{
        AssetDefinition, AssetId,
        definition::{AssetConfidentialPolicy, ConfidentialPolicyMode},
    },
    isi::{InstructionBox, Mint, Transfer, error::InstructionExecutionError, zk::RegisterZkAsset},
    permission::Permission,
    prelude::*,
};
use iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionConfidentialPolicy;
use iroha_model_base::domain::DomainId;
use mv::storage::StorageReadOnly;
use std::num::NonZeroU64;
fn checked_random_confidential_policy_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked confidential policy keypair")
}
fn checked_random_confidential_policy_account_id() -> AccountId {
    AccountId::new(
        checked_random_confidential_policy_keypair()
            .public_key()
            .clone(),
    )
}
#[test]
fn confidential_policy_fixture_uses_checked_randomness() {
    let key_pair = checked_random_confidential_policy_keypair();
    assert_eq!(key_pair.public_key().algorithm(), Algorithm::Ed25519);
}
fn init_state() -> (
    State,
    iroha_data_model::block::BlockHeader,
    AccountId,
    AccountId,
    AssetDefinitionId,
) {
    init_state_with(|_| {})
}
/// Apply the authenticated genesis over the universal owner and recipient accounts and the
/// owner's explicit domain-owned asset definition, with its first-release hybrid ledger.
///
/// Live domain registration requires an SNS lease, which is not a confidential policy gate,
/// so the accounts, domain and definition are part of the genesis World. `RegisterZkAsset` is
/// an administrative instruction that only the signed genesis may seed (later changes need a
/// governed lifecycle), so the genesis authority holds the exact confidential-policy grant for
/// this definition and registers its ledger. The returned header is the first ordinary block
/// after genesis.
fn init_state_with(
    configure: impl FnOnce(&mut TestChainConfig),
) -> (
    State,
    iroha_data_model::block::BlockHeader,
    AccountId,
    AccountId,
    AssetDefinitionId,
) {
    let domain_id: DomainId = DomainId::try_new("zkdomain", "universal").expect("domain id");
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("zkdomain", "universal").unwrap(),
            "shielded".parse().unwrap(),
        );
    let owner = checked_random_confidential_policy_account_id();
    let recipient = checked_random_confidential_policy_account_id();
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&owner)],
        [
            Account::new(owner.clone()).build(&owner),
            Account::new(recipient.clone()).build(&recipient),
        ],
        [AssetDefinition::numeric(
            asset_def_id.clone(),
            "shielded".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(domain_id),
        )
        .build(&owner)],
    );
    let mut config = TestChainConfig::new(world, 0);
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    let policy_grant: Permission = CanManageAssetDefinitionConfidentialPolicy {
        asset_definition: asset_def_id.clone(),
    }
    .into();
    let mut genesis_permissions = config
        .world
        .account_permissions_mut_for_testing()
        .view()
        .get(&genesis_account)
        .cloned()
        .unwrap_or_default();
    genesis_permissions.insert(policy_grant);
    config
        .world
        .account_permissions_mut_for_testing()
        .insert(genesis_account, genesis_permissions);
    config
        .genesis_instructions
        .push(RegisterZkAsset::new(asset_def_id.clone(), None).into());
    configure(&mut config);
    let state = certified_state_with(config);
    let header = ordinary_header(&state, 2);
    (state, header, owner, recipient, asset_def_id)
}
/// An ordinary fixture block on the current committed tip of the authenticated chain.
fn ordinary_header(state: &State, height: u64) -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero height"),
        state.view().latest_block_hash(),
        None,
        height,
        0,
    )
}
fn set_confidential_policy_mode(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset_definition_id: &AssetDefinitionId,
    mode: ConfidentialPolicyMode,
) {
    let asset_definition = state_transaction
        .world
        .asset_definition_mut(asset_definition_id)
        .expect("asset definition exists");
    let mut policy = *asset_definition.confidential_policy();
    policy.mode = mode;
    asset_definition.set_confidential_policy(policy);
}
#[test]
fn transparent_mint_rejected_for_shielded_only_policy() {
    // The authenticated genesis seeds the universal account, the explicit domain-owned
    // asset definition and its first-release hybrid ledger.
    let (state, header, owner, _recipient, asset_def_id) = init_state();
    let mut block = state.block(header);
    let mut stx = block.transaction();
    // Emulate a completed governance transition to ShieldedOnly.
    {
        let asset_def = stx
            .world
            .asset_definition_mut(&asset_def_id)
            .expect("asset definition exists");
        let previous_policy = *asset_def.confidential_policy();
        let mut policy = AssetConfidentialPolicy::shielded_only();
        policy.vk_set_hash = *previous_policy.vk_set_hash();
        policy.poseidon_params_id = previous_policy.poseidon_params_id();
        policy.pedersen_params_id = previous_policy.pedersen_params_id();
        asset_def.set_confidential_policy(policy);
    }
    // Attempt to mint transparently; should be rejected by policy gate.
    let asset_id = AssetId::new(asset_def_id.clone(), owner.clone());
    let mint = Mint::asset_quantity(10_u32, asset_id);
    let result = stx.world.executor().clone().execute_instruction(
        &mut stx,
        &owner,
        &InstructionBox::from(mint),
    );
    let err = result.expect_err("mint expected to fail");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(msg)) => {
            assert!(
                msg.contains("transparent mint not permitted by policy"),
                "unexpected error message: {msg}"
            )
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn transparent_transfer_rejected_after_policy_switch_to_shielded_only() {
    // The authenticated genesis seeds both universal accounts, the explicit domain-owned
    // asset definition and its first-release hybrid ledger, so transparent mint succeeds.
    let (state, header, owner, recipient, asset_def_id) = init_state();
    let mut block = state.block(header);
    // The transfer executes under one explicit retained invocation, as a signed call does.
    let mut stx = block.transaction_for_fastpq_testing(Hash::new(b"confidential-policy-transfer"));
    let asset_id = AssetId::new(asset_def_id.clone(), owner.clone());
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            &InstructionBox::from(Mint::asset_quantity(25_u32, asset_id.clone())),
        )
        .expect("initial mint should succeed");
    // Flip policy to ShieldedOnly to emulate a governance transition becoming effective.
    {
        let asset_def = stx
            .world
            .asset_definition_mut(&asset_def_id)
            .expect("asset definition exists");
        let previous_policy = *asset_def.confidential_policy();
        let mut policy = AssetConfidentialPolicy::shielded_only();
        policy.vk_set_hash = *previous_policy.vk_set_hash();
        policy.poseidon_params_id = previous_policy.poseidon_params_id();
        policy.pedersen_params_id = previous_policy.pedersen_params_id();
        asset_def.set_confidential_policy(policy);
    }
    // Attempt a transparent transfer; policy gate should reject it.
    let transfer = Transfer::asset_quantity(asset_id, 5_u32, recipient);
    let result = stx.world.executor().clone().execute_instruction(
        &mut stx,
        &owner,
        &InstructionBox::from(transfer),
    );
    let err = result.expect_err("transfer expected to fail");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(msg)) => {
            assert!(
                msg.contains("transparent transfer not permitted by policy"),
                "unexpected error message: {msg}"
            )
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn schedule_shielded_only_requires_window() {
    // The authenticated genesis registers the confidential asset state.
    let (state, header, owner, _recipient, asset_def_id) = init_state();
    let mut block = state.block(header);
    let mut stx = block.transaction();
    set_confidential_policy_mode(&mut stx, &asset_def_id, ConfidentialPolicyMode::Convertible);
    stx.apply();
    block
        .commit_empty_block_for_testing()
        .expect("commit setup block");
    let header2 = ordinary_header(&state, 3);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let delay = defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS;
    let notice = defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS;
    let current_height = stx2.block_height();
    let lead_time = delay.max(notice) + 1;
    let effective_height = current_height + lead_time;
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        Hash::new(b"missing-window"),
        None,
    );
    let result = schedule.execute(&owner, &mut stx2);
    let err = result.expect_err("transition without window must fail");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("conversion window is required"),
        "unexpected error message: {msg}"
    );
}
#[test]
fn shielded_transition_abort_retains_active_confidential_mode_when_supply_is_non_zero() {
    // The authenticated genesis registers the confidential asset state.
    let (state, header, owner, _recipient, asset_def_id) = init_state();
    let mut block = state.block(header);
    let mut stx = block.transaction();
    set_confidential_policy_mode(&mut stx, &asset_def_id, ConfidentialPolicyMode::Convertible);
    let asset_id = AssetId::new(asset_def_id.clone(), owner.clone());
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            &InstructionBox::from(Mint::asset_quantity(10_u32, asset_id.clone())),
        )
        .expect("mint should succeed");
    stx.apply();
    block
        .commit_empty_block_for_testing()
        .expect("commit setup block");
    let header2 = ordinary_header(&state, 3);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let delay = defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS;
    let notice = defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS;
    let current_height = stx2.block_height();
    let lead_time = delay.max(notice) + 3;
    let effective_height = current_height + lead_time;
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        Hash::new(b"abort-shielded"),
        Some(defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS),
    );
    schedule
        .execute(&owner, &mut stx2)
        .expect("schedule transition");
    // Model a transition whose conversion window opened from TransparentOnly:
    // the active mode is already Convertible, while the audit record retains
    // the pre-window mode. Aborting finalization must never strand commitments
    // by restoring that earlier transparent mode.
    let mut policy = *stx2
        .world
        .asset_definition(&asset_def_id)
        .expect("asset definition exists")
        .confidential_policy();
    let mut pending = policy
        .pending_transition
        .expect("scheduled transition is present");
    pending.previous_mode = ConfidentialPolicyMode::TransparentOnly;
    policy.pending_transition = Some(pending);
    stx2.world
        .asset_definition_mut(&asset_def_id)
        .expect("asset definition exists")
        .set_confidential_policy(policy);
    stx2.apply();
    block2
        .commit_empty_block_for_testing()
        .expect("commit scheduling block");
    let header3 = ordinary_header(&state, effective_height);
    let mut block3 = state.block(header3);
    let stx3 = block3.transaction();
    stx3.apply();
    block3
        .commit_world_overlay_for_testing()
        .expect("commit transition block");
    let view = state.view();
    let def = view
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .expect("asset definition present");
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::Convertible,
        "transition should abort because transparent supply is non-zero"
    );
    assert!(
        def.confidential_policy().pending_transition().is_none(),
        "pending transition must be cleared after abort"
    );
}
#[test]
fn policy_transition_reaches_shielded_only_on_schedule() {
    // The policy test configuration is installed before the genesis policies are signed.
    let mut zk_cfg = certified_state(World::new()).installed_zk().clone();
    zk_cfg.policy_transition_delay_blocks = 1;
    zk_cfg.policy_transition_window_blocks = 1;
    let (state, header, owner, _recipient, asset_def_id) =
        init_state_with(|config| config.zk = Some(zk_cfg));
    // The authenticated genesis registers the confidential asset state.
    let mut block = state.block(header);
    let mut stx = block.transaction();
    set_confidential_policy_mode(&mut stx, &asset_def_id, ConfidentialPolicyMode::Convertible);
    stx.apply();
    block
        .commit_empty_block_for_testing()
        .expect("commit setup block");
    let header2 = ordinary_header(&state, 3);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let effective_height = stx2.block_height() + 2;
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        Hash::new(b"auto-apply"),
        Some(1),
    );
    schedule
        .execute(&owner, &mut stx2)
        .expect("schedule transition");
    stx2.apply();
    block2
        .commit_empty_block_for_testing()
        .expect("commit scheduling block");
    let header_transition = ordinary_header(&state, effective_height);
    let block_transition = state.block(header_transition);
    block_transition
        .commit_world_overlay_for_testing()
        .expect("commit transition block");
    let view = state.view();
    let def = view
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .expect("asset definition exists");
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::ShieldedOnly,
        "transition should elevate policy to ShieldedOnly"
    );
    assert!(
        def.confidential_policy().pending_transition().is_none(),
        "pending transition must clear after application"
    );
}
