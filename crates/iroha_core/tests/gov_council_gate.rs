//! Council gating for governance proposals: referenda open only after quorum.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
        ProposalKind,
    },
    isi::governance::ApproveGovernanceProposal,
    prelude::*,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn setup_council_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
    let domain = Domain::new(domain_id).build(&ALICE_ID);
    let world = World::with(
        [domain],
        [alice_account, bob_account],
        std::iter::empty::<AssetDefinition>(),
    );
    let mut state = State::new_for_testing(world, kura, query);
    state.gov.min_enactment_delay = 1;
    state.gov.window_span = 4;
    state.gov.parliament_term_blocks = 10;
    state.gov.parliament_quorum_bps = 5_000;
    state
}

fn seed_referendum_and_proposal(state: &mut State, pid: [u8; 32], rid: &str) {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let council = iroha_core::governance::state::ParliamentTerm {
        epoch: 0,
        members: vec![ALICE_ID.clone(), BOB_ID.clone()],
        candidate_count: 2,
        ..Default::default()
    };
    stx.world.council_mut().insert(0, council);

    let h_start = header
        .height()
        .get()
        .saturating_add(state.gov.min_enactment_delay);
    let h_end = h_start.saturating_add(state.gov.window_span.saturating_sub(1));
    let referendum = iroha_core::state::GovernanceReferendumRecord {
        h_start,
        h_end,
        status: iroha_core::state::GovernanceReferendumStatus::Proposed,
        mode: iroha_core::state::GovernanceReferendumMode::Zk,
    };
    stx.world
        .governance_referenda_mut()
        .insert(rid.to_string(), referendum);
    let payload = DeployContractProposal {
        namespace: "apps".into(),
        contract_id: "calc.v1".into(),
        code_hash_hex: ContractCodeHash::from_hex_str(&hex::encode([0x11; 32])).expect("code hash"),
        abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode([0x22; 32])).expect("abi hash"),
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    };
    let pipeline = iroha_core::state::GovernancePipeline::seeded(
        header.height().get(),
        Some(&referendum),
        &stx.gov,
    );
    let proposal = iroha_core::state::GovernanceProposalRecord {
        proposer: ALICE_ID.clone(),
        kind: ProposalKind::DeployContract(payload),
        created_height: header.height().get(),
        status: iroha_core::state::GovernanceProposalStatus::Proposed,
        pipeline,
    };
    stx.world.governance_proposals_mut().insert(pid, proposal);
    stx.apply();
    block.commit().expect("commit genesis block");
}

fn referendum_status(state: &State, rid: &str) -> iroha_core::state::GovernanceReferendumStatus {
    state
        .view()
        .world()
        .governance_referenda()
        .get(rid)
        .copied()
        .expect("referendum present")
        .status
}

fn approve_at_height(
    state: &mut State,
    height: u64,
    body: ParliamentBody,
    pid: [u8; 32],
    signer: &AccountId,
) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    ApproveGovernanceProposal {
        body,
        proposal_id: pid,
    }
    .execute(signer, &mut stx)
    .expect("approval executes");
    stx.apply();
    block.commit().expect("commit approval block");
}

fn assert_quorum_records(state: &State, rid: &str) {
    let view = state.view();
    let approvals = view
        .world()
        .governance_stage_approvals()
        .get(rid)
        .expect("approval record");
    assert!(
        approvals.quorum_met(ParliamentBody::RulesCommittee, 0),
        "rules committee quorum recorded"
    );
    assert!(
        approvals.quorum_met(ParliamentBody::AgendaCouncil, 0),
        "agenda council quorum recorded"
    );
}

#[test]
fn referendum_opens_after_council_quorum() {
    let mut state = setup_council_state();
    let pid = [0xAA; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    let block2 = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    block2.commit().expect("commit height 2");
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );

    approve_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "rules committee quorum alone must not open the referendum"
    );

    approve_at_height(&mut state, 4, ParliamentBody::AgendaCouncil, pid, &BOB_ID);

    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
    assert_quorum_records(&state, &rid);
}
