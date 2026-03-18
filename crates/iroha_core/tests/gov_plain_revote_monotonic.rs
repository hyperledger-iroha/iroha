//! Plain ballot re-vote monotonicity tests (extend-only and owner check).
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
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::governance::CastPlainBallot,
    permission::Permission,
    prelude::{Account, Domain, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

#[test]
fn plain_ballot_revotes_extend_only_and_owner_matches() {
    // Minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account: Account =
        Account::new(ALICE_ID.clone().to_account_id(domain_id.clone())).build(&ALICE_ID);
    let bob_account: Account =
        Account::new(BOB_ID.clone().to_account_id(domain_id)).build(&ALICE_ID);
    let world = World::with([domain], [alice_account, bob_account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0;
    gov_cfg.conviction_step_blocks = 1;
    state.set_gov(gov_cfg);

    // Build a signed block header at H=1
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
    let rid = "rid-revote".to_string();
    stx.world.governance_referenda_mut().insert(
        rid.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 10_000,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );

    // First vote by ALICE
    let first = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 100,
        duration_blocks: 200,
        direction: 0,
    };
    first
        .execute(&ALICE_ID, &mut stx)
        .expect("first ballot should succeed");
    let events = stx.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockCreated(_)))
    )));

    // Re-vote with shorter duration should be rejected
    let shorter = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 100,
        duration_blocks: 10,
        direction: 0,
    };
    let err = shorter.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("re-vote cannot reduce"));
    let evs_rej = stx.world.take_external_events();
    assert!(evs_rej.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(_)))
    )));

    // Re-vote with smaller amount should be rejected
    let smaller = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 50,
        duration_blocks: 200,
        direction: 0,
    };
    let err2 = smaller.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s2 = format!("{err2}");
    assert!(s2.contains("re-vote cannot reduce"));

    // Re-vote with longer duration (extend) should work and emit LockExtended
    let extend = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 120,
        duration_blocks: 400,
        direction: 0,
    };
    extend
        .execute(&ALICE_ID, &mut stx)
        .expect("extend should succeed");
    let evs_ext = stx.world.take_external_events();
    assert!(evs_ext.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockExtended(_)))
    )));

    // Owner mismatch must be rejected
    let mismatch = CastPlainBallot {
        referendum_id: rid,
        owner: BOB_ID.clone(),
        amount: 200,
        duration_blocks: 100,
        direction: 1,
    };
    let err3 = mismatch.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s3 = format!("{err3}");
    assert!(s3.contains("owner must equal authority"));
}
