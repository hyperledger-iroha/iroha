//! Assert that `MetadataInserted` events for ZK ISIs carry the same JSON (including `proof_hash`).
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::{
    events::{EventBox, data::prelude::*},
    prelude::*,
};

#[allow(clippy::too_many_lines)]
#[test]
fn zk_events_carry_proof_hash_in_metadata_inserted() {
    // Build initial world with domain/account/asset definition
    let (authority_id, kp) = iroha_test_samples::gen_account_in("zkd");
    let domain: Domain = Domain::new("zkd".parse().unwrap()).build(&authority_id);
    let acc = Account::new(authority_id.clone()).build(&authority_id);
    let ad: AssetDefinition =
        AssetDefinition::new("zcoin#zkd".parse().unwrap(), NumericSpec::default())
            .build(&authority_id);
    let world = iroha_core::state::World::with([domain], [acc], [ad]);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let state = {
        #[cfg(feature = "telemetry")]
        {
            iroha_core::state::State::new(
                world,
                kura,
                query,
                iroha_core::telemetry::StateTelemetry::default(),
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            iroha_core::state::State::new(world, kura, query)
        }
    };

    // Prepare ZK ISIs: mint, register policy, transfer, unshield
    let asset_def_id: AssetDefinitionId = "zcoin#zkd".parse().unwrap();
    let asset = AssetId::of(asset_def_id.clone(), authority_id.clone());
    let transfer_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let pr_transfer = transfer_fixture.proof_box("halo2/ipa");
    let unshield_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let pr_unshield = unshield_fixture.proof_box("halo2/ipa");
    let vk_transfer = transfer_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let vk_unshield = unshield_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let attach_t = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        pr_transfer.clone(),
        vk_transfer,
    );
    let attach_u = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        pr_unshield.clone(),
        vk_unshield,
    );
    let expected_hash_transfer = iroha_core::zk::hash_proof(&pr_transfer);
    let expected_hash_unshield = iroha_core::zk::hash_proof(&pr_unshield);

    let instructions: Vec<InstructionBox> = vec![
        Mint::asset_numeric(10_000u64, asset.clone()).into(),
        InstructionBox::from(iroha_data_model::isi::zk::RegisterZkAsset::new(
            asset_def_id.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )),
        InstructionBox::from(iroha_data_model::isi::zk::ZkTransfer::new(
            asset_def_id.clone(),
            vec![[5u8; 32]],
            vec![[9u8; 32]],
            attach_t,
            None,
        )),
        InstructionBox::from(iroha_data_model::isi::zk::Unshield::new(
            asset_def_id.clone(),
            authority_id.clone(),
            123u128,
            vec![[6u8; 32]],
            attach_u,
            None,
        )),
    ];
    let tx = TransactionBuilder::new(ChainId::from("chain"), authority_id.clone())
        .with_instructions(instructions)
        .sign(kp.private_key());

    // Build, validate, commit a block with the single tx and capture events
    let tx_call_hash = tx.hash_as_entrypoint();
    let acc_tx = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = BlockBuilder::new(vec![acc_tx])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());

    // Extract Data events
    let data_events: Vec<_> = events
        .into_iter()
        .filter_map(|ev| match ev {
            EventBox::Data(d) => Some(d),
            _ => None,
        })
        .collect();

    // Find AssetDefinition::MetadataInserted for zk.transfer.last and zk.unshield.last
    let mut found_transfer = None;
    let mut found_unshield = None;
    for ev in data_events {
        if let DataEvent::Domain(DomainEvent::AssetDefinition(
            AssetDefinitionEvent::MetadataInserted(mc),
        )) = ev.as_ref()
        {
            let key_s = mc.key().as_ref();
            if key_s == "zk.transfer.last" {
                found_transfer = Some(mc.value().clone());
            } else if key_s == "zk.unshield.last" {
                found_unshield = Some(mc.value().clone());
            }
        }
    }
    let t_val = found_transfer.expect("zk.transfer.last MetadataInserted event present");
    let u_val = found_unshield.expect("zk.unshield.last MetadataInserted event present");

    // Both JSON payloads must carry proof_hash equal to the expected proof hash
    let t_obj: norito::json::Value = t_val.try_into_any_norito().expect("json decode");
    let u_obj: norito::json::Value = u_val.try_into_any_norito().expect("json decode");
    let t_hash = t_obj
        .get("proof_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let u_hash = u_obj
        .get("proof_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(t_hash, hex::encode(expected_hash_transfer));
    assert_eq!(u_hash, hex::encode(expected_hash_unshield));

    // Both JSON payloads must carry call_hash equal to the transaction call hash
    let t_ch = t_obj
        .get("call_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let u_ch = u_obj
        .get("call_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        t_ch,
        hex::encode(iroha_crypto::Hash::from(tx_call_hash).as_ref())
    );
    assert_eq!(
        u_ch,
        hex::encode(iroha_crypto::Hash::from(tx_call_hash).as_ref())
    );
}
