//! Tests for automatic ReferendumOpened/ReferendumClosed events via height triggers.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Skipped by default; enable with `IROHA_RUN_IGNORED=1`.
#![allow(
    clippy::doc_markdown,
    clippy::too_many_lines,
    clippy::items_after_statements
)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn referendum_open_and_close_by_height() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: referendum open/close test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    use core::num::NonZeroU64;

    use iroha_data_model::{
        Registrable,
        events::data::governance::GovernanceEvent,
        isi::governance::{CastPlainBallot, ProposeDeployContract, VotingMode},
        permission::Permission,
        prelude::{Account, Domain, Grant},
    };
    use iroha_executor_data_model::permission::governance::{
        CanProposeContractDeployment, CanSubmitGovernanceBallot,
    };

    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain: Domain = Domain::new(iroha_test_samples::ALICE_ID.domain.clone())
        .build(&iroha_test_samples::ALICE_ID);
    let account: Account =
        Account::new(iroha_test_samples::ALICE_ID.clone()).build(&iroha_test_samples::ALICE_ID);
    let world = World::with([domain], [account], []);
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        world,
        kura,
        query_handle,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(world, kura, query_handle);

    // Configure short schedule: start at +1, span 2 ⇒ end = start + 1
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    state.set_gov(cfg);

    // Block H=1: propose a Plain referendum
    let block1 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock1 = state.block(block1);
    let mut stx1 = sblock1.transaction();
    // Grant permissions for proposing and ballots
    let p1: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".into(),
    }
    .into();
    Grant::account_permission(p1, iroha_test_samples::ALICE_ID.clone())
        .execute(&iroha_test_samples::ALICE_ID, &mut stx1)
        .expect("grant propose");
    let p2: Permission = CanSubmitGovernanceBallot {
        referendum_id: "any".into(),
    }
    .into();
    Grant::account_permission(p2, iroha_test_samples::ALICE_ID.clone())
        .execute(&iroha_test_samples::ALICE_ID, &mut stx1)
        .expect("grant ballot");
    ProposeDeployContract {
        namespace: "apps".into(),
        contract_id: "demo.contract".into(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".into(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: None,
    }
    .execute(&iroha_test_samples::ALICE_ID, &mut stx1)
    .expect("propose");
    stx1.apply();
    // No open event at H=1
    assert!(sblock1.world.take_external_events().is_empty());

    // Block H=2: opens. Cast a plain ballot so that approve > reject at close
    let block2 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock2 = state.block(block2);
    {
        let rid = state
            .view()
            .world()
            .governance_referenda()
            .iter()
            .next()
            .map(|(k, _)| k.clone())
            .unwrap();
        let mut stx2 = sblock2.transaction();
        CastPlainBallot {
            referendum_id: rid,
            owner: iroha_test_samples::ALICE_ID.clone(),
            amount: 10000,
            duration_blocks: 10,
            direction: 0,
        }
        .execute(&iroha_test_samples::ALICE_ID, &mut stx2)
        .expect("ballot ok");
        stx2.apply();
    }
    let evs2 = sblock2.world.take_external_events();
    assert!(evs2.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::ReferendumOpened(_)
                )
            )
    )));

    // Block H=3: closes
    let header3 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(3).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock3 = state.block(header3);
    let evs3 = sblock3.world.take_external_events();
    assert!(evs3.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::ReferendumClosed(_)
                )
            )
    )));
    // Decision emitted automatically at h_end
    assert!(evs3.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::ProposalApproved(_)
                )
            )
    )));
}
