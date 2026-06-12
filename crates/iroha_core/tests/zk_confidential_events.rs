#![doc = "Regression tests covering confidential event emission for shield, transfer, and unshield flows."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::confidential_v2,
};
use iroha_crypto::Hash as CryptoHash;
use iroha_data_model::{
    account::NewAccount,
    block::BlockHeader,
    confidential::ConfidentialEncryptedPayload,
    events::{
        EventBox,
        data::{DataEvent, confidential::ConfidentialEvent},
    },
    isi::{
        Grant, Mint,
        register::Register,
        verifying_keys,
        zk::{self, RegisterZkAsset},
    },
    permission::Permission,
    prelude::*,
    proof::{ProofAttachment, VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const HALO2_BACKEND: &str = "halo2/ipa";
const TEST_CHAIN_ID: &str = "confidential_chain";
const TRANSFER_VK_NAME: &str = "transfer_vk";
const UNSHIELD_VK_NAME: &str = "unshield_vk";

#[derive(Clone, Copy)]
struct ConfidentialNoteFixture {
    spend_key: [u8; 32],
    rho: [u8; 32],
    diversifier: [u8; 32],
    amount: u128,
    commitment: [u8; 32],
}

fn encrypted_payload(seed: u8) -> ConfidentialEncryptedPayload {
    let mut nonce = [0_u8; 24];
    nonce.fill(seed);
    let mut ciphertext = b"zk-confidential-events-payload-v1".to_vec();
    ciphertext.extend_from_slice(&[seed; 32]);
    ConfidentialEncryptedPayload::new([1_u8; 32], nonce, ciphertext)
}

fn transfer_vk_record() -> VerifyingKeyRecord {
    confidential_v2::confidential_transfer_v2_vk_record(TRANSFER_VK_NAME, 1)
        .expect("confidential transfer v2 verifying key record")
}

fn unshield_vk_record() -> VerifyingKeyRecord {
    confidential_v2::confidential_unshield_v2_vk_record(UNSHIELD_VK_NAME, 1)
        .expect("confidential unshield v2 verifying key record")
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

fn seed_shielded_note(
    state: &State,
    account_id: &AccountId,
    asset_def_id: &AssetDefinitionId,
    note: ConfidentialNoteFixture,
    block_height: u64,
) -> [u8; 32] {
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(block_height).expect("block height must be non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let shield = zk::Shield::new(
        asset_def_id.clone(),
        account_id.clone(),
        note.amount,
        note.commitment,
        encrypted_payload(note.rho[0]),
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, account_id, shield.into())
        .expect("seed shield");
    stx.apply();
    block.commit().expect("commit seed shield block");
    confidential_v2::compute_confidential_root_v2(&[note.commitment])
        .expect("single-note confidential root")
}

fn setup_state() -> (State, AccountId, iroha_crypto::KeyPair, AssetDefinitionId) {
    let (account_id, keypair) = gen_account_in("zkd");
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(World::new(), kura, query, ChainId::from(TEST_CHAIN_ID));

    // ZkTransfer/Unshield execute real proof verification under `verify_backend_with_timing_checked`,
    // so these tests must opt into the halo2 verifier explicitly.
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = std::time::Duration::ZERO;

    let domain_id: DomainId = DomainId::try_new("zkd", "universal").unwrap();
    let asset_def_id =
        AssetDefinitionId::new(domain_id.clone(), "zcoin".parse().expect("asset name"));
    let vk_transfer_id = VerifyingKeyId::new(HALO2_BACKEND, TRANSFER_VK_NAME);
    let vk_unshield_id = VerifyingKeyId::new(HALO2_BACKEND, UNSHIELD_VK_NAME);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let asset_id = AssetId::of(asset_def_id.clone(), account_id.clone());
    let instructions: Vec<InstructionBox> = vec![
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(account_id.clone())).into(),
        Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(())),
            account_id.clone(),
        )
        .into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone()).with_name("zcoin".to_owned()),
        )
        .into(),
        Mint::asset_numeric(10_000u64, asset_id).into(),
        verifying_keys::RegisterVerifyingKey {
            id: vk_transfer_id.clone(),
            record: transfer_vk_record(),
        }
        .into(),
        verifying_keys::RegisterVerifyingKey {
            id: vk_unshield_id.clone(),
            record: unshield_vk_record(),
        }
        .into(),
        RegisterZkAsset::new(
            asset_def_id.clone(),
            zk::ZkAssetMode::Hybrid,
            true,
            true,
            Some(vk_transfer_id),
            Some(vk_unshield_id),
            None,
        )
        .into(),
    ];

    let executor = stx.world.executor().clone();
    for instr in instructions {
        executor
            .clone()
            .execute_instruction(&mut stx, &account_id, instr)
            .expect("setup instruction must succeed");
    }
    stx.apply();
    block.commit().expect("commit setup block");
    {
        let view = state.view();
        assert!(
            view.world.asset_definitions().get(&asset_def_id).is_some(),
            "asset definition must exist after setup"
        );
    }

    (state, account_id, keypair, asset_def_id)
}

#[test]
fn shield_emits_confidential_event() {
    let (state, account_id, keypair, asset_def_id) = setup_state();
    let commitment = [0xABu8; 32];
    let instruction = InstructionBox::from(zk::Shield::new(
        asset_def_id.clone(),
        account_id.clone(),
        123u128,
        commitment,
        encrypted_payload(0xAB),
    ));
    let chain_id = ChainId::from("confidential_chain");
    let tx = TransactionBuilder::new(chain_id, account_id.clone())
        .with_instructions([instruction])
        .sign(keypair.private_key());
    let tx_call_hash = tx.hash_as_entrypoint();
    let acc_tx = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let block = BlockBuilder::new(vec![acc_tx])
        .chain(0, None)
        .sign(keypair.private_key())
        .unpack(|_| {});
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());
    sb.commit().expect("commit shield block");

    let shield_event = extract_confidential_event(events, |ev| match ev {
        ConfidentialEvent::Shielded(shielded) => Some(shielded),
        _ => None,
    })
    .expect("shield event expected");

    assert_eq!(shield_event.asset_definition, asset_def_id);
    assert_eq!(shield_event.account, account_id);
    assert_eq!(shield_event.commitment, commitment);
    assert!(shield_event.root_before.is_none());

    let latest_root = state
        .view()
        .world
        .zk_assets()
        .get(&shield_event.asset_definition)
        .and_then(|st| st.root_history.last().copied())
        .unwrap();
    assert_eq!(shield_event.root_after, latest_root);

    let mut expected_call_hash = [0u8; 32];
    let tx_call_hash_bytes: CryptoHash = tx_call_hash.into();
    expected_call_hash.copy_from_slice(tx_call_hash_bytes.as_ref());
    assert_eq!(shield_event.call_hash, Some(expected_call_hash));
}

#[test]
fn transfer_emits_confidential_event() {
    let (state, account_id, keypair, asset_def_id) = setup_state();
    let note = note_fixture(&asset_def_id, 0x11, 0x21, b"transfer-input", 7);
    let root = seed_shielded_note(&state, &account_id, &asset_def_id, note, 2);
    let vk_record = transfer_vk_record();
    let vk_box = vk_record
        .key
        .clone()
        .expect("inline transfer verifying key");
    let recipient_owner_tag = confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
        &[0x44; 32],
        confidential_v2::derive_confidential_diversifier_v2(b"transfer-recipient"),
    )
    .expect("recipient owner tag");
    let change_owner_tag = confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
        &[0x55; 32],
        confidential_v2::derive_confidential_diversifier_v2(b"transfer-change"),
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
                amount: 4,
                rho: [0x31; 32],
                owner_tag: recipient_owner_tag,
            },
            confidential_v2::ConfidentialTransferOutputV2 {
                amount: 3,
                rho: [0x32; 32],
                owner_tag: change_owner_tag,
            },
        ],
        root,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("confidential transfer proof");
    let mut attachment = ProofAttachment::new_ref(
        HALO2_BACKEND.into(),
        proof.proof,
        VerifyingKeyId::new(HALO2_BACKEND, TRANSFER_VK_NAME),
    );
    attachment.vk_commitment = Some(vk_record.commitment);
    let outputs = proof.output_commitments.clone();
    let nullifiers = proof.nullifiers.clone();
    let instruction = InstructionBox::from(zk::ZkTransfer::new(
        asset_def_id.clone(),
        nullifiers.clone(),
        outputs.clone(),
        attachment.clone(),
        Some(root),
    ));
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let tx = TransactionBuilder::new(chain_id, account_id.clone())
        .with_instructions([instruction])
        .sign(keypair.private_key());
    let tx_call_hash = tx.hash_as_entrypoint();
    let acc_tx = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let block = BlockBuilder::new(vec![acc_tx])
        .chain(0, None)
        .sign(keypair.private_key())
        .unpack(|_| {});
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());
    sb.commit().expect("commit transfer block");

    let transfer_event = extract_confidential_event(events, |ev| match ev {
        ConfidentialEvent::Transferred(transferred) => Some(transferred),
        _ => None,
    })
    .expect("transfer event expected");

    assert_eq!(transfer_event.asset_definition, asset_def_id);
    assert_eq!(transfer_event.nullifiers, nullifiers);
    let mut expected_outputs = outputs.clone();
    expected_outputs.sort_unstable();
    assert_eq!(transfer_event.outputs, expected_outputs);
    assert_eq!(transfer_event.root_before, Some(root));

    let latest_root = state
        .view()
        .world
        .zk_assets()
        .get(&transfer_event.asset_definition)
        .and_then(|st| st.root_history.last().copied())
        .unwrap();
    assert_eq!(transfer_event.root_after, latest_root);

    let expected_proof_hash = iroha_core::zk::hash_proof(&attachment.proof);
    assert_eq!(transfer_event.proof_hash, expected_proof_hash);

    let mut expected_call_hash = [0u8; 32];
    let tx_call_hash_bytes: CryptoHash = tx_call_hash.into();
    expected_call_hash.copy_from_slice(tx_call_hash_bytes.as_ref());
    assert_eq!(transfer_event.call_hash, Some(expected_call_hash));

    assert_eq!(transfer_event.envelope_hash, attachment.envelope_hash);
}

#[test]
fn unshield_emits_confidential_event() {
    let (state, account_id, keypair, asset_def_id) = setup_state();

    let note = note_fixture(&asset_def_id, 0x77, 0x88, b"unshield-input", 250);
    let root = seed_shielded_note(&state, &account_id, &asset_def_id, note, 2);
    let vk_record = unshield_vk_record();
    let vk_box = vk_record
        .key
        .clone()
        .expect("inline unshield verifying key");
    let proof = confidential_v2::build_confidential_unshield_proof_v2(
        &ChainId::from(TEST_CHAIN_ID),
        &asset_def_id.to_string(),
        &note.spend_key,
        &[note.commitment],
        &[confidential_v2::ConfidentialUnshieldInputV2 {
            amount: note.amount,
            rho: note.rho,
            diversifier: note.diversifier,
            leaf_index: 0,
        }],
        note.amount,
        root,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("confidential unshield proof");
    let nullifier = proof.nullifiers[0];
    let mut attachment = ProofAttachment::new_ref(
        HALO2_BACKEND.into(),
        proof.proof,
        VerifyingKeyId::new(HALO2_BACKEND, UNSHIELD_VK_NAME),
    );
    attachment.vk_commitment = Some(vk_record.commitment);
    let instruction = InstructionBox::from(zk::Unshield::new(
        asset_def_id.clone(),
        account_id.clone(),
        note.amount,
        proof.nullifiers.clone(),
        attachment.clone(),
        Some(root),
    ));
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let tx = TransactionBuilder::new(chain_id, account_id.clone())
        .with_instructions([instruction])
        .sign(keypair.private_key());
    let tx_call_hash = tx.hash_as_entrypoint();
    let acc_tx = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let block = BlockBuilder::new(vec![acc_tx])
        .chain(0, None)
        .sign(keypair.private_key())
        .unpack(|_| {});
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());
    sb.commit().expect("commit unshield block");

    let unshield_event = extract_confidential_event(events, |ev| match ev {
        ConfidentialEvent::Unshielded(unshielded) => Some(unshielded),
        _ => None,
    })
    .expect("unshield event expected");

    assert_eq!(unshield_event.asset_definition, asset_def_id);
    assert_eq!(unshield_event.account, account_id);
    assert_eq!(unshield_event.public_amount, note.amount);
    assert_eq!(unshield_event.nullifiers, vec![nullifier]);
    assert_eq!(unshield_event.root_hint, Some(root));
    assert_eq!(
        unshield_event.proof_hash,
        iroha_core::zk::hash_proof(&attachment.proof)
    );
    assert_eq!(unshield_event.envelope_hash, attachment.envelope_hash);

    let mut expected_call_hash = [0u8; 32];
    let tx_call_hash_bytes: CryptoHash = tx_call_hash.into();
    expected_call_hash.copy_from_slice(tx_call_hash_bytes.as_ref());
    assert_eq!(unshield_event.call_hash, Some(expected_call_hash));
}

fn extract_confidential_event<F, T>(events: Vec<EventBox>, select: F) -> Option<T>
where
    F: Fn(ConfidentialEvent) -> Option<T>,
{
    events.into_iter().find_map(|event| match event {
        EventBox::Data(data) => match data.as_ref() {
            DataEvent::Confidential(conf) => select(conf.clone()),
            _ => None,
        },
        _ => None,
    })
}
