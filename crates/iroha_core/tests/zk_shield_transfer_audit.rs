//! Tests audit metadata for Shield and `ZkTransfer` include roots and commitments.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use std::str::FromStr;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::confidential_v2,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::NewAccount,
    isi::{Grant, verifying_keys},
    name::Name,
    permission::Permission,
    prelude::*,
    proof::{ProofAttachment, VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const HALO2_BACKEND: &str = "halo2/ipa";
const TEST_CHAIN_ID: &str = "confidential_chain";
const TRANSFER_VK_NAME: &str = "transfer_vk";

#[derive(Clone, Copy)]
struct ConfidentialNoteFixture {
    spend_key: [u8; 32],
    rho: [u8; 32],
    diversifier: [u8; 32],
    amount: u128,
    commitment: [u8; 32],
}

fn encrypted_payload(seed: u8) -> iroha_data_model::confidential::ConfidentialEncryptedPayload {
    let mut nonce = [0_u8; 24];
    nonce.fill(seed);
    let mut ciphertext = b"zk-shield-transfer-audit-payload-v1".to_vec();
    ciphertext.extend_from_slice(&[seed; 32]);
    iroha_data_model::confidential::ConfidentialEncryptedPayload::new([1_u8; 32], nonce, ciphertext)
}

fn checked_random_zk_shield_transfer_audit_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked zk shield-transfer audit keypair")
}

fn checked_random_zk_shield_transfer_audit_account_id() -> AccountId {
    AccountId::new(
        checked_random_zk_shield_transfer_audit_keypair()
            .public_key()
            .clone(),
    )
}

#[test]
fn zk_shield_transfer_audit_fixture_uses_checked_randomness() {
    let key_pair = checked_random_zk_shield_transfer_audit_keypair();
    assert_eq!(key_pair.public_key().algorithm(), Algorithm::Ed25519);
}

fn transfer_vk_record() -> VerifyingKeyRecord {
    confidential_v2::confidential_transfer_v2_vk_record(TRANSFER_VK_NAME, 1)
        .expect("confidential transfer v2 verifying key record")
}

fn note_fixture(
    asset_def_id: &AssetDefinitionId,
    spend_seed: u8,
    rho_seed: u8,
    diversifier_label: &[u8],
    amount: u128,
) -> ConfidentialNoteFixture {
    let spend_key = [spend_seed; 32];
    let rho = [rho_seed; 32];
    let diversifier = confidential_v2::derive_confidential_diversifier_v2(diversifier_label);
    let owner_tag =
        confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, diversifier)
            .expect("owner tag");
    let commitment = confidential_v2::derive_confidential_note_v2(
        &asset_def_id.to_string(),
        amount,
        rho,
        owner_tag,
    )
    .expect("note commitment");
    ConfidentialNoteFixture {
        spend_key,
        rho,
        diversifier,
        amount,
        commitment,
    }
}

#[test]
fn shield_and_transfer_emit_audit_roots_and_commitments() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(World::new(), kura, query, ChainId::from(TEST_CHAIN_ID));

    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = std::time::Duration::ZERO;

    // Seed domain/account/asset and mint; register ZK policy (Hybrid allow both)
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("zkd", "universal").unwrap(),
        "zcoin".parse().unwrap(),
    );
    let owner = checked_random_zk_shield_transfer_audit_account_id();
    let note = note_fixture(&asset_def_id, 0x21, 0x31, b"audit-transfer-input", 100);
    let vk_record = transfer_vk_record();
    let vk_transfer_id = VerifyingKeyId::new(HALO2_BACKEND, TRANSFER_VK_NAME);
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(())),
            owner.clone(),
        )
        .into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
        verifying_keys::RegisterVerifyingKey {
            id: vk_transfer_id.clone(),
            record: vk_record.clone(),
        }
        .into(),
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            asset_def_id.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            Some(vk_transfer_id.clone()),
            None,
            None,
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .expect("init ok");
    }

    // 1) Shield one commitment
    let shield = iroha_data_model::isi::zk::Shield::new(
        asset_def_id.clone(),
        owner.clone(),
        note.amount,
        note.commitment,
        encrypted_payload(5),
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, shield.into())
        .expect("shield ok");
    stx.apply();
    block.commit().expect("commit shield audit block");

    let view = state.view();
    let def = view.world.asset_definitions().get(&asset_def_id).unwrap();
    let key_s = Name::from_str("zk.shield.last").unwrap();
    let val_s = def.metadata().get(&key_s).expect("zk.shield.last present");
    let obj_s: norito::json::Value = val_s.try_into_any_norito().expect("json decode");
    // commitment hex must match
    let got_cm = obj_s.get("commitment").and_then(|v| v.as_str()).unwrap();
    assert_eq!(got_cm, hex::encode(note.commitment));
    // root_after equals latest root in WSV
    let st = view.world.zk_assets().get(&asset_def_id).unwrap();
    let latest = st.root_history.last().copied().unwrap();
    let root = confidential_v2::compute_confidential_root_v2(&[note.commitment])
        .expect("single-note confidential root");
    assert_eq!(latest, root);
    let got_after = obj_s.get("root_after").and_then(|v| v.as_str()).unwrap();
    assert_eq!(got_after, hex::encode(latest));

    // 2) ZkTransfer appends outputs and emits root_before/after and outputs_commitments
    let vk_box = vk_record
        .key
        .clone()
        .expect("inline transfer verifying key");
    let recipient_owner_tag = confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
        &[0x41; 32],
        confidential_v2::derive_confidential_diversifier_v2(b"audit-recipient"),
    )
    .expect("recipient owner tag");
    let change_owner_tag = confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
        &[0x42; 32],
        confidential_v2::derive_confidential_diversifier_v2(b"audit-change"),
    )
    .expect("change owner tag");
    let proof = confidential_v2::build_confidential_transfer_proof_v2(
        &ChainId::from(TEST_CHAIN_ID),
        &asset_def_id.to_string(),
        &note.spend_key,
        &[note.commitment],
        &[confidential_v2::ConfidentialTransferInputV2 {
            amount: note.amount,
            rho: note.rho,
            diversifier: note.diversifier,
            leaf_index: 0,
        }],
        &[
            confidential_v2::ConfidentialTransferOutputV2 {
                amount: 60,
                rho: [0x51; 32],
                owner_tag: recipient_owner_tag,
            },
            confidential_v2::ConfidentialTransferOutputV2 {
                amount: 40,
                rho: [0x52; 32],
                owner_tag: change_owner_tag,
            },
        ],
        root,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("confidential transfer proof");
    let mut att = ProofAttachment::new_ref(HALO2_BACKEND.into(), proof.proof, vk_transfer_id);
    att.vk_commitment = Some(vk_record.commitment);
    let outs = proof.output_commitments.clone();
    let transf = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        proof.nullifiers.clone(),
        outs.clone(),
        att,
        Some(root),
    );
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, transf.into())
        .expect("transfer ok");
    stx2.apply();
    block2.commit().expect("commit transfer audit block");

    let view2 = state.view();
    let def2 = view2.world.asset_definitions().get(&asset_def_id).unwrap();
    let key_t = Name::from_str("zk.transfer.last").unwrap();
    let val_t = def2
        .metadata()
        .get(&key_t)
        .expect("zk.transfer.last present");
    let obj_t: norito::json::Value = val_t.try_into_any_norito().expect("json decode");
    // outputs_commitments includes each output hex
    let arr = obj_t
        .get("outputs_commitments")
        .and_then(|v| v.as_array())
        .unwrap();
    let got: Vec<String> = arr
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let mut expected: Vec<String> = outs.iter().map(|c| hex::encode(c)).collect();
    expected.sort();
    assert_eq!(
        got, expected,
        "outputs must be emitted in deterministic order"
    );
    // root_after equals latest in WSV
    let st2 = view2.world.zk_assets().get(&asset_def_id).unwrap();
    let latest2 = st2.root_history.last().copied().unwrap();
    let got_after2 = obj_t.get("root_after").and_then(|v| v.as_str()).unwrap();
    assert_eq!(got_after2, hex::encode(latest2));
}
