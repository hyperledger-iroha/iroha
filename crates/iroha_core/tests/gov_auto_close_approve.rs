//! Auto-close approval test: decision at `h_end` without explicit finalize.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::too_many_lines, clippy::items_after_statements)]

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
fn auto_close_emits_approved() {
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
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    // Minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account: Account = Account::new(BOB_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account, bob_account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);

    // Use default thresholds (1/2) and min_turnout=0
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.approval_threshold_q_num = 1;
    cfg.approval_threshold_q_den = 2;
    cfg.min_turnout = 0;
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    state.set_gov(cfg);

    // H=1: propose a Plain referendum
    let block1 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut sblock1 = state.block(block1);
        let mut stx1 = sblock1.transaction();
        let p1: Permission = CanProposeContractDeployment {
            contract_id: "demo.contract".into(),
        }
        .into();
        Grant::account_permission(p1, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx1)
            .expect("grant propose");
        let p2: Permission = CanSubmitGovernanceBallot {
            referendum_id: "any".into(),
        }
        .into();
        Grant::account_permission(p2.clone(), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx1)
            .expect("grant ballot A");
        Grant::account_permission(p2, BOB_ID.clone())
            .execute(&BOB_ID, &mut stx1)
            .expect("grant ballot B");
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
        .execute(&ALICE_ID, &mut stx1)
        .expect("propose");
        stx1.apply();
        let _ = sblock1.commit();
    }

    // H=2: open + seed weights (approve sqrt(9)=3, reject sqrt(1)=1)
    let block2 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    // Configure small conviction step to allow short durations before casting ballots
    {
        let mut cfg2 = state.gov.clone();
        cfg2.conviction_step_blocks = 1;
        state.set_gov(cfg2);
    }
    let mut sblock2 = state.block(block2);
    {
        // Discover rid and cast ballots
        let rid = state
            .view()
            .world()
            .governance_referenda()
            .iter()
            .next()
            .map(|(k, _)| k.clone())
            .unwrap();
        let mut stx = sblock2.transaction();
        CastPlainBallot {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 9,
            duration_blocks: 200,
            direction: 0,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("alice");
        CastPlainBallot {
            referendum_id: rid.clone(),
            owner: BOB_ID.clone(),
            amount: 1,
            duration_blocks: 200,
            direction: 1,
        }
        .execute(&BOB_ID, &mut stx)
        .expect("bob");
        // Extend ALICE lock via instruction to produce a LockExtended event
        let instr = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 9,
            duration_blocks: 200,
            direction: 0,
        };
        instr
            .clone()
            .execute(&ALICE_ID, &mut stx)
            .expect("extend ok");
        stx.apply();
        let events2 = sblock2.world.take_external_events();
        assert!(events2.iter().any(|e| matches!(
            e,
            iroha_data_model::events::EventBox::Data(ev)
                if matches!(
                    ev.as_ref(),
                    iroha_data_model::events::data::DataEvent::Governance(
                        iroha_data_model::events::data::governance::GovernanceEvent::LockExtended(_)
                    )
                )
        )));
    }
    let _ = sblock2.commit();

    // H=3: auto close + ProposalApproved
    let block3 = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(3).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock3 = state.block(block3);
    let evs3 = sblock3.world.take_external_events();
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
