#![doc = "Gated test: `CastZkBallot` verifies via the production Halo2/IPA vote envelope.\nRequires Halo2 dev tests. Skipped by default; run with `IROHA_RUN_IGNORED=1`."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "halo2-dev-tests")]

mod zk_testkit;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn zk_ballot_verifies_with_registered_production_vote_vk() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gated (IROHA_RUN_IGNORED!=1)");
        return;
    }

    use core::num::NonZeroU64;

    use iroha_core::{
        executor::Executor, kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute,
        state::State,
    };
    use iroha_data_model::{
        Registrable,
        block::BlockHeader,
        isi::{governance::CastZkBallot, verifying_keys, zk::CreateElection},
        permission::Permission,
        prelude::{Account, Domain, Grant, InstructionBox},
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanSubmitGovernanceBallot,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;

    // Build a state with production Halo2 verification enabled.
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id).build(&ALICE_ID);
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = iroha_core::state::World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query);
    state.gov.min_bond_amount = 0_u64.into();
    state.zk.halo2.enabled = true;

    let bundle = zk_testkit::vote_merkle8_bundle();
    let mut gov_cfg = state.gov.clone();
    gov_cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: bundle.vk_id.name.clone(),
    });
    state.set_gov(gov_cfg);

    // Begin a transaction context
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let vk_id = bundle.vk_id.clone();
    let manage_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(manage_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let exec = Executor::default();
    let reg_instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
        .expect("register vk");
    let parliament_permission: Permission = CanManageParliament.into();
    Grant::account_permission(parliament_permission, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_permission: Permission = CanSubmitGovernanceBallot {
        referendum_id: "ref-vk".to_string(),
    }
    .into();
    Grant::account_permission(ballot_permission, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");

    // Create election with 1 option, use the same backend tag
    let create = CreateElection {
        election_id: "ref-vk".to_string(),
        options: 1,
        eligible_root: bundle.root_bytes(),
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
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

    // Cast ballot with a base64-encoded OpenVerifyEnvelope.
    let proof_b64 = bundle.proof_b64();
    let public_inputs = norito::json::object([(
        "root_hint",
        norito::json::to_value(&hex::encode(bundle.root_bytes())).expect("serialize root_hint"),
    )])
    .expect("serialize public inputs");
    let public_inputs =
        norito::json::to_json(&public_inputs).expect("encode public inputs to JSON");
    let cast = CastZkBallot {
        election_id: "ref-vk".to_string(),
        proof_b64,
        public_inputs_json: public_inputs,
    };
    cast.execute(&ALICE_ID, &mut stx).expect("cast ok");
}
