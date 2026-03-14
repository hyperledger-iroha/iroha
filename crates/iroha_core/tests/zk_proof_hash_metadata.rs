//! Tests that ZK ISIs write metadata with a `proof_hash` field for auditability.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::{self, test_utils::halo2_fixture_envelope},
};
use iroha_data_model::{account::NewAccount, prelude::*};
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[test]
fn zk_transfer_and_unshield_emit_proof_hash_in_metadata() {
    // Minimal state
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

    // Seed: domain, account, asset def, mint, register ZK policy (Hybrid allow both)
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let domain_id: DomainId = "zkd".parse().unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "zkd".parse().unwrap(),
        "zcoin".parse().unwrap(),
    );
    let (owner, _owner_key) = gen_account_in("zkd");
    let init: [InstructionBox; 5] = [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new_in_domain(owner.clone(), domain_id.clone())).into(),
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
    ];
    for instr in init {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, instr)
            .expect("init ok");
    }

    // 1) ZkTransfer emits zk.transfer.last with proof_hash
    let transfer_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let pr_transfer = transfer_fixture.proof_box("halo2/ipa");
    let vk_transfer = transfer_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let attach_t = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        pr_transfer.clone(),
        vk_transfer,
    );
    let expected_hash_transfer = zk::hash_proof(&pr_transfer);
    let tx = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        vec![[5u8; 32]],
        vec![[9u8; 32]],
        attach_t,
        None,
    );
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, tx.into())
        .expect("transfer ok");
    stx.apply();
    block.commit().expect("commit setup block");

    {
        let view = state.view();
        let def = view.world.asset_definitions().get(&asset_def_id).unwrap();
        let key_t: iroha_data_model::name::Name = "zk.transfer.last".parse().unwrap();
        let val_t = def
            .metadata()
            .get(&key_t)
            .expect("zk.transfer.last present");
        let obj_t: norito::json::Value = val_t.try_into_any_norito().expect("json decode");
        let got_hash_t = obj_t
            .get("proof_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(got_hash_t, hex::encode(expected_hash_transfer));
    }

    // 2) Unshield emits zk.unshield.last with proof_hash
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let unshield_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let pr_unshield = unshield_fixture.proof_box("halo2/ipa");
    let vk_unshield = unshield_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let attach_u = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        pr_unshield.clone(),
        vk_unshield,
    );
    let expected_hash_unshield = zk::hash_proof(&pr_unshield);
    let un = iroha_data_model::isi::zk::Unshield::new(
        asset_def_id.clone(),
        owner.clone(),
        123u128,
        vec![[6u8; 32]],
        attach_u,
        None,
    );
    stx2.world
        .executor()
        .clone()
        .execute_instruction(&mut stx2, &owner, un.into())
        .expect("unshield ok");
    stx2.apply();
    block2.commit().expect("commit unshield block");

    let view2 = state.view();
    let def2 = view2.world.asset_definitions().get(&asset_def_id).unwrap();
    let key_u: iroha_data_model::name::Name = "zk.unshield.last".parse().unwrap();
    let val_u = def2
        .metadata()
        .get(&key_u)
        .expect("zk.unshield.last present");
    let obj_u: norito::json::Value = val_u.try_into_any_norito().expect("json decode");
    let got_hash_u = obj_u
        .get("proof_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    assert_eq!(got_hash_u, hex::encode(expected_hash_unshield));
}
