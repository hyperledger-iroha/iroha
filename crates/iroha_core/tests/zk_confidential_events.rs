#![doc = "Regression tests covering confidential event emission for shield, transfer, and unshield flows."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
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
        Mint,
        register::Register,
        zk::{self, RegisterZkAsset},
    },
    prelude::*,
};
use iroha_test_samples::gen_account_in;
use iroha_zkp_halo2::confidential;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn setup_state() -> (State, AccountId, iroha_crypto::KeyPair, AssetDefinitionId) {
    let (account_id, keypair) = gen_account_in("zkd");
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

    let domain_id = account_id.domain.clone();
    let asset_def_id: AssetDefinitionId = format!("zcoin#{domain_id}").parse().unwrap();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let asset_id = AssetId::of(asset_def_id.clone(), account_id.clone());
    let instructions: [InstructionBox; 5] = [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(account_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone())).into(),
        Mint::asset_numeric(10_000u64, asset_id).into(),
        RegisterZkAsset::new(
            asset_def_id.clone(),
            zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
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
        ConfidentialEncryptedPayload::default(),
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
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let proof_box = fixture.proof_box("halo2/ipa");
    let vk = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk);
    let outputs = vec![[9u8; 32], [3u8; 32]];
    let nullifiers = vec![[1u8; 32], [2u8; 32]];
    let instruction = InstructionBox::from(zk::ZkTransfer::new(
        asset_def_id.clone(),
        nullifiers.clone(),
        outputs.clone(),
        attachment.clone(),
        None,
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
    assert!(transfer_event.root_before.is_none());

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

    // Seed a shielded note via direct execution so the ledger has commitments.
    {
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let shield = zk::Shield::new(
            asset_def_id.clone(),
            account_id.clone(),
            500u128,
            [0x11; 32],
            ConfidentialEncryptedPayload::default(),
        );
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &account_id, shield.into())
            .expect("seed shield");
        stx.apply();
        block.commit().expect("commit seed shield block");
    }

    let nk = [7u8; 32];
    let rho = [11u8; 32];
    let chain = "iroha-test-chain";
    let nullifier = derive_test_nullifier(&nk, &rho, &asset_def_id.to_string(), chain);
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let proof_box = fixture.proof_box("halo2/ipa");
    let vk = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk);
    let instruction = InstructionBox::from(zk::Unshield::new(
        asset_def_id.clone(),
        account_id.clone(),
        250u128,
        vec![nullifier],
        attachment.clone(),
        None,
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
    sb.commit().expect("commit unshield block");

    let unshield_event = extract_confidential_event(events, |ev| match ev {
        ConfidentialEvent::Unshielded(unshielded) => Some(unshielded),
        _ => None,
    })
    .expect("unshield event expected");

    assert_eq!(unshield_event.asset_definition, asset_def_id);
    assert_eq!(unshield_event.account, account_id);
    assert_eq!(unshield_event.public_amount, 250u128);
    assert_eq!(unshield_event.nullifiers, vec![nullifier]);
    assert!(unshield_event.root_hint.is_none());
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

fn derive_test_nullifier(
    nk: &[u8; 32],
    rho: &[u8; 32],
    asset_id: &str,
    chain_id: &str,
) -> [u8; 32] {
    confidential::derive_nullifier(nk, rho, asset_id.as_bytes(), chain_id.as_bytes())
}
