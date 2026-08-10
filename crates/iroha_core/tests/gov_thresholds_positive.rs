//! Positive-path threshold test: approvals meet ratio and turnout.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        GovernanceProposalStatus, GovernanceReferendumMode, GovernanceReferendumRecord,
        GovernanceReferendumStatus, GovernanceStageApprovals, State, World, WorldReadOnly,
    },
};
use iroha_data_model::{
    Registrable,
    domain::DomainId,
    governance::types::ParliamentBody,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

fn proposal_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &iroha_test_samples::ALICE_ID,
        0,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .expect("proposal contract address")
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
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account: Account = Account::new(BOB_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account, bob_account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);

    // Set threshold num/den = 1/2, min_turnout=0 (defaults); ensure ratio 3/(3+1) >= 1/2
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0_u64.into();
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
        contract_address: proposal_contract_address(),
    }
    .into();
    Grant::account_permission(p1, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant propose");
    // Propose Plain-mode referendum
    ProposeDeployContract {
        contract_address: proposal_contract_address(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".to_string(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: None,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("propose");

    let proposal_id = {
        let mut proposals = stx.world.governance_proposals().iter();
        let proposal_id = *proposals.next().expect("proposal record").0;
        assert!(
            proposals.next().is_none(),
            "fixture must create exactly one proposal"
        );
        proposal_id
    };
    let rid = hex::encode(proposal_id);
    assert!(
        stx.world.governance_referenda().get(&rid).is_some(),
        "proposal must create its exact referendum"
    );

    let ballot_permission: Permission = CanSubmitGovernanceBallot {
        referendum_id: rid.clone(),
    }
    .into();
    Grant::account_permission(ballot_permission.clone(), ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot A");
    Grant::account_permission(ballot_permission, BOB_ID.clone())
        .execute(&BOB_ID, &mut stx)
        .expect("grant ballot B");

    let required_bodies = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
    ];
    let mut approvals = GovernanceStageApprovals::default();
    for body in required_bodies {
        approvals
            .ensure_stage(body, 0, 1, stx.gov.parliament_quorum_bps)
            .record(ALICE_ID.clone());
    }
    assert!(
        required_bodies
            .into_iter()
            .all(|body| approvals.quorum_met(body, 0))
    );
    stx.world
        .governance_stage_approvals_mut()
        .insert(rid.clone(), approvals);

    // Model the exact post-Parliament state with a one-block inclusive voting window.
    stx.world.governance_referenda_mut().insert(
        rid.clone(),
        GovernanceReferendumRecord {
            h_start: 1,
            h_end: 1,
            status: GovernanceReferendumStatus::Open,
            mode: GovernanceReferendumMode::Plain,
        },
    );
    // Duration 1 gives weights 6 and 2, preserving the 3/4 approval ratio.
    CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 9_u64.into(),
        duration_blocks: 1,
        direction: 0,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("alice ballot");
    CastPlainBallot {
        referendum_id: rid.clone(),
        owner: BOB_ID.clone(),
        amount: 1_u64.into(),
        duration_blocks: 1,
        direction: 1,
    }
    .execute(&BOB_ID, &mut stx)
    .expect("bob ballot");
    stx.apply();
    sblock.commit().expect("commit inclusive voting block");

    // H=2 is the first height after the inclusive voting window.
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("finalize ok");
    stx.apply();

    let evs = sblock.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalApproved(_)))
    )));
    let proposal = sblock
        .world
        .governance_proposals()
        .get(&proposal_id)
        .expect("proposal remains recorded");
    assert_eq!(proposal.status, GovernanceProposalStatus::Approved);
    let evidence = proposal
        .finalization_evidence
        .expect("approval retains finalization evidence");
    assert_eq!(evidence.proposal_id, proposal_id);
    assert_eq!(evidence.referendum_id, proposal_id);
    assert_eq!(evidence.finalized_at_height, 1);
    assert_eq!(
        (evidence.approve, evidence.reject, evidence.abstain),
        (6, 2, 0)
    );
    assert!(evidence.approved);
    assert_eq!(
        sblock
            .world
            .governance_referenda()
            .get(&rid)
            .expect("referendum remains recorded")
            .status,
        GovernanceReferendumStatus::Closed
    );
}
