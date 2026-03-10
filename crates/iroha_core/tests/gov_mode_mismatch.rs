//! Governance referendum mode mismatch tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn plain_ballot_rejected_on_zk_referendum() {
    use core::num::NonZeroU64;

    use iroha_data_model::{
        events::data::governance::GovernanceEvent,
        isi::governance::{CastPlainBallot, ProposeDeployContract, VotingMode},
        permission::Permission,
        prelude::Grant,
    };
    use iroha_executor_data_model::permission::governance::{
        CanProposeContractDeployment, CanSubmitGovernanceBallot,
    };
    use iroha_test_samples::ALICE_ID;

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone().to_account_id(domain_id)).build(&ALICE_ID);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.conviction_step_blocks = 1;
    state.set_gov(cfg);

    // H=1: propose a Zk referendum
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 0;
    cfg.window_span = 10;
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.conviction_step_blocks = 1;
    state.set_gov(cfg);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let p1: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".into(),
    }
    .into();
    Grant::account_permission(p1, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant propose");
    let p2: Permission = CanSubmitGovernanceBallot {
        referendum_id: "any".into(),
    }
    .into();
    Grant::account_permission(p2, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot");
    ProposeDeployContract {
        namespace: "apps".into(),
        contract_id: "demo.contract".into(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".into(),
        window: None,
        mode: Some(VotingMode::Zk),
        manifest_provenance: None,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("propose");
    let rid = stx
        .world
        .governance_referenda()
        .iter()
        .next()
        .map(|(k, _)| k.clone())
        .expect("referendum created");
    let instr = CastPlainBallot {
        referendum_id: rid,
        owner: ALICE_ID.clone(),
        amount: 1000,
        duration_blocks: 10,
        direction: 0,
    };
    let err = instr.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("referendum mode mismatch"));
    stx.apply();
    let events = sblock.world.take_external_events();
    assert!(events.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::BallotRejected(_)
                )
            )
    )));
}
