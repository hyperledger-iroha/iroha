#![doc = "Gated test: `FinalizeElection` verifies tally proof via real VK (tiny-add public input).\nRequires Halo2 dev tests. Skipped by default; run with `IROHA_RUN_IGNORED=1`."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn zk_finalize_verifies_with_inline_vk_public_input() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gated (IROHA_RUN_IGNORED!=1)");
        return;
    }

    use core::num::NonZeroU64;

    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, WorldReadOnly},
        zk::test_utils::halo2_fixture_envelope,
    };
    use iroha_data_model::{
        block::BlockHeader,
        confidential::ConfidentialStatus,
        isi::{
            verifying_keys,
            zk::{CreateElection, FinalizeElection},
        },
        permission::Permission,
        prelude::Grant,
        proof::{ProofAttachment, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(iroha_core::state::World::default(), kura, query);

    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-public-v1", [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_commitment = iroha_core::zk::hash_vk(&vk_box);
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_tally");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        "halo2/pasta/tiny-add-public-v1",
        BackendTag::Halo2IpaPasta,
        "pallas",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = fixture.proof_bytes.len() as u32;
    vk_record.gas_schedule_id = Some("halo2_default".into());
    vk_record.key = Some(vk_box.clone());
    vk_record.status = ConfidentialStatus::Active;
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant vk permission");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant parliament permission");
    verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register vk");

    // Create election with 1 option.
    let create = CreateElection {
        election_id: "ref-final".to_string(),
        options: 1,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create.execute(&ALICE_ID, &mut stx).expect("create ok");

    // Finalize with tally [4] and inline VK
    let att =
        ProofAttachment::new_inline("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_box);
    let fin = FinalizeElection {
        election_id: "ref-final".to_string(),
        tally: vec![4],
        tally_proof: att,
    };
    fin.execute(&ALICE_ID, &mut stx).expect("finalize ok");

    // Assert finalized
    let st = stx
        .world
        .elections()
        .get(&"ref-final".to_string())
        .cloned()
        .unwrap();
    assert!(st.finalized);
    assert_eq!(st.tally, vec![4]);
}
