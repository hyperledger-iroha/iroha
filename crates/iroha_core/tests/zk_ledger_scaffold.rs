//! Scaffold tests for zk asset handlers: `RegisterZkAsset`, Shield, `ZkTransfer`, Unshield.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use std::{num::NonZeroU64, str::FromStr};

use iroha_config::parameters::defaults;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::NewAccount,
    asset::definition::ConfidentialPolicyMode,
    confidential::{ConfidentialEncryptedPayload, ConfidentialStatus},
    isi::error::{InstructionExecutionError, InvalidParameterError},
    name::Name,
    permission::Permission,
    prelude::*,
    proof::{VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use iroha_zkp_halo2::confidential;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const ASSET_HIDDEN_POOL_ID: &str = "boi-private-is-pool";
const ASSET_HIDDEN_BACKEND: &str = "halo2/ipa";
const ASSET_HIDDEN_TEST_CIRCUIT: &str = "halo2/ipa:tiny-add";

fn derive_test_nullifier(
    nk: &[u8; 32],
    rho: &[u8; 32],
    asset_id: &str,
    chain_id: &str,
) -> [u8; 32] {
    confidential::derive_nullifier(nk, rho, asset_id.as_bytes(), chain_id.as_bytes())
}

fn asset_hidden_test_fixture() -> iroha_core::zk::test_utils::FixtureEnvelope {
    let seed = halo2_fixture_envelope(ASSET_HIDDEN_TEST_CIRCUIT, [0u8; 32]);
    let vk_hash = seed
        .vk_hash(ASSET_HIDDEN_BACKEND)
        .expect("test fixture verifying key hash");
    halo2_fixture_envelope(ASSET_HIDDEN_TEST_CIRCUIT, vk_hash)
}

fn native_ipa_attachment() -> iroha_data_model::proof::ProofAttachment {
    let fixture = asset_hidden_test_fixture();
    let proof = fixture.proof_box(ASSET_HIDDEN_BACKEND);
    let vk_ref = VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "native_ipa_vk");
    iroha_data_model::proof::ProofAttachment::new_ref(ASSET_HIDDEN_BACKEND.into(), proof, vk_ref)
}

fn native_ipa_verifying_key_record() -> (VerifyingKeyId, VerifyingKeyRecord) {
    let id = VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "native_ipa_vk");
    let fixture = asset_hidden_test_fixture();
    let vk_box = fixture
        .vk_box(ASSET_HIDDEN_BACKEND)
        .expect("test fixture verifying key");
    let commitment = iroha_core::zk::hash_vk(&vk_box);
    let mut record = VerifyingKeyRecord::new(
        1,
        ASSET_HIDDEN_TEST_CIRCUIT,
        BackendTag::Halo2IpaPasta,
        "pallas",
        fixture.schema_hash,
        commitment,
    );
    record.vk_len = vk_box.bytes.len() as u32;
    record.max_proof_bytes = 1_000_000;
    record.gas_schedule_id = Some("halo2_default".into());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    (id, record)
}

fn state_with_account(domain_name: &str) -> (State, AccountId) {
    let (owner, _owner_key) = gen_account_in(domain_name);
    let domain_id = DomainId::try_new(domain_name, "universal").unwrap();
    let domain = Domain::new(domain_id).build(&owner);
    let account = iroha_data_model::account::Account::new(owner.clone()).build(&owner);
    let world = World::with_assets([domain], [account], [], [], []);
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut zk = iroha_core::state::default_zk_config();
    zk.halo2.enabled = true;
    zk.halo2.curve = iroha_config::parameters::actual::ZkCurve::Pallas;
    zk.halo2.backend = iroha_config::parameters::actual::Halo2Backend::Ipa;
    state.set_zk(zk);
    (state, owner)
}

fn asset_hidden_storage_state(domain_name: &str) -> (State, AccountId, AssetDefinitionId) {
    let (state, owner) = state_with_account(domain_name);
    let domain_id = DomainId::try_new(domain_name, "universal").unwrap();
    let asset_def_id = AssetDefinitionId::new(domain_id, "asset_hidden_pool".parse().unwrap());
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            Register::asset_definition(
                AssetDefinition::numeric(asset_def_id.clone())
                    .with_name(asset_def_id.name().to_string()),
            )
            .into(),
        )
        .expect("register asset-hidden pool storage asset");
    stx.apply();
    block.commit().expect("commit asset-hidden storage setup");
    (state, owner, asset_def_id)
}

fn grant_vk_management(stx: &mut iroha_core::state::StateTransaction<'_, '_>, owner: &AccountId) {
    let permission = Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(()));
    Grant::account_permission(permission, owner.clone())
        .execute(owner, stx)
        .expect("grant verifying-key management");
}

fn registered_asset_hidden_pool() -> (State, AccountId, AssetDefinitionId, [u8; 32]) {
    let (state, owner, asset_def_id) = asset_hidden_storage_state("ahpool");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    grant_vk_management(&mut stx, &owner);
    let (vk_id, vk_record) = native_ipa_verifying_key_record();
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: vk_record,
            }
            .into(),
        )
        .expect("register native IPA verifier");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::zk::RegisterAssetHiddenZkPool::new(
                ASSET_HIDDEN_POOL_ID.to_owned(),
                asset_def_id.clone(),
                [0xA0; 32],
                vk_id,
            )
            .into(),
        )
        .expect("register asset-hidden pool");
    stx.apply();
    block.commit().expect("commit asset-hidden pool setup");
    (state, owner, asset_def_id, [0u8; 32])
}

fn execute_asset_hidden_on_state(
    state: &State,
    owner: &AccountId,
    instruction: iroha_data_model::isi::zk::AssetHiddenZkTransfer,
    height: u64,
) -> Result<(), iroha_data_model::ValidationFail> {
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let result = stx.world.executor().clone().execute_instruction(
        &mut stx,
        owner,
        InstructionBox::from(instruction),
    );
    if result.is_ok() {
        stx.apply();
        block.commit().expect("commit asset-hidden transfer");
    }
    result
}

fn execute_asset_hidden_for_test(
    instruction: iroha_data_model::isi::zk::AssetHiddenZkTransfer,
) -> Result<(), iroha_data_model::ValidationFail> {
    let (state, owner) = state_with_account("ahzk");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.executor().clone().execute_instruction(
        &mut stx,
        &owner,
        InstructionBox::from(instruction),
    )
}

fn asset_hidden_transfer(
    pool_id: impl Into<String>,
    proof: iroha_data_model::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
) -> iroha_data_model::isi::zk::AssetHiddenZkTransfer {
    iroha_data_model::isi::zk::AssetHiddenZkTransfer::new(
        pool_id.into(),
        vec![[0xA1; 32]],
        vec![[0xB2; 32]],
        proof,
        root_hint,
    )
}

fn assert_instruction_error_contains(err: iroha_data_model::ValidationFail, expected: &str) {
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            InstructionExecutionError::InvariantViolation(msg),
        ) => assert!(msg.contains(expected), "unexpected error message: {msg}"),
        iroha_data_model::ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(msg)),
        ) => assert!(msg.contains(expected), "unexpected error message: {msg}"),
        other => {
            let msg = format!("{other:?}");
            assert!(
                msg.contains(expected),
                "unexpected error message: {msg}; expected substring {expected:?}"
            );
        }
    }
}

#[test]
fn asset_hidden_transfer_is_fail_closed_until_pool_verifier_registered() {
    let err = execute_asset_hidden_for_test(asset_hidden_transfer(
        "boi-private-is-pool",
        native_ipa_attachment(),
        Some([0xC3; 32]),
    ))
    .expect_err("asset-hidden transfer must fail until pool verifier state exists");
    assert_instruction_error_contains(err, "pool is not registered");
}

#[test]
fn asset_hidden_transfer_rejects_blank_pool_id_before_proof_work() {
    let err = execute_asset_hidden_for_test(asset_hidden_transfer(
        "   ",
        native_ipa_attachment(),
        Some([0xC3; 32]),
    ))
    .expect_err("blank pool id must be rejected");
    assert_instruction_error_contains(err, "pool_id must be non-empty");
}

#[test]
fn asset_hidden_transfer_requires_root_hint() {
    let err = execute_asset_hidden_for_test(asset_hidden_transfer(
        "boi-private-is-pool",
        native_ipa_attachment(),
        None,
    ))
    .expect_err("asset-hidden transfer must bind to a pool root");
    assert_instruction_error_contains(err, "requires root_hint");
}

#[test]
fn asset_hidden_transfer_requires_input_nullifiers() {
    let err = execute_asset_hidden_for_test(iroha_data_model::isi::zk::AssetHiddenZkTransfer::new(
        "boi-private-is-pool".to_owned(),
        Vec::new(),
        vec![[0xB2; 32]],
        native_ipa_attachment(),
        Some([0xC3; 32]),
    ))
    .expect_err("asset-hidden transfer without input nullifiers must fail");
    assert_instruction_error_contains(err, "requires at least one input nullifier");
}

#[test]
fn asset_hidden_transfer_requires_output_commitments() {
    let err = execute_asset_hidden_for_test(iroha_data_model::isi::zk::AssetHiddenZkTransfer::new(
        "boi-private-is-pool".to_owned(),
        vec![[0xA1; 32]],
        Vec::new(),
        native_ipa_attachment(),
        Some([0xC3; 32]),
    ))
    .expect_err("asset-hidden transfer without output commitments must fail");
    assert_instruction_error_contains(err, "requires at least one output commitment");
}

#[test]
fn asset_hidden_transfer_rejects_proof_backend_mismatch() {
    let mut attachment = native_ipa_attachment();
    attachment.proof.backend = "stark/fri/open-v1".into();
    let err = execute_asset_hidden_for_test(asset_hidden_transfer(
        "boi-private-is-pool",
        attachment,
        Some([0xC3; 32]),
    ))
    .expect_err("proof backend mismatch must fail before verifier lookup");
    assert_instruction_error_contains(err, "proof backend mismatch");
}

#[test]
fn register_asset_hidden_pool_rejects_blank_pool_id() {
    let (state, owner, asset_def_id) = asset_hidden_storage_state("ahblank");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let (_, vk_record) = native_ipa_verifying_key_record();
    let vk_id = VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "unused_blank_pool_vk");
    grant_vk_management(&mut stx, &owner);
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: vk_record,
            }
            .into(),
        )
        .expect("register verifier");
    let err = stx
        .world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::zk::RegisterAssetHiddenZkPool::new(
                "   ".to_owned(),
                asset_def_id,
                [0xA0; 32],
                vk_id,
            )
            .into(),
        )
        .expect_err("blank pool ids must be rejected");
    assert_instruction_error_contains(err, "pool_id must be non-empty");
}

#[test]
fn register_asset_hidden_pool_rejects_zero_asset_set_root() {
    let (state, owner, asset_def_id) = asset_hidden_storage_state("ahzero");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let err = stx
        .world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::zk::RegisterAssetHiddenZkPool::new(
                ASSET_HIDDEN_POOL_ID.to_owned(),
                asset_def_id,
                [0u8; 32],
                VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "missing"),
            )
            .into(),
        )
        .expect_err("zero asset-set roots must be rejected before verifier lookup");
    assert_instruction_error_contains(err, "asset_set_root must be nonzero");
}

#[test]
fn register_asset_hidden_pool_rejects_missing_verifier() {
    let (state, owner, asset_def_id) = asset_hidden_storage_state("ahmissingvk");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let err = stx
        .world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            iroha_data_model::isi::zk::RegisterAssetHiddenZkPool::new(
                ASSET_HIDDEN_POOL_ID.to_owned(),
                asset_def_id,
                [0xA0; 32],
                VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "does-not-exist"),
            )
            .into(),
        )
        .expect_err("missing pool verifier must fail");
    assert_instruction_error_contains(err, "VerifyingKeyMissing");
}

#[test]
fn asset_hidden_transfer_executes_against_registered_pool_and_updates_state() {
    let (state, owner, asset_def_id, root_hint) = registered_asset_hidden_pool();
    execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(
            ASSET_HIDDEN_POOL_ID,
            native_ipa_attachment(),
            Some(root_hint),
        ),
        3,
    )
    .expect("registered asset-hidden transfer must execute");
    let view = state.view();
    let zk_state = view.world.zk_assets().get(&asset_def_id).expect("zk state");
    assert_eq!(
        zk_state.asset_hidden_pool_id.as_deref(),
        Some(ASSET_HIDDEN_POOL_ID)
    );
    assert!(zk_state.nullifiers.contains(&[0xA1; 32]));
    assert_eq!(zk_state.commitments, vec![[0xB2; 32]]);
    assert_ne!(
        zk_state.root_history.last().copied().unwrap_or([0u8; 32]),
        [0u8; 32]
    );
}

#[test]
fn asset_hidden_transfer_rejects_stale_pool_root() {
    let (state, owner, _asset_def_id, _root_hint) = registered_asset_hidden_pool();
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(
            ASSET_HIDDEN_POOL_ID,
            native_ipa_attachment(),
            Some([0xE5; 32]),
        ),
        3,
    )
    .expect_err("unknown roots must fail");
    assert_instruction_error_contains(err, "root_hint is not in pool root history");
}

#[test]
fn asset_hidden_transfer_rejects_duplicate_nullifier_within_tx() {
    let (state, owner, _asset_def_id, root_hint) = registered_asset_hidden_pool();
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        iroha_data_model::isi::zk::AssetHiddenZkTransfer::new(
            ASSET_HIDDEN_POOL_ID.to_owned(),
            vec![[0xA1; 32], [0xA1; 32]],
            vec![[0xB2; 32]],
            native_ipa_attachment(),
            Some(root_hint),
        ),
        3,
    )
    .expect_err("duplicate nullifiers in one transfer must fail");
    assert_instruction_error_contains(err, "duplicate input nullifier");
}

#[test]
fn asset_hidden_transfer_rejects_zero_output_commitment() {
    let (state, owner, _asset_def_id, root_hint) = registered_asset_hidden_pool();
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        iroha_data_model::isi::zk::AssetHiddenZkTransfer::new(
            ASSET_HIDDEN_POOL_ID.to_owned(),
            vec![[0xA1; 32]],
            vec![[0u8; 32]],
            native_ipa_attachment(),
            Some(root_hint),
        ),
        3,
    )
    .expect_err("zero output commitments must fail");
    assert_instruction_error_contains(err, "output commitment must be nonzero");
}

#[test]
fn asset_hidden_transfer_rejects_duplicate_nullifier_replay() {
    let (state, owner, _asset_def_id, root_hint) = registered_asset_hidden_pool();
    execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(
            ASSET_HIDDEN_POOL_ID,
            native_ipa_attachment(),
            Some(root_hint),
        ),
        3,
    )
    .expect("first transfer must execute");
    let next_root = state
        .view()
        .world
        .zk_assets()
        .iter()
        .find_map(|(_, st)| {
            st.asset_hidden_pool_id
                .as_deref()
                .map(|_| st.root_history.last().copied())
        })
        .flatten()
        .expect("pool root");
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(
            ASSET_HIDDEN_POOL_ID,
            native_ipa_attachment(),
            Some(next_root),
        ),
        4,
    )
    .expect_err("replayed nullifier must fail");
    assert_instruction_error_contains(err, "nullifier already spent");
}

#[test]
fn asset_hidden_transfer_rejects_verifier_reference_mismatch() {
    let (state, owner, _asset_def_id, root_hint) = registered_asset_hidden_pool();
    let mut attachment = native_ipa_attachment();
    attachment.vk_ref = VerifyingKeyId::new(ASSET_HIDDEN_BACKEND, "wrong_vk");
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(ASSET_HIDDEN_POOL_ID, attachment, Some(root_hint)),
        3,
    )
    .expect_err("wrong verifier reference must fail");
    assert_instruction_error_contains(err, "verifying key reference mismatch");
}

#[test]
fn asset_hidden_transfer_rejects_invalid_proof_without_state_update() {
    let (state, owner, asset_def_id, root_hint) = registered_asset_hidden_pool();
    let before_commitments = {
        let view = state.view();
        view.world
            .zk_assets()
            .get(&asset_def_id)
            .expect("zk state")
            .commitments
            .len()
    };
    let mut attachment = native_ipa_attachment();
    attachment.proof.bytes[0] ^= 0xFF;
    let err = execute_asset_hidden_on_state(
        &state,
        &owner,
        asset_hidden_transfer(ASSET_HIDDEN_POOL_ID, attachment, Some(root_hint)),
        3,
    )
    .expect_err("invalid proofs must fail");
    assert_instruction_error_contains(err, "invalid asset-hidden transfer proof");
    let after_commitments = {
        let view = state.view();
        view.world
            .zk_assets()
            .get(&asset_def_id)
            .expect("zk state")
            .commitments
            .len()
    };
    assert_eq!(before_commitments, after_commitments);
}

#[test]
fn register_zk_asset_writes_policy_metadata() {
    // Minimal state and transaction
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Setup: domain and asset def
    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "zcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    // Register zk policy
    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    let ib: InstructionBox = reg.into();
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, ib)
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    // Verify metadata key exists
    let view_policy = state.view();
    let def = view_policy
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .unwrap();
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::Convertible
    );
    assert!(def.confidential_policy().vk_set_hash().is_none());
    assert_eq!(
        def.confidential_policy().poseidon_params_id(),
        defaults::confidential::POSEIDON_PARAMS_ID
    );
    assert_eq!(
        def.confidential_policy().pedersen_params_id(),
        defaults::confidential::PEDERSEN_PARAMS_ID
    );
    let policy_key = Name::from_str("zk.policy").unwrap();
    let val = def.metadata().get(&policy_key);
    assert!(val.is_some());
    let policy_json: norito::json::Value = val.unwrap().try_into_any_norito().expect("json decode");
    let digest_hex = policy_json
        .get("features_digest")
        .and_then(|v| v.as_str())
        .expect("features_digest present");
    assert_eq!(
        digest_hex,
        hex::encode(def.confidential_policy().features_digest().as_ref())
    );
}

#[test]
fn register_zk_asset_without_shielding_sets_transparent_policy() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "desk".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        false,
        false,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(reg))
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    let view_policy = state.view();
    let def = view_policy
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .unwrap();
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::TransparentOnly
    );
    assert!(def.confidential_policy().vk_set_hash().is_none());
    assert_eq!(
        def.confidential_policy().poseidon_params_id(),
        defaults::confidential::POSEIDON_PARAMS_ID
    );
    assert_eq!(
        def.confidential_policy().pedersen_params_id(),
        defaults::confidential::PEDERSEN_PARAMS_ID
    );
    let policy_key = Name::from_str("zk.policy").unwrap();
    let val = def.metadata().get(&policy_key).unwrap();
    let policy_json: norito::json::Value = val.try_into_any_norito().expect("json decode");
    let digest_hex = policy_json
        .get("features_digest")
        .and_then(|v| v.as_str())
        .expect("features_digest present");
    assert_eq!(
        digest_hex,
        hex::encode(def.confidential_policy().features_digest().as_ref())
    );
}

#[test]
fn schedule_confidential_policy_transition_records_pending() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Setup base entities.
    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "schedule".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    // Register asset with convertible policy (allow shield/unshield).
    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    // New block for scheduling the transition.
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();

    let delay = defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS;
    let window_blocks = defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS;
    let effective_height = stx2.block_height() + delay + window_blocks;
    let transition_id = Hash::new(b"convert-to-shielded");
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        transition_id.clone(),
        Some(defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS),
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, schedule.into())
        .unwrap();
    stx2.apply();
    block2.commit().expect("commit schedule block");

    let view = state.view();
    let def = view
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .expect("asset definition present");
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::Convertible
    );
    let pending = def
        .confidential_policy()
        .pending_transition()
        .expect("pending transition scheduled");
    assert_eq!(pending.new_mode(), ConfidentialPolicyMode::ShieldedOnly);
    assert_eq!(pending.effective_height(), effective_height);
    assert_eq!(pending.transition_id(), &transition_id);

    let policy_key = Name::from_str("zk.policy").unwrap();
    let policy_json: norito::json::Value = def
        .metadata()
        .get(&policy_key)
        .expect("policy metadata present")
        .try_into_any_norito()
        .expect("policy metadata decodes");
    assert!(
        policy_json
            .get("pending_transition")
            .and_then(|value| value.as_object())
            .is_some(),
        "metadata should capture pending transition summary"
    );
}

#[test]
fn confidential_policy_transition_applies_at_effective_height() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "transition".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }
    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let delay = defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS;
    let window_blocks = defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS;
    let effective_height = stx2.block_height() + delay + window_blocks;
    let transition_id = Hash::new(b"convert->shielded");
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        transition_id.clone(),
        Some(defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS),
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, schedule.into())
        .unwrap();
    stx2.apply();
    block2.commit().expect("commit schedule block");

    // New block at the scheduled effective height.
    let header3 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(effective_height).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block3 = state.block(header3);
    let mut stx3 = block3.transaction();
    let next_effective = stx3.block_height() + delay + 4;
    let transition_id_2 = Hash::new(b"shielded->convertible");
    let reschedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::Convertible,
        next_effective,
        transition_id_2.clone(),
        None,
    );
    stx3.world
        .executor()
        .clone()
        .execute_instruction(&mut stx3, &owner, reschedule.into())
        .unwrap();
    stx3.apply();
    block3.commit().expect("commit effective block");

    let view = state.view();
    let def = view
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .expect("asset definition present");
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::ShieldedOnly,
        "previous transition should have activated"
    );
    let pending = def
        .confidential_policy()
        .pending_transition()
        .expect("new pending transition set");
    assert_eq!(pending.new_mode(), ConfidentialPolicyMode::Convertible);
    assert_eq!(pending.transition_id(), &transition_id_2);
    assert_eq!(pending.effective_height(), next_effective);
}

#[test]
fn cancel_confidential_policy_transition_clears_pending() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "cancel".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }
    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let delay = defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS;
    let window_blocks = defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS;
    let effective_height = stx2.block_height() + delay + window_blocks;
    let transition_id = Hash::new(b"pending-cancel");
    let schedule = iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        ConfidentialPolicyMode::ShieldedOnly,
        effective_height,
        transition_id.clone(),
        Some(defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS),
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, schedule.into())
        .unwrap();

    let cancel = iroha_data_model::isi::zk::CancelConfidentialPolicyTransition::new(
        asset_def_id.clone(),
        transition_id.clone(),
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, cancel.into())
        .unwrap();
    stx2.apply();
    block2.commit().expect("commit cancel block");

    let view = state.view();
    let def = view
        .world
        .asset_definitions()
        .get(&asset_def_id)
        .expect("asset definition present");
    assert!(def.confidential_policy().pending_transition().is_none());
    assert_eq!(
        def.confidential_policy().mode(),
        ConfidentialPolicyMode::Convertible
    );
    let policy_key = Name::from_str("zk.policy").unwrap();
    let policy_json: norito::json::Value = def
        .metadata()
        .get(&policy_key)
        .expect("policy metadata present")
        .try_into_any_norito()
        .expect("policy metadata decodes");
    assert!(
        policy_json
            .get("pending_transition")
            .map(|value| value.is_null())
            .unwrap_or(true),
        "pending transition metadata should be cleared"
    );
}

#[test]
fn transfer_rejects_when_nullifiers_exceed_cap() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(World::new(), kura, query);
    let mut zk_cfg = state.zk.clone();
    zk_cfg.max_nullifiers_per_tx = 1;
    state.set_zk(zk_cfg);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "capcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");

    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(reg))
        .expect("register zk asset");

    let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        vec![[1u8; 32], [2u8; 32]],
        vec![[9u8; 32]],
        native_ipa_attachment(),
        None,
    );
    let err = stx
        .world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(transfer))
        .expect_err("transfer should exceed nullifier cap");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(msg)),
        ) => {
            assert!(msg.contains("nullifiers per transaction cap exceeded"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn transfer_rejects_repeated_nullifier_in_same_instruction() {
    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "dupnf".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");

    let domain = Domain::new(domain_id.clone()).build(&owner);
    let account = iroha_data_model::account::Account::new(owner.clone()).build(&owner);
    let asset_def =
        AssetDefinition::numeric(asset_def_id.clone()).with_name(asset_def_id.name().to_string());
    let world = World::with_assets([domain], [account], [asset_def.build(&owner)], [], []);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(reg))
        .expect("register zk asset");

    let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        vec![[7u8; 32], [7u8; 32]],
        vec![[9u8; 32]],
        native_ipa_attachment(),
        None,
    );
    let err = stx
        .world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(transfer))
        .expect_err("transfer should reject repeated nullifier");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("duplicate nullifier"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn shield_rejected_when_policy_disallows() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "denyshield".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(1_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        false,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    let mut block2 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx2 = block2.transaction();
    let shield = iroha_data_model::isi::zk::Shield::new(
        asset_def_id.clone(),
        owner.clone(),
        100u128,
        [3u8; 32],
        ConfidentialEncryptedPayload::default(),
    );
    let res = stx2.world.executor().clone().execute_instruction(
        &mut stx2,
        &owner,
        InstructionBox::from(shield),
    );
    assert!(
        res.is_err(),
        "shield must be rejected when policy disallows shielding"
    );
}

#[test]
fn unshield_rejected_when_policy_disallows() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "denyunshield".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        false,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    let mut block2 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx2 = block2.transaction();
    let unshield = iroha_data_model::isi::zk::Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        50u128,
        vec![[4u8; 32]],
        native_ipa_attachment(),
        None,
    );
    let res = stx2.world.executor().clone().execute_instruction(
        &mut stx2,
        &owner,
        InstructionBox::from(unshield),
    );
    assert!(
        res.is_err(),
        "unshield must be rejected when policy disallows unshielding"
    );
}

#[test]
fn zk_transfer_rejected_when_policy_transparent() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "ztrans".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(1_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    let reg = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        true,
        true,
        None,
        None,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, reg.into())
        .unwrap();
    stx.apply();
    block.commit().expect("commit setup block");

    // Shield once to create commitments.
    let mut block2 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx2 = block2.transaction();
    let shield = iroha_data_model::isi::zk::Shield::new(
        asset_def_id.clone(),
        owner.clone(),
        100u128,
        [5u8; 32],
        ConfidentialEncryptedPayload::default(),
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, InstructionBox::from(shield))
        .unwrap();
    stx2.apply();
    block2.commit().expect("commit shield block");

    // Re-register with shielding disabled (policy becomes transparent).
    let mut block3 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(3_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx3 = block3.transaction();
    let disable = iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_def_id.clone(),
        iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
        false,
        false,
        None,
        None,
        None,
    );
    stx3.world
        .executor()
        .clone()
        .execute_instruction(&mut stx3, &owner, disable.into())
        .unwrap();
    stx3.apply();
    block3.commit().expect("commit policy block");

    // Attempt transfer; should fail under transparent policy.
    let mut block4 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(4_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx4 = block4.transaction();
    let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        vec![[7u8; 32]],
        vec![[8u8; 32]],
        native_ipa_attachment(),
        None,
    );
    let res = stx4.world.executor().clone().execute_instruction(
        &mut stx4,
        &owner,
        InstructionBox::from(transfer),
    );
    assert!(
        res.is_err(),
        "transfer must be rejected when policy forbids shielded operations"
    );
}

#[test]
fn shield_burns_and_unshield_mints() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "zcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");

    // Setup: register domain/account/asset and mint
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(1000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            asset_def_id.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    // Shield 400
    let shield = iroha_data_model::isi::zk::Shield::new(
        asset_def_id.clone(),
        owner.clone(),
        400u128,
        [1u8; 32],
        ConfidentialEncryptedPayload::default(),
    );
    let ib: InstructionBox = shield.into();
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, ib)
        .unwrap();
    stx.apply();
    block.commit().expect("commit shield block");

    let asset_id = AssetId::of(asset_def_id.clone(), owner.clone());
    let bal_after_shield = state
        .view()
        .world
        .assets()
        .get(&asset_id)
        .map(|v| v.clone().0)
        .unwrap();
    assert_eq!(bal_after_shield, 600u64.into());

    // Check ZK state exists and root updated
    {
        let view = state.view();
        let zk_state = view.world.zk_assets().get(&asset_def_id).expect("zk state");
        assert!(zk_state.root_history.last().copied().unwrap_or([0u8; 32]) != [0u8; 32]);
        assert_eq!(zk_state.commitments.len(), 1);
    }

    // Unshield 250
    let nk = [7u8; 32];
    let rho = [11u8; 32];
    let asset_id_str = asset_def_id.to_string();
    let chain_id = "iroha-test-chain";
    let nullifier = derive_test_nullifier(&nk, &rho, &asset_id_str, chain_id);
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let unshield = iroha_data_model::isi::zk::Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        250u128,
        vec![nullifier],
        native_ipa_attachment(),
        None,
    );
    let ib2: InstructionBox = unshield.into();
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, ib2)
        .unwrap();
    stx2.apply();
    block2.commit().expect("commit unshield block");
    let bal_after_unshield = state
        .view()
        .world
        .assets()
        .get(&asset_id)
        .map(|v| v.clone().0)
        .unwrap();
    assert_eq!(bal_after_unshield, 850u64.into());

    // Nullifier was consumed by unshield; second unshield with same nullifier must fail
    let mut block3 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(3_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx3 = block3.transaction();
    let repeat = iroha_data_model::isi::zk::Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![nullifier],
        native_ipa_attachment(),
        None,
    );
    let res = stx3
        .world
        .executor()
        .clone()
        .execute_instruction(&mut stx3, &owner, repeat.into());
    assert!(res.is_err(), "duplicate nullifier must be rejected");
}

#[test]
fn zk_roots_are_bounded_in_world_state() {
    use iroha_config::parameters::{actual as cfg, defaults};
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use nonzero_ext::nonzero;

    // Create state and set a small ZK cap
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(World::new(), kura, query);
    state.set_zk(cfg::Zk {
        halo2: cfg::Halo2 {
            enabled: defaults::zk::halo2::ENABLED,
            curve: cfg::ZkCurve::Pallas,
            backend: cfg::Halo2Backend::Ipa,
            max_k: defaults::zk::halo2::MAX_K,
            verifier_budget_ms: defaults::zk::halo2::VERIFIER_BUDGET_MS,
            verifier_max_batch: defaults::zk::halo2::VERIFIER_MAX_BATCH,
            ..cfg::Halo2::default()
        },
        fastpq: cfg::Fastpq {
            execution_mode: cfg::FastpqExecutionMode::Cpu,
            poseidon_mode: cfg::FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap: defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes: defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries: defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: defaults::zk::fastpq::METAL_DEBUG_FUSED,
        },
        stark: cfg::Stark::default(),
        root_history_cap: 4,
        ballot_history_cap: defaults::zk::vote::BALLOT_HISTORY_CAP,
        empty_root_on_empty: defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
        merkle_depth: defaults::zk::ledger::EMPTY_ROOT_DEPTH,
        preverify_max_bytes: defaults::zk::preverify::MAX_BYTES,
        preverify_budget_bytes: defaults::zk::preverify::BUDGET_BYTES,
        proof_history_cap: defaults::zk::proof::RECORD_HISTORY_CAP,
        proof_retention_grace_blocks: defaults::zk::proof::RETENTION_GRACE_BLOCKS,
        proof_prune_batch: defaults::zk::proof::PRUNE_BATCH_SIZE,
        bridge_proof_max_range_len: defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
        bridge_proof_max_past_age_blocks: defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
        bridge_proof_max_future_drift_blocks: defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
        sccp_allow_unready_transparent_proofs: false,
        poseidon_params_id: defaults::confidential::POSEIDON_PARAMS_ID,
        pedersen_params_id: defaults::confidential::PEDERSEN_PARAMS_ID,
        kaigi_roster_join_vk: None,
        kaigi_roster_leave_vk: None,
        kaigi_usage_vk: None,
        max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
        max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
        max_commitments_per_tx: 32,
        max_confidential_ops_per_block: 32,
        verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
        max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
        max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
        max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
        max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
        max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
        reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
        policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
        policy_transition_window_blocks: defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
        tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
        tree_frontier_checkpoint_interval:
            defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
        registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
        registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
        registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
        gas: cfg::ConfidentialGas {
            proof_base: defaults::confidential::gas::PROOF_BASE,
            per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: defaults::confidential::gas::PER_COMMITMENT,
        },
    });

    // Begin block/transaction
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Setup domain/account/asset and mint
    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "zcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
        // Register zk policy (Hybrid; allow shield)
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            asset_def_id.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }

    // Perform many shields to exceed the cap
    for i in 0..16u8 {
        let mut note = [0u8; 32];
        note[0] = i;
        let ib: InstructionBox = iroha_data_model::isi::zk::Shield::new(
            asset_def_id.clone(),
            owner.clone(),
            100u128,
            note,
            ConfidentialEncryptedPayload::default(),
        )
        .into();
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, ib)
            .unwrap();
    }
    stx.apply();
    block.commit().expect("commit shield block");

    // Assert bounded roots in world state
    let view = state.view();
    let zk_state = view.world.zk_assets().get(&asset_def_id).expect("zk state");
    assert!(zk_state.root_history.len() <= 4);
    assert_eq!(zk_state.root_history.len(), 4);
}

#[test]
fn frontier_checkpoints_respect_reorg_depth_bound() {
    use iroha_config::parameters::{actual as cfg, defaults};
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use nonzero_ext::nonzero;

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(World::new(), kura, query);

    state.set_zk(cfg::Zk {
        halo2: cfg::Halo2 {
            enabled: defaults::zk::halo2::ENABLED,
            curve: cfg::ZkCurve::Pallas,
            backend: cfg::Halo2Backend::Ipa,
            max_k: defaults::zk::halo2::MAX_K,
            verifier_budget_ms: defaults::zk::halo2::VERIFIER_BUDGET_MS,
            verifier_max_batch: defaults::zk::halo2::VERIFIER_MAX_BATCH,
            ..cfg::Halo2::default()
        },
        fastpq: cfg::Fastpq {
            execution_mode: cfg::FastpqExecutionMode::Cpu,
            poseidon_mode: cfg::FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap: defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes: defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries: defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: defaults::zk::fastpq::METAL_DEBUG_FUSED,
        },
        stark: cfg::Stark::default(),
        root_history_cap: 8,
        ballot_history_cap: defaults::zk::vote::BALLOT_HISTORY_CAP,
        empty_root_on_empty: defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
        merkle_depth: defaults::zk::ledger::EMPTY_ROOT_DEPTH,
        preverify_max_bytes: defaults::zk::preverify::MAX_BYTES,
        preverify_budget_bytes: defaults::zk::preverify::BUDGET_BYTES,
        proof_history_cap: defaults::zk::proof::RECORD_HISTORY_CAP,
        proof_retention_grace_blocks: defaults::zk::proof::RETENTION_GRACE_BLOCKS,
        proof_prune_batch: defaults::zk::proof::PRUNE_BATCH_SIZE,
        bridge_proof_max_range_len: defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
        bridge_proof_max_past_age_blocks: defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
        bridge_proof_max_future_drift_blocks: defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
        sccp_allow_unready_transparent_proofs: false,
        poseidon_params_id: defaults::confidential::POSEIDON_PARAMS_ID,
        pedersen_params_id: defaults::confidential::PEDERSEN_PARAMS_ID,
        kaigi_roster_join_vk: None,
        kaigi_roster_leave_vk: None,
        kaigi_usage_vk: None,
        max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
        max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
        max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
        max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
        verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
        max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
        max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
        max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
        max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
        max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
        reorg_depth_bound: 3,
        policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
        policy_transition_window_blocks: defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
        tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
        tree_frontier_checkpoint_interval: 1,
        registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
        registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
        registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
        gas: cfg::ConfidentialGas {
            proof_base: defaults::confidential::gas::PROOF_BASE,
            per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: defaults::confidential::gas::PER_COMMITMENT,
        },
    });

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "zcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");

    // Block 1: bootstrap domain/account/asset and register policy.
    {
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        for instr in [
            Register::domain(Domain::new(domain_id.clone())).into(),
            Register::account(NewAccount::new(owner.clone())).into(),
            Register::asset_definition(
                AssetDefinition::numeric(asset_def_id.clone())
                    .with_name(asset_def_id.name().to_string()),
            )
            .into(),
            Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def_id.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
                true,
                true,
                None,
                None,
                None,
            )
            .into(),
        ] {
            stx.world
                .executor()
                .clone()
                .execute_instruction(&mut stx, &owner, instr)
                .unwrap();
        }
        stx.apply();
        block.commit().expect("commit setup block");
    }

    // Subsequent blocks append a single shield to advance frontiers.
    for h in 2_u64..=8 {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(h).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let mut commitment = [0u8; 32];
        commitment[0] = h as u8;
        let shield: InstructionBox = iroha_data_model::isi::zk::Shield::new(
            asset_def_id.clone(),
            owner.clone(),
            100u128,
            commitment,
            ConfidentialEncryptedPayload::default(),
        )
        .into();
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, shield)
            .unwrap();
        stx.apply();
        block.commit().expect("commit shield block");
    }

    let view = state.view();
    let zk_state = view.world.zk_assets().get(&asset_def_id).expect("zk state");
    assert_eq!(zk_state.frontier_checkpoints.len(), 4);
    assert_eq!(
        zk_state.frontier_checkpoints.first().map(|cp| cp.height),
        Some(5)
    );
    assert_eq!(
        zk_state.frontier_checkpoints.last().map(|cp| cp.height),
        Some(8)
    );
}
