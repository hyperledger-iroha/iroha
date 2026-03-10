//! Positive-path threshold test: approvals meet ratio and turnout.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus, State,
        World, WorldReadOnly,
    },
};
use iroha_data_model::{
    Registrable,
    domain::DomainId,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn approves_when_ratio_and_turnout_met() {
    use core::num::NonZeroU64;

    use iroha_data_model::{
        events::data::{DataEvent, governance::GovernanceEvent},
        isi::governance::{CastPlainBallot, FinalizeReferendum, ProposeDeployContract, VotingMode},
        permission::Permission,
        prelude::Grant,
    };
    use iroha_executor_data_model::permission::governance::{
        CanProposeContractDeployment, CanSubmitGovernanceBallot,
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};

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

    // Set threshold num/den = 1/2, min_turnout=0 (defaults); ensure ratio 3/(3+1) >= 1/2
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.approval_threshold_q_num = 1;
    cfg.approval_threshold_q_den = 2;
    cfg.min_turnout = 0;
    cfg.conviction_step_blocks = 1;
    state.set_gov(cfg);

    // H=1: open a Plain referendum via propose
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
    // Grant permissions
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
    Grant::account_permission(p2.clone(), ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot A");
    Grant::account_permission(p2, BOB_ID.clone())
        .execute(&BOB_ID, &mut stx)
        .expect("grant ballot B");
    // Propose Plain-mode referendum
    ProposeDeployContract {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".to_string(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: None,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("propose");
    // Defer apply until after ballots and finalize

    // Approve: sqrt(9)=3 by ALICE; Reject: sqrt(1)=1 by BOB; ratio=3/4 >= 1/2
    // Discover referendum id (rid)
    let rid = stx
        .world
        .governance_referenda()
        .iter()
        .next()
        .map_or_else(|| "rid-threshold-pos".to_string(), |(k, _)| k.clone());
    stx.world.governance_referenda_mut().insert(
        rid.clone(),
        GovernanceReferendumRecord {
            h_start: 0,
            h_end: u64::MAX,
            status: GovernanceReferendumStatus::Open,
            mode: GovernanceReferendumMode::Plain,
        },
    );
    // Cast ballots: ALICE Aye with amount 9; BOB Nay with amount 1
    CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 9,
        duration_blocks: 10,
        direction: 0,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("alice ballot");
    CastPlainBallot {
        referendum_id: rid.clone(),
        owner: BOB_ID.clone(),
        amount: 1,
        duration_blocks: 10,
        direction: 1,
    }
    .execute(&BOB_ID, &mut stx)
    .expect("bob ballot");

    let instr = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id: [0xEF; 32],
    };
    instr.execute(&ALICE_ID, &mut stx).expect("finalize ok");
    stx.apply();
    let evs = sblock.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalApproved(_)))
    )));
}
