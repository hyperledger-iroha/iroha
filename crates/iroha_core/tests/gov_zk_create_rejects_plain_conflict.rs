//! `CreateElection` must reject when a matching referendum exists in Plain mode.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
    zk::hash_vk,
};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::AssetDefinition,
    block::BlockHeader,
    confidential::ConfidentialStatus,
    domain::{Domain, DomainId},
    isi::{verifying_keys, zk::CreateElection},
    permission::Permission,
    prelude::Grant,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::CanManageParliament;
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;

#[test]
fn create_election_rejects_plain_conflict() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = "wonderland".parse().expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&iroha_test_samples::ALICE_ID);
    let account =
        Account::new(iroha_test_samples::ALICE_ID.clone()).build(&iroha_test_samples::ALICE_ID);
    let world = World::with([domain], [account], Vec::<AssetDefinition>::new());
    let state = State::new_for_testing(world, kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // Seed a Plain referendum with the same id
    stx.world.governance_referenda_mut().insert(
        "ref-conflict".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 10,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );

    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk-conflict");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        "vk-conflict",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x11; 32],
        hash_vk(&vk_box),
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("halo2_default".to_string());
    vk_record.key = Some(vk_box);
    vk_record.vk_len = vk_record.key.as_ref().map_or(0_u32, |k| {
        u32::try_from(k.bytes.len()).expect("vk length fits in u32")
    });
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_vk, iroha_test_samples::ALICE_ID.clone())
        .execute(&iroha_test_samples::ALICE_ID, &mut stx)
        .expect("grant vk permission");
    Grant::account_permission(perm_parliament, iroha_test_samples::ALICE_ID.clone())
        .execute(&iroha_test_samples::ALICE_ID, &mut stx)
        .expect("grant parliament permission");
    verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record.clone(),
    }
    .execute(&iroha_test_samples::ALICE_ID, &mut stx)
    .expect("register verifying key");

    let create = CreateElection {
        election_id: "ref-conflict".to_string(),
        options: 1,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    };
    let err = create
        .execute(&iroha_test_samples::ALICE_ID, &mut stx)
        .expect_err("plain-mode referendum conflict should fail");
    let s = format!("{err}");
    assert!(s.contains("mode mismatch"), "unexpected error message: {s}");
}
