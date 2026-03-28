//! `CreateElection` should seed a Zk referendum when none exists, using governance window defaults.
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
    domain::Domain,
    isi::{verifying_keys, zk::CreateElection},
    permission::Permission,
    prelude::Grant,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::CanManageParliament;
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[test]
fn create_election_inserts_referendum_with_configured_window() {
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let alice_id = iroha_test_samples::ALICE_ID.clone();
    let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&alice_id);
    let account = Account::new_in_domain(alice_id.clone(), domain_id).build(&alice_id);
    let world = World::with([domain], [account], Vec::<AssetDefinition>::new());
    let mut state = State::new_for_testing(world, kura, query_handle);
    // Configure governance window defaults
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 3;
    cfg.window_span = 5;
    state.set_gov(cfg);

    let header = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // No referendum exists yet
    assert!(
        stx.world
            .governance_referenda_mut()
            .get("ref-auto")
            .is_none()
    );

    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk-auto");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        "vk-auto",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x11; 32],
        hash_vk(&vk_box),
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.key = Some(vk_box);
    vk_record.vk_len = vk_record.key.as_ref().map_or(0_u32, |k| {
        u32::try_from(k.bytes.len()).expect("vk length fits in u32")
    });
    vk_record.max_proof_bytes = 1024;
    vk_record.gas_schedule_id = Some("halo2_default".into());
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_vk, alice_id.clone())
        .execute(&alice_id, &mut stx)
        .expect("grant vk permission");
    Grant::account_permission(perm_parliament, alice_id.clone())
        .execute(&alice_id, &mut stx)
        .expect("grant parliament permission");
    verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record.clone(),
    }
    .execute(&alice_id, &mut stx)
    .expect("register verifying key");

    let create = CreateElection {
        election_id: "ref-auto".to_string(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create
        .execute(&alice_id, &mut stx)
        .expect("create election seeds referendum");

    let rec = stx
        .world
        .governance_referenda_mut()
        .get("ref-auto")
        .copied()
        .expect("referendum inserted");
    assert_eq!(rec.mode, iroha_core::state::GovernanceReferendumMode::Zk);
    // h_start = current height (10) + min_enactment_delay (3) = 13
    assert_eq!(rec.h_start, 13);
    // h_end = h_start + span - 1 = 13 + 5 - 1 = 17
    assert_eq!(rec.h_end, 17);
    assert_eq!(
        rec.status,
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}
