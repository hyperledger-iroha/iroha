#![doc = "Gated test: `CastZkBallot` verifies via real VK path (tiny-add2inst public inputs) with dev toggle OFF.\nRequires Halo2 dev tests. Skipped by default; run with `IROHA_RUN_IGNORED=1`."]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "halo2-dev-tests")]

mod zk_testkit;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn zk_ballot_verifies_with_registered_vk_add2inst_public() {
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
        isi::{governance::CastZkBallot, verifying_keys, zk::CreateElection},
        prelude::{InstructionBox, PeerId},
    };
    use iroha_test_samples::ALICE_ID;

    // Build State (dev toggle OFF by default)
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(iroha_core::state::World::default(), kura, query);

    // Ensure dev toggle is OFF explicitly
    let bundle = zk_testkit::add2inst_public_bundle(5, 8);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: bundle.vk_id.name.clone(),
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

    let vk_id = bundle.vk_id.clone();
    let exec = Executor::default();
    let reg_instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
        .expect("register vk");

    // Create election with 1 option, use the same backend tag
    let create = CreateElection::new(
        "ref-vk".to_string(),
        1,
        bundle.root_bytes(),
        0,
        0,
        vk_id.clone(),
        vk_id.clone(),
        "gov:ballot:v1".to_string(),
    );
    create.execute(&ALICE_ID, &mut stx).expect("create ok");
    stx.world.governance_referenda_mut().insert(
        "ref-vk".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    // Cast ballot with base64-encoded ZK1
    let proof_b64 = bundle.proof_b64.clone();
    let public_inputs = norito::json::object([(
        "root_hint",
        norito::json::to_value(&hex::encode(bundle.root_bytes())).expect("serialize root_hint"),
    )])
    .expect("serialize public inputs");
    let public_inputs = norito::json::to_json(&public_inputs).expect("encode public inputs to JSON");
    let cast = CastZkBallot {
        election_id: "ref-vk".to_string(),
        proof_b64,
        public_inputs_json: public_inputs,
    };
    cast.execute(&ALICE_ID, &mut stx).expect("cast ok");
}
