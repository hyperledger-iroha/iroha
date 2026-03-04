//! Verifies that ZK asset operations enforce the configured verifying keys.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    account::NewAccount,
    asset::AssetDefinition,
    confidential::ConfidentialStatus,
    isi::{
        Grant, Register, verifying_keys,
        zk::{RegisterZkAsset, Unshield, ZkAssetMode, ZkTransfer},
    },
    permission::Permission,
    prelude::*,
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;

const BACKEND: &str = "halo2/ipa";
const FIXTURE_CIRCUIT: &str = "halo2/ipa:tiny-add-v1";

fn proof_fixture() -> iroha_core::zk::test_utils::FixtureEnvelope {
    halo2_fixture_envelope(FIXTURE_CIRCUIT, [0u8; 32])
}

fn prepare_state() -> (
    State,
    AccountId,
    AssetDefinitionId,
    VerifyingKeyId,
    VerifyingKeyId,
    VerifyingKeyId,
) {
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

    // These tests verify real Halo2 proofs via `verify_backend_with_timing_checked`, so enable the
    // halo2 verifier in the node config snapshot.
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = std::time::Duration::ZERO;

    let (vk_transfer_id, vk_unshield_id, vk_other_id, asset_def_id, owner) = {
        let domain_id: DomainId = "zkd".parse().unwrap();
        let asset_def_id: AssetDefinitionId = "zcoin#zkd".parse().unwrap();
        let owner_keypair = KeyPair::random();
        let owner: AccountId = AccountId::of(domain_id.clone(), owner_keypair.public_key().clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let result = {
            let mut stx = block.transaction();
            let executor = stx.world.executor().clone();

            let domain = Domain::new(domain_id.clone());
            executor
                .clone()
                .execute_instruction(&mut stx, &owner, Register::domain(domain).into())
                .expect("register domain");
            executor
                .clone()
                .execute_instruction(
                    &mut stx,
                    &owner,
                    Register::account(NewAccount::new(owner.clone())).into(),
                )
                .expect("register account");
            executor
                .clone()
                .execute_instruction(
                    &mut stx,
                    &owner,
                    Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone()))
                        .into(),
                )
                .expect("register asset definition");

            let perm = Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(()));
            Grant::account_permission(perm, owner.clone())
                .execute(&owner, &mut stx)
                .expect("grant manage vk");

            let fixture = halo2_fixture_envelope(FIXTURE_CIRCUIT, [0u8; 32]);
            let fixture_vk = fixture.vk_box(BACKEND).expect("fixture verifying key");
            let schema_hash = fixture.schema_hash;
            fn make_record(
                name: &str,
                vk_box: VerifyingKeyBox,
                schema_hash: [u8; 32],
            ) -> VerifyingKeyRecord {
                let commitment = iroha_core::zk::hash_vk(&vk_box);
                let vk_len = vk_box.bytes.len() as u32;
                let mut rec = VerifyingKeyRecord::new(
                    1,
                    format!("{BACKEND}:{name}"),
                    BackendTag::Halo2IpaPasta,
                    "pallas",
                    schema_hash,
                    commitment,
                );
                rec.vk_len = vk_len;
                rec.status = ConfidentialStatus::Active;
                rec.key = Some(vk_box);
                rec.gas_schedule_id = Some("halo2_default".into());
                rec
            }
            let vk_transfer_id = VerifyingKeyId::new(BACKEND, "vk_transfer");
            let vk_transfer_rec = make_record("vk_transfer", fixture_vk.clone(), schema_hash);
            executor
                .clone()
                .execute_instruction(
                    &mut stx,
                    &owner,
                    verifying_keys::RegisterVerifyingKey {
                        id: vk_transfer_id.clone(),
                        record: vk_transfer_rec,
                    }
                    .into(),
                )
                .expect("register vk_transfer");

            let vk_unshield_id = VerifyingKeyId::new(BACKEND, "vk_unshield");
            let vk_unshield_rec = make_record("vk_unshield", fixture_vk.clone(), schema_hash);
            executor
                .clone()
                .execute_instruction(
                    &mut stx,
                    &owner,
                    verifying_keys::RegisterVerifyingKey {
                        id: vk_unshield_id.clone(),
                        record: vk_unshield_rec,
                    }
                    .into(),
                )
                .expect("register vk_unshield");

            let vk_other_id = VerifyingKeyId::new(BACKEND, "vk_other");
            let vk_other_rec = make_record("vk_other", fixture_vk, schema_hash);
            executor
                .clone()
                .execute_instruction(
                    &mut stx,
                    &owner,
                    verifying_keys::RegisterVerifyingKey {
                        id: vk_other_id.clone(),
                        record: vk_other_rec,
                    }
                    .into(),
                )
                .expect("register vk_other");

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
                        Some(vk_transfer_id.clone()),
                        Some(vk_unshield_id.clone()),
                        None,
                    )
                    .into(),
                )
                .expect("register zk asset");

            stx.apply();
            (
                vk_transfer_id,
                vk_unshield_id,
                vk_other_id,
                asset_def_id,
                owner,
            )
        };
        block.commit().expect("commit block");
        result
    };

    (
        state,
        owner,
        asset_def_id,
        vk_transfer_id,
        vk_unshield_id,
        vk_other_id,
    )
}

#[test]
fn zk_transfer_accepts_expected_verifying_key() {
    let (state, owner, asset_def_id, vk_transfer_id, _, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_transfer_id.clone());
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        vec![[1u8; 32]],
        attachment,
        None,
    );

    executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect("transfer succeeds with matching vk");
    stx.apply();
}

#[test]
fn zk_transfer_accepts_inline_verifying_key() {
    let (state, owner, asset_def_id, _, _, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let vk_inline = fixture.vk_box(BACKEND).expect("fixture verifying key");
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        vec![[3u8; 32]],
        attachment,
        None,
    );

    executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect("transfer succeeds with inline vk that matches commitment");
    stx.apply();
}

#[test]
fn zk_transfer_rejects_mismatched_verifying_key() {
    let (state, owner, asset_def_id, _, _, vk_other_id) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_other_id.clone());
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        vec![[2u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("verifying key mismatch must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(
                msg.contains("verifying key reference mismatch")
                    || msg.contains("verifying key commitment mismatch")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zk_transfer_rejects_inline_commitment_mismatch() {
    let (state, owner, asset_def_id, _, _, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let vk_inline = VerifyingKeyBox::new(BACKEND.into(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        vec![[4u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("inline commitment mismatch must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("inline verifying key commitment mismatch"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zk_transfer_rejects_missing_verifying_key() {
    let (state, owner, asset_def_id, _, _, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment {
        backend: BACKEND.into(),
        proof,
        vk_ref: None,
        vk_inline: None,
        vk_commitment: None,
        envelope_hash: None,
        lane_privacy: None,
    };
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        vec![[5u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("missing verifying key must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("proof missing verifying key reference or inline key"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zk_transfer_rejects_invalid_proof() {
    let (state, owner, asset_def_id, vk_transfer_id, _, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let mut proof = proof_fixture().proof_box(BACKEND);
    proof.bytes.clear();
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_transfer_id.clone());
    let transfer = ZkTransfer::new(
        asset_def_id.clone(),
        vec![[7u8; 32]],
        vec![[8u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, transfer.into())
        .expect_err("invalid proof must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("invalid transfer proof"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unshield_accepts_expected_verifying_key() {
    let (state, owner, asset_def_id, _, vk_unshield_id, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_unshield_id.clone());
    let unshield = Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![[0u8; 32]],
        attachment,
        None,
    );

    executor
        .clone()
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect("unshield succeeds with matching vk");
    stx.apply();
}

#[test]
fn unshield_rejects_invalid_proof() {
    let (state, owner, asset_def_id, _, vk_unshield_id, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let mut proof = proof_fixture().proof_box(BACKEND);
    proof.bytes.clear();
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_unshield_id.clone());
    let unshield = Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![[0u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect_err("invalid proof must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("invalid unshield proof"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unshield_rejects_mismatched_verifying_key() {
    let (state, owner, asset_def_id, _, _vk_unshield_id, vk_other_id) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_ref(BACKEND.into(), proof, vk_other_id.clone());
    let unshield = Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![[0u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect_err("verifying key mismatch must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("verifying key reference mismatch"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unshield_accepts_inline_verifying_key() {
    let (state, owner, asset_def_id, _, _vk_unshield_id, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let vk_inline = fixture.vk_box(BACKEND).expect("fixture verifying key");
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);
    let unshield = Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![[0u8; 32]],
        attachment,
        None,
    );

    executor
        .clone()
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect("unshield succeeds with inline vk matching commitment");
    stx.apply();
}

#[test]
fn unshield_rejects_inline_commitment_mismatch() {
    let (state, owner, asset_def_id, _, _vk_unshield_id, _) = prepare_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let executor = stx.world.executor().clone();

    let fixture = proof_fixture();
    let vk_inline = VerifyingKeyBox::new(BACKEND.into(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let proof = fixture.proof_box(BACKEND);
    let attachment = ProofAttachment::new_inline(BACKEND.into(), proof, vk_inline);
    let unshield = Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        1u128,
        vec![[0u8; 32]],
        attachment,
        None,
    );

    let err = executor
        .clone()
        .execute_instruction(&mut stx, &owner, unshield.into())
        .expect_err("inline commitment mismatch must fail");
    match err {
        iroha_data_model::ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
        ) => {
            assert!(msg.contains("inline verifying key commitment mismatch"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
