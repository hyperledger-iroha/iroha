#![doc = "ZK ballot should reject when the configured verifying key is not Active."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! ZK ballot should reject when the configured verifying key is not Active.

#[test]
#[allow(clippy::too_many_lines)]
fn zk_ballot_rejects_when_vk_not_active() {
    use base64::Engine;
    use iroha_core::{
        executor::Executor, kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute,
        state::State, tx::InstructionBox,
    };
    use iroha_data_model::{
        Registrable,
        confidential::ConfidentialStatus,
        isi::{governance::CastZkBallot, zk::CreateElection},
        permission::Permission,
        prelude::{Account, Domain},
        proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    // Minimal state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = iroha_core::state::World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query);
    state.gov.min_bond_amount = 0;

    // Configure gov: disallow unverified ballots (default is false already)
    let mut gov_cfg = state.gov.clone();
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.window_span = 100;
    gov_cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: "halo2/pasta/tiny-add".to_string(),
        name: "ballot_current".to_string(),
    });
    state.set_gov(gov_cfg);

    // Begin a block/tx
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().expect("permission name"),
        Json::new(()),
    );
    iroha_data_model::prelude::Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant vk manage");
    let perm = Permission::new(
        "CanManageParliament".parse().expect("permission name"),
        Json::new(()),
    );
    iroha_data_model::prelude::Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant parliament manage");

    // Register Active VK (with inline bytes to pass bytes-available gate)
    let backend = "halo2/pasta/tiny-add";
    let vk_id = VerifyingKeyId::new(backend, "ballot_current");
    let vk_box = VerifyingKeyBox::new(backend.into(), vec![1, 2, 3]);
    let commitment = iroha_core::zk::hash_vk(&vk_box);
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        "ballot_current",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x44; 32],
        commitment,
    );
    vk_record.vk_len = 3;
    vk_record.status = ConfidentialStatus::Active;
    vk_record.key = Some(vk_box);
    vk_record.gas_schedule_id = Some("halo2_default".into());
    let vk_key = vk_record.key.clone();
    let exec = Executor::default();
    let reg_instr: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
        .expect("register vk");

    // Create election referencing the same VK
    let create = CreateElection {
        election_id: "ref-vk".to_string(),
        options: 1,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create.execute(&ALICE_ID, &mut stx).expect("create ok");
    let mut vk_record2 = VerifyingKeyRecord::new(
        2,
        "ballot_current",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x44; 32],
        commitment,
    );
    vk_record2.vk_len = 3;
    vk_record2.status = ConfidentialStatus::Proposed;
    vk_record2.key = vk_key;
    vk_record2.gas_schedule_id = Some("halo2_default".into());
    let upd_instr: InstructionBox = iroha_data_model::isi::verifying_keys::UpdateVerifyingKey {
        id: vk_id.clone(),
        record: vk_record2,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), upd_instr)
        .expect("update vk");
    stx.world.governance_referenda_mut().insert(
        "ref-vk".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    // Submit ZK ballot with some non-empty base64 body
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode([0x01u8, 0x02, 0x03]);
    let cast = CastZkBallot {
        election_id: "ref-vk".to_string(),
        proof_b64,
        public_inputs_json: "{}".to_string(),
    };
    let result = cast.execute(&ALICE_ID, &mut stx);
    assert!(
        result.is_err(),
        "ballot with non-active VK must be rejected"
    );
}
