//! Plain ballot re-vote monotonicity and implicit-authority ownership tests.
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
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
#[test]
fn plain_ballot_revotes_extend_only_and_bind_owner_to_authority() {
    // Minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0_u64.into();
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
    stx.world.put_governance_referendum_for_testing(
        rid.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            // Keep the shortest re-vote valid for the inclusive referendum
            // window so the monotonic-lock check is the rejecting contract.
            h_end: 11,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            final_tally: None,
        },
    );
    // First vote by ALICE
    let first = CastPlainBallot {
        referendum_id: rid.clone(),
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 100_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(200).expect("non-zero lock duration"),
        },
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
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 100_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(10).expect("non-zero lock duration"),
        },
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
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 50_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(200).expect("non-zero lock duration"),
        },
    };
    let err2 = smaller.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s2 = format!("{err2}");
    assert!(s2.contains("re-vote cannot reduce"));
    // Re-vote with longer duration (extend) should work and emit LockExtended
    let extend = CastPlainBallot {
        referendum_id: rid.clone(),
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 120_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(400).expect("non-zero lock duration"),
        },
    };
    extend
        .execute(&ALICE_ID, &mut stx)
        .expect("extend should succeed");
    let evs_ext = stx.world.take_external_events();
    assert!(evs_ext.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockExtended(_)))
    )));
    let retained = stx
        .world
        .governance_locks
        .get(&rid)
        .and_then(|locks| locks.locks.get(&ALICE_ID))
        .expect("implicit-authority lock retained");
    assert_eq!(retained.owner, *ALICE_ID);
}
