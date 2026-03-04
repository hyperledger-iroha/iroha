//! Ensure shielded asset operations reject raw STARK envelopes and require `OpenVerifyEnvelope`.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "zk-stark"))]

use iroha_core::state::WorldReadOnly;
use iroha_core::{kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State};
use iroha_crypto::Hash as CryptoHash;
use iroha_data_model::{
    account::NewAccount,
    confidential::ConfidentialStatus,
    isi::{
        Grant, verifying_keys,
        zk::{RegisterZkAsset, Unshield, ZkAssetMode, ZkTransfer},
    },
    permission::Permission,
    prelude::*,
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;

const BACKEND: &str = "stark/fri-v1/sha256-goldilocks-v1";
const STARK_TRANSFER_CIRCUIT: &str = "stark/fri-v1/sha256-goldilocks-v1:zk-transfer-v1";

fn stark_vk_bytes(circuit_id: &str) -> Vec<u8> {
    use iroha_core::zk_stark::{STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1};

    let payload = StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_owned(),
        n_log2: 1,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 0,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
    };
    norito::to_bytes(&payload).expect("encode stark vk payload")
}

fn stark_open_verify_envelope_bytes(circuit_id: &str, schema_descriptor: &[u8]) -> Vec<u8> {
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![[0xAA; 32]], vec![[0xBB; 32]]],
        envelope_bytes: vec![0xFE, 0xED, 0xFA, 0xCE],
    };
    let open_bytes = norito::to_bytes(&open).expect("encode stark open proof");
    let env = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        circuit_id,
        [0u8; 32],
        schema_descriptor.to_vec(),
        open_bytes,
    );
    norito::to_bytes(&env).expect("encode OpenVerifyEnvelope")
}

fn prepare_state() -> (State, AccountId, AssetDefinitionId) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        iroha_core::state::World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(iroha_core::state::World::new(), kura, query);
    state.zk.stark.enabled = true;

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let domain_id: DomainId = "zkd".parse().unwrap();
    let (owner, _owner_key) = gen_account_in("zkd");
    let asset_def_id: AssetDefinitionId = "zcoin#zkd".parse().unwrap();

    for instr in [
        Register::domain(Domain::new(domain_id)).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone())).into(),
        RegisterZkAsset::new(
            asset_def_id.clone(),
            ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
    ] {
        executor
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .expect("bootstrap");
    }

    stx.apply();
    block.commit().expect("commit bootstrap block");

    (state, owner, asset_def_id)
}

fn prepare_state_with_bound_stark_vk(
    schema_descriptor: &[u8],
) -> (State, AccountId, AssetDefinitionId, VerifyingKeyId) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        iroha_core::state::World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(iroha_core::state::World::new(), kura, query);
    state.zk.stark.enabled = true;

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let domain_id: DomainId = "zkd".parse().expect("domain id");
    let (owner, _owner_key) = gen_account_in("zkd");
    let asset_def_id: AssetDefinitionId = "zcoin#zkd".parse().expect("asset definition id");
    let vk_id = VerifyingKeyId::new(BACKEND, "vk_transfer");

    for instr in [
        Register::domain(Domain::new(domain_id)).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone())).into(),
    ] {
        executor
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .expect("bootstrap");
    }

    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().expect("permission id"),
        Json::new(()),
    );
    Grant::account_permission(perm, owner.clone())
        .execute(&owner, &mut stx)
        .expect("grant manage vk");

    let vk_box = VerifyingKeyBox::new(BACKEND.into(), stark_vk_bytes(STARK_TRANSFER_CIRCUIT));
    let commitment = iroha_core::zk::hash_vk(&vk_box);
    let schema_hash: [u8; 32] = CryptoHash::new(schema_descriptor).into();
    let mut rec = VerifyingKeyRecord::new_with_owner(
        1,
        STARK_TRANSFER_CIRCUIT,
        None,
        "test",
        BackendTag::Stark,
        "goldilocks",
        schema_hash,
        commitment,
    );
    rec.status = ConfidentialStatus::Active;
    rec.gas_schedule_id = Some("stark_default".to_owned());
    rec.vk_len = u32::try_from(vk_box.bytes.len()).expect("vk length fits u32");
    rec.key = Some(vk_box);
    executor
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: rec,
            }
            .into(),
        )
        .expect("register verifying key");

    executor
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            RegisterZkAsset::new(
                asset_def_id.clone(),
                ZkAssetMode::Hybrid,
                true,
                true,
                Some(vk_id.clone()),
                Some(vk_id.clone()),
                None,
            )
            .into(),
        )
        .expect("register zk asset");

    stx.apply();
    block.commit().expect("commit bootstrap block");

    (state, owner, asset_def_id, vk_id)
}

fn raw_stark_envelope_bytes() -> Vec<u8> {
    use iroha_core::zk_stark::{
        STARK_HASH_SHA256_V1, StarkCommitmentsV1, StarkFriParamsV1, StarkProofV1,
        StarkVerifyEnvelopeV1,
    };

    let env = StarkVerifyEnvelopeV1 {
        params: StarkFriParamsV1 {
            version: 1,
            n_log2: 1,
            blowup_log2: 1,
            fold_arity: 2,
            queries: 0,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "raw-envelope".to_owned(),
        },
        proof: StarkProofV1 {
            version: 1,
            commits: StarkCommitmentsV1 {
                version: 1,
                roots: vec![[0u8; 32]],
                comp_root: None,
            },
            queries: Vec::new(),
            comp_values: None,
        },
        transcript_label: "RAW-STARK".to_owned(),
    };
    norito::to_bytes(&env).expect("encode raw STARK envelope")
}

#[test]
fn zk_transfer_rejects_raw_stark_envelope() {
    let (state, owner, asset_def_id) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    // Proof bytes are a (Norito-encoded) raw STARK envelope, not an OpenVerifyEnvelope wrapper.
    let proof = ProofBox::new(BACKEND.into(), raw_stark_envelope_bytes());
    let vk_inline = VerifyingKeyBox::new(BACKEND.into(), vec![1, 2, 3, 4]);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);

    let transfer = ZkTransfer::new(asset_def_id, Vec::new(), vec![[1u8; 32]], attachment, None);

    let err = executor
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("raw STARK envelope must be rejected");
    let iroha_data_model::executor::ValidationFail::InstructionFailed(inner) = err else {
        panic!("unexpected error: {err:?}");
    };
    let iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(param) = inner
    else {
        panic!("unexpected error: {inner:?}");
    };
    let msg = param.to_string();
    assert!(
        msg.contains("invalid OpenVerifyEnvelope payload"),
        "unexpected error: {msg}"
    );
}

#[test]
fn zk_transfer_rejects_stark_open_verify_envelope_with_schema_hash_mismatch() {
    let expected_schema = b"schema:transfer:v1";
    let wrong_schema = b"schema:transfer:v2";
    let (state, owner, asset_def_id, vk_id) = prepare_state_with_bound_stark_vk(expected_schema);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let proof_payload = stark_open_verify_envelope_bytes(STARK_TRANSFER_CIRCUIT, wrong_schema);
    let proof = ProofBox::new(BACKEND.into(), proof_payload);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_id);
    let transfer = ZkTransfer::new(asset_def_id, Vec::new(), vec![[0x11; 32]], attachment, None);

    let err = executor
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("schema hash mismatch must be rejected before verification");
    let iroha_data_model::executor::ValidationFail::InstructionFailed(inner) = err else {
        panic!("unexpected error: {err:?}");
    };
    let iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message) =
        inner
    else {
        panic!("unexpected error: {inner:?}");
    };
    assert!(
        message.contains("public inputs schema hash mismatch"),
        "unexpected error: {message}"
    );
}

#[test]
fn unshield_rejects_stark_open_verify_envelope_with_schema_hash_mismatch() {
    let expected_schema = b"schema:unshield:v1";
    let wrong_schema = b"schema:unshield:v2";
    let (state, owner, asset_def_id, vk_id) = prepare_state_with_bound_stark_vk(expected_schema);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let proof_payload = stark_open_verify_envelope_bytes(STARK_TRANSFER_CIRCUIT, wrong_schema);
    let proof = ProofBox::new(BACKEND.into(), proof_payload);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_id);
    let unshield = Unshield::new(
        asset_def_id,
        owner.clone(),
        1u128,
        Vec::new(),
        attachment,
        None,
    );

    let err = executor
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect_err("schema hash mismatch must be rejected before verification");
    let iroha_data_model::executor::ValidationFail::InstructionFailed(inner) = err else {
        panic!("unexpected error: {err:?}");
    };
    let iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message) =
        inner
    else {
        panic!("unexpected error: {inner:?}");
    };
    assert!(
        message.contains("public inputs schema hash mismatch"),
        "unexpected error: {message}"
    );
}

#[test]
fn unshield_rejects_raw_stark_envelope() {
    let (state, owner, asset_def_id) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let proof = ProofBox::new(BACKEND.into(), raw_stark_envelope_bytes());
    let vk_inline = VerifyingKeyBox::new(BACKEND.into(), vec![1, 2, 3, 4]);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);

    let unshield = Unshield::new(
        asset_def_id,
        owner.clone(),
        1u128,
        Vec::new(),
        attachment,
        None,
    );
    let err = executor
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect_err("raw STARK envelope must be rejected");
    let iroha_data_model::executor::ValidationFail::InstructionFailed(inner) = err else {
        panic!("unexpected error: {err:?}");
    };
    let iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(param) = inner
    else {
        panic!("unexpected error: {inner:?}");
    };
    let msg = param.to_string();
    assert!(
        msg.contains("invalid OpenVerifyEnvelope payload"),
        "unexpected error: {msg}"
    );
}
