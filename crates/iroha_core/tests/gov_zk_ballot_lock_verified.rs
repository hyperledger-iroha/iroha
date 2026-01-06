#![doc = "ZK ballot lock creation with verified proofs (commit/root public inputs) and lock hints.\nSkipped by default; run with `IROHA_RUN_IGNORED=1`. Requires Halo2 dev tests."]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "halo2-dev-tests")]

mod zk_testkit;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn zk_ballot_creates_and_extends_lock_on_verified_proof() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gated (IROHA_RUN_IGNORED!=1)");
        return;
    }

    use core::num::NonZeroU64;

    use iroha_core::{
        block::ValidBlock, executor::Executor, kura::Kura, query::store::LiveQueryStore,
        smartcontracts::Execute, state::State,
    };
    use iroha_data_model::{
        events::data::{DataEvent, governance::GovernanceEvent},
        isi::{governance::CastZkBallot, verifying_keys, zk::CreateElection},
        permission::Permission,
        prelude::{Grant, InstructionBox, PeerId},
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanSubmitGovernanceBallot,
    };
    use iroha_test_samples::ALICE_ID;

    // Build State (dev toggle OFF)
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(iroha_core::state::World::default(), kura, query);
    let bundle1 = zk_testkit::add2inst_public_bundle(5, 8);
    let bundle2 = zk_testkit::add2inst_public_bundle(6, 8);
    let bundle3 = zk_testkit::add2inst_public_bundle(7, 8);
    let root_hint = hex::encode(bundle1.root_bytes());
    let mut gov_cfg = state.gov.clone();
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.window_span = 100;
    gov_cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle1.backend.to_string(),
        name: bundle1.vk_id.name.clone(),
    });
    state.set_gov(gov_cfg);

    // Begin a transaction context
    let (pk, sk) = iroha_crypto::KeyPair::random().into_parts();
    let topo = iroha_core::sumeragi::network_topology::Topology::new(vec![PeerId::new(pk)]);
    let block = ValidBlock::new_dummy_and_modify_header(&sk, |h| {
        h.set_height(NonZeroU64::new(1).unwrap());
    })
    .commit(&topo)
    .unpack(|_| {})
    .unwrap();
    let mut sblock = state.block(block.as_ref().header());
    let mut stx = sblock.transaction();

    let vk_id = bundle1.vk_id.clone();
    let exec = Executor::default();
    let reg_instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle1.vk_record.clone(),
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
        .expect("register vk");
    let parliament_perm: Permission = CanManageParliament.into();
    Grant::account_permission(parliament_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "ref-zk-lock".to_string(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");

    // Create election with vk refs
    let create = CreateElection::new(
        "ref-zk-lock".to_string(),
        1,
        bundle1.root_bytes(),
        0,
        0,
        vk_id.clone(),
        vk_id.clone(),
        "gov:ballot:v1".to_string(),
    );
    create.execute(&ALICE_ID, &mut stx).expect("create ok");
    stx.world.governance_referenda_mut().insert(
        "ref-zk-lock".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    let proof_b64 = bundle1.proof_b64.clone();

    // Cast ballot with public inputs including owner/amount/duration
    let pub_inputs = norito::json::object([
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("serialize root"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "amount",
            norito::json::to_value(&1000u64).expect("serialize amount"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&200u64).expect("serialize duration"),
        ),
    ])
    .expect("serialize zk public inputs")
    .to_string();
    let cast = CastZkBallot {
        election_id: "ref-zk-lock".to_string(),
        proof_b64: proof_b64.clone(),
        public_inputs_json: pub_inputs,
    };
    cast.execute(&ALICE_ID, &mut stx).expect("cast ok");
    // Expect a lock exists for rid
    let rid = "ref-zk-lock".to_string();
    let locks = stx
        .world
        .governance_locks
        .get(&rid)
        .cloned()
        .expect("locks present");
    assert!(locks.locks.contains_key(&ALICE_ID));
    let created_events = stx.world.take_external_events();
    assert!(created_events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockCreated(_)))
    )));

    // Cast another ballot extending duration (should emit LockExtended)
    let pub_inputs2 = norito::json::object([
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("serialize root"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "amount",
            norito::json::to_value(&1200u64).expect("serialize amount"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&400u64).expect("serialize duration"),
        ),
    ])
    .expect("serialize zk public inputs")
    .to_string();
    let cast2 = CastZkBallot {
        election_id: "ref-zk-lock".to_string(),
        proof_b64: bundle2.proof_b64.clone(),
        public_inputs_json: pub_inputs2,
    };
    cast2.execute(&ALICE_ID, &mut stx).expect("extend ok");
    let events = stx.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockExtended(_)))
    )));

    // Attempt to reduce amount/duration (must be rejected)
    let shrink_inputs = norito::json::object([
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("serialize root"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "amount",
            norito::json::to_value(&900u64).expect("serialize amount"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&250u64).expect("serialize duration"),
        ),
    ])
    .expect("serialize shrink inputs")
    .to_string();
    let shrink = CastZkBallot {
        election_id: "ref-zk-lock".to_string(),
        proof_b64: bundle3.proof_b64.clone(),
        public_inputs_json: shrink_inputs,
    };
    let err = shrink
        .execute(&ALICE_ID, &mut stx)
        .expect_err("shrink must fail");
    assert!(format!("{err}").contains("re-vote cannot reduce existing lock"));
    let rejected = stx.world.take_external_events();
    assert!(rejected.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(rej)))
            if rej.reason.contains("re-vote cannot reduce")
    )));
}
