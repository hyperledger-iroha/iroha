//! Plain ballot must fail when referendum is missing or closed.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    Registrable,
    block::BlockHeader,
    domain::DomainId,
    isi::governance::CastPlainBallot,
    permission::Permission,
    prelude::{Account, Domain, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn plain_ballot_rejected_when_referendum_absent_or_closed() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0;
    gov_cfg.conviction_step_blocks = 1;
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "any".to_string(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot permission");

    // No referendum exists
    let ballot = CastPlainBallot {
        referendum_id: "missing".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10,
        duration_blocks: 10,
        direction: 0,
    };
    let err = ballot
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum is absent");
    assert!(
        err.to_string().contains("referendum"),
        "unexpected error: {err}"
    );

    // Insert a closed referendum and ensure rejection
    stx.world.governance_referenda_mut().insert(
        "closed".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
    let ballot_closed = CastPlainBallot {
        referendum_id: "closed".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10,
        duration_blocks: 10,
        direction: 0,
    };
    let err_closed = ballot_closed
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum is closed");
    assert!(
        err_closed.to_string().contains("closed"),
        "unexpected error: {err_closed}"
    );
}
