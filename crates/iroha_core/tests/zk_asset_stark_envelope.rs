//! Ensure shielded asset operations reject raw STARK envelopes and require `OpenVerifyEnvelope`.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "zk-stark"))]

use iroha_core::state::WorldReadOnly;
use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::State};
use iroha_data_model::{
    account::NewAccount,
    isi::zk::{RegisterZkAsset, Unshield, ZkAssetMode, ZkTransfer},
    prelude::*,
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
};
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;

const BACKEND: &str = "stark/fri-v1/sha256-goldilocks-v1";

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
