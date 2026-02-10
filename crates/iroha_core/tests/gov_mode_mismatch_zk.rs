#![doc = "ZK ballot rejected on Plain-mode referendum (mode mismatch).\nSkipped by default; enable with `IROHA_RUN_IGNORED=1`."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "halo2-dev-tests")]
#![cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#![allow(clippy::items_after_statements)]

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
mod zk_testkit;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, StateReadOnly, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    prelude::{Account, Domain},
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn zk_ballot_rejected_on_plain_referendum() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk mode mismatch test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    use core::num::NonZeroU64;

    use iroha_data_model::{
        events::data::{DataEvent, governance::GovernanceEvent},
        isi::governance::{CastZkBallot, ProposeDeployContract, VotingMode},
        permission::Permission,
        prelude::Grant,
    };
    use iroha_executor_data_model::permission::governance::{
        CanProposeContractDeployment, CanSubmitGovernanceBallot,
    };
    use iroha_test_samples::ALICE_ID;

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain: Domain = Domain::new(ALICE_ID.domain.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let bundle = zk_testkit::tiny_add_bundle();
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.min_enactment_delay = 0;
    cfg.window_span = 10;
    let vk_name = bundle.vk_id.name.clone();
    cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: vk_name.clone(),
    });
    cfg.vk_tally = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: vk_name,
    });
    state.set_gov(cfg);

    // Create a block at H=1 and open a Plain referendum via ProposeDeployContract
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    // Grant permissions to ALICE to propose and submit ballots
    let p1: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".to_string(),
    }
    .into();
    Grant::account_permission(p1, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant propose");
    let p2: Permission = CanSubmitGovernanceBallot {
        referendum_id: "any".to_string(),
    }
    .into();
    Grant::account_permission(p2, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot");
    let manage_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(manage_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant manage vk");
    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: bundle.vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register vk");
    // Propose a Plain-mode referendum; record any created rid
    let prop = ProposeDeployContract {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".to_string(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: None,
    };
    prop.execute(&ALICE_ID, &mut stx).expect("propose");
    stx.apply();
    // Discover the referendum id (rid) created by proposal
    let view = state.view();
    let rid = view
        .world()
        .governance_referenda()
        .iter()
        .next()
        .map_or_else(|| "rid-plain".to_string(), |(k, _)| k.clone());
    // Valid proof bundle reused from TinyAdd helper
    let proof_b64 = bundle.proof_b64.clone();
    let mut stx2 = sblock.transaction();
    stx2.world.elections_mut().insert(
        rid.clone(),
        iroha_core::state::ElectionState {
            options: 1,
            eligible_root: [0u8; 32],
            start_ts: 0,
            end_ts: 0,
            finalized: false,
            tally: vec![0],
            ballot_nullifiers: std::collections::BTreeSet::default(),
            ciphertexts: Vec::new(),
            vk_ballot: Some(bundle.vk_id.clone()),
            vk_ballot_commitment: None,
            vk_tally: Some(bundle.vk_id.clone()),
            vk_tally_commitment: None,
            domain_tag: "gov:ballot:v1".to_string(),
        },
    );
    let public_inputs = "{}".to_string();
    let instr = CastZkBallot {
        election_id: rid.clone(),
        proof_b64,
        public_inputs_json: public_inputs,
    };
    let err = instr.execute(&ALICE_ID, &mut stx2).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("referendum mode mismatch"));
    stx2.apply();
    let events = sblock.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(_)))
    )));
}
