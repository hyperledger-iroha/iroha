//! Council gating for governance proposals: referenda open only after quorum.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use iroha_core::{
    governance::draw,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::blake2::{Blake2b512, Digest as _};
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
    let alice_account =
        Account::new(ALICE_ID.clone().to_account_id(domain_id.clone())).build(&ALICE_ID);
    let bob_account = Account::new(BOB_ID.clone().to_account_id(domain_id.clone())).build(&BOB_ID);
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

fn enable_parliament_module(state: &mut State) {
    let nexus = state.nexus.get_mut();
    let mut parliament_module = iroha_config::parameters::actual::GovernanceModule::default();
    parliament_module.module_type = Some("parliament".to_string());
    nexus.governance.default_module = Some("parliament".to_string());
    nexus.governance.modules = BTreeMap::from([("parliament".to_string(), parliament_module)]);
}

fn roster_root(bodies: &iroha_data_model::governance::types::ParliamentBodies) -> [u8; 32] {
    let encoded = norito::to_bytes(bodies).expect("encode roster bodies");
    let digest = Blake2b512::digest(encoded);
    let mut root = [0u8; 32];
    root.copy_from_slice(&digest[..32]);
    root
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
        parliament_snapshot: None,
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

#[test]
fn parliament_snapshot_allows_approvals_without_council_state() {
    let mut state = setup_council_state();
    enable_parliament_module(&mut state);
    let pid = [0xBC; 32];
    let rid = hex::encode(pid);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.citizens_mut().insert(
        ALICE_ID.clone(),
        iroha_core::state::CitizenshipRecord::new(ALICE_ID.clone(), 20_000, 1),
    );
    stx.world.citizens_mut().insert(
        BOB_ID.clone(),
        iroha_core::state::CitizenshipRecord::new(BOB_ID.clone(), 20_000, 1),
    );
    let referendum = iroha_core::state::GovernanceReferendumRecord {
        h_start: 1,
        h_end: 8,
        status: iroha_core::state::GovernanceReferendumStatus::Proposed,
        mode: iroha_core::state::GovernanceReferendumMode::Zk,
    };
    stx.world
        .governance_referenda_mut()
        .insert(rid.clone(), referendum);
    let selection_epoch = header.height().get();
    let beacon = [0x44; 32];
    let bodies = draw::derive_parliament_bodies_from_bonded_citizens(
        &stx.gov,
        &stx.chain_id,
        selection_epoch,
        &beacon,
        [(&*ALICE_ID, 20_000_u128), (&*BOB_ID, 20_000_u128)],
        iroha_data_model::isi::governance::CouncilDerivationKind::Vrf,
    );
    let pipeline = iroha_core::state::GovernancePipeline::seeded(
        header.height().get(),
        Some(&referendum),
        &stx.gov,
    );
    stx.world.governance_proposals_mut().insert(
        pid,
        iroha_core::state::GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                namespace: "apps".into(),
                contract_id: "jit-only.contract".into(),
                code_hash_hex: ContractCodeHash::from_hex_str(&hex::encode([0x77; 32]))
                    .expect("code hash"),
                abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode([0x88; 32]))
                    .expect("abi hash"),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: header.height().get(),
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline,
            parliament_snapshot: Some(iroha_core::state::GovernanceParliamentSnapshot {
                selection_epoch,
                beacon,
                roster_root: roster_root(&bodies),
                bodies: bodies.clone(),
            }),
        },
    );
    stx.apply();
    block.commit().expect("commit seed block");

    let rules_signer = bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("rules signer");
    let agenda_signer = bodies
        .rosters
        .get(&ParliamentBody::AgendaCouncil)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("agenda signer");

    approve_at_height(
        &mut state,
        2,
        ParliamentBody::RulesCommittee,
        pid,
        &rules_signer,
    );
    approve_at_height(
        &mut state,
        3,
        ParliamentBody::AgendaCouncil,
        pid,
        &agenda_signer,
    );

    assert!(
        state.view().world().council().get(&0).is_none(),
        "council state should not be required for parliament snapshot approvals",
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
}
