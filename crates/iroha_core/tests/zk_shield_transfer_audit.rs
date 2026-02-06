//! Tests audit metadata for Shield and `ZkTransfer` include roots and commitments.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use std::str::FromStr;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::{account::NewAccount, name::Name, prelude::*};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[test]
fn shield_and_transfer_emit_audit_roots_and_commitments() {
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

    // Seed domain/account/asset and mint; register ZK policy (Hybrid allow both)
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let domain_id: DomainId = "zkd".parse().unwrap();
    let asset_def_id: AssetDefinitionId = "zcoin#zkd".parse().unwrap();
    let owner: AccountId = "alice@zkd".parse().unwrap();
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(owner.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone())).into(),
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
            .expect("init ok");
    }

    // 1) Shield one commitment
    let cm = [5u8; 32];
    let shield = iroha_data_model::isi::zk::Shield::new(
        asset_def_id.clone(),
        owner.clone(),
        100u128,
        cm,
        iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, shield.into())
        .expect("shield ok");
    stx.apply();

    let view = state.view();
    let def = view.world.asset_definitions().get(&asset_def_id).unwrap();
    let key_s = Name::from_str("zk.shield.last").unwrap();
    let val_s = def.metadata().get(&key_s).expect("zk.shield.last present");
    let obj_s: norito::json::Value = val_s.try_into_any_norito().expect("json decode");
    // commitment hex must match
    let got_cm = obj_s.get("commitment").and_then(|v| v.as_str()).unwrap();
    assert_eq!(got_cm, hex::encode(cm));
    // root_after equals latest root in WSV
    let st = view.world.zk_assets().get(&asset_def_id).unwrap();
    let latest = st.root_history.last().copied().unwrap();
    let got_after = obj_s.get("root_after").and_then(|v| v.as_str()).unwrap();
    assert_eq!(got_after, hex::encode(latest));

    // 2) ZkTransfer appends outputs and emits root_before/after and outputs_commitments
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let pr = fixture.proof_box("halo2/ipa");
    let vk = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let att = iroha_data_model::proof::ProofAttachment::new_inline("halo2/ipa".into(), pr, vk);
    let outs = vec![[9u8; 32], [3u8; 32]];
    let transf = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        vec![],
        outs.clone(),
        att,
        None,
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
