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
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBodies,
        ParliamentBody, ParliamentRoster, ProposalKind, RuntimeUpgradeProposal,
    },
    isi::governance::{
        ApproveGovernanceProposal, CastParliamentBallot, CouncilDerivationKind, ParliamentDecision,
    },
    prelude::*,
    runtime::RuntimeUpgradeManifest,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn sample_contract_address(
    account_id: &iroha_data_model::account::AccountId,
    deploy_nonce: u64,
) -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(
        iroha_config::parameters::defaults::common::chain_discriminant(),
        account_id,
        deploy_nonce,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .expect("sample contract address")
}

fn setup_council_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
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
    state.gov.window_span = 16;
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

fn deployment_stage_bodies() -> [ParliamentBody; 6] {
    [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
    ]
}

fn all_stage_bodies() -> [ParliamentBody; 7] {
    [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ]
}

fn roster_root(bodies: &iroha_data_model::governance::types::ParliamentBodies) -> [u8; 32] {
    let encoded = norito::to_bytes(bodies).expect("encode roster bodies");
    let digest = Blake2b512::digest(encoded);
    let mut root = [0u8; 32];
    root.copy_from_slice(&digest[..32]);
    root
}

fn sample_deploy_proposal(deploy_nonce: u64) -> ProposalKind {
    ProposalKind::DeployContract(DeployContractProposal {
        contract_address: sample_contract_address(&ALICE_ID, deploy_nonce),
        code_hash_hex: ContractCodeHash::from_hex_str(&hex::encode([0x11; 32])).expect("code hash"),
        abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode([0x22; 32])).expect("abi hash"),
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    })
}

fn sample_runtime_upgrade_proposal() -> ProposalKind {
    ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
        manifest: RuntimeUpgradeManifest {
            name: "parliament-gate-runtime".to_string(),
            description: "runtime upgrade gate fixture".to_string(),
            abi_version: 1,
            abi_hash: [0x42; 32],
            added_syscalls: Vec::new(),
            added_pointer_types: Vec::new(),
            start_height: 64,
            end_height: 128,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        },
    })
}

fn seed_referendum_and_proposal_with_kind(
    state: &mut State,
    pid: [u8; 32],
    rid: &str,
    kind: ProposalKind,
) {
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
    stx.world.parliament_bodies_mut().insert(
        0,
        ParliamentBodies {
            selection_epoch: 0,
            rosters: all_stage_bodies()
                .into_iter()
                .map(|body| {
                    (
                        body,
                        ParliamentRoster {
                            body,
                            epoch: 0,
                            members: vec![ALICE_ID.clone(), BOB_ID.clone()],
                            alternates: Vec::new(),
                            verified: 0,
                            candidate_count: 2,
                            derived_by: CouncilDerivationKind::Fallback,
                        },
                    )
                })
                .collect(),
        },
    );

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
    let pipeline = iroha_core::state::GovernancePipeline::seeded(
        header.height().get(),
        Some(&referendum),
        &stx.gov,
    );
    let proposal = iroha_core::state::GovernanceProposalRecord {
        proposer: ALICE_ID.clone(),
        kind,
        created_height: header.height().get(),
        status: iroha_core::state::GovernanceProposalStatus::Proposed,
        pipeline,
        parliament_snapshot: None,
    };
    stx.world.governance_proposals_mut().insert(pid, proposal);
    stx.apply();
    block.commit().expect("commit genesis block");
}

fn seed_referendum_and_proposal(state: &mut State, pid: [u8; 32], rid: &str) {
    seed_referendum_and_proposal_with_kind(state, pid, rid, sample_deploy_proposal(0));
}

fn seed_proposal_without_referendum(state: &mut State, pid: [u8; 32]) {
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
    stx.world.parliament_bodies_mut().insert(
        0,
        ParliamentBodies {
            selection_epoch: 0,
            rosters: all_stage_bodies()
                .into_iter()
                .map(|body| {
                    (
                        body,
                        ParliamentRoster {
                            body,
                            epoch: 0,
                            members: vec![ALICE_ID.clone(), BOB_ID.clone()],
                            alternates: Vec::new(),
                            verified: 0,
                            candidate_count: 2,
                            derived_by: CouncilDerivationKind::Fallback,
                        },
                    )
                })
                .collect(),
        },
    );

    let pipeline =
        iroha_core::state::GovernancePipeline::seeded(header.height().get(), None, &stx.gov);
    stx.world.governance_proposals_mut().insert(
        pid,
        iroha_core::state::GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: sample_deploy_proposal(0),
            created_height: header.height().get(),
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline,
            parliament_snapshot: None,
        },
    );
    stx.apply();
    block.commit().expect("commit proposal-only block");
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

fn cast_ballot_at_height(
    state: &mut State,
    height: u64,
    body: ParliamentBody,
    pid: [u8; 32],
    signer: &AccountId,
    decision: ParliamentDecision,
) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    CastParliamentBallot {
        body,
        proposal_id: pid,
        decision,
    }
    .execute(signer, &mut stx)
    .expect("parliament ballot executes");
    stx.apply();
    block.commit().expect("commit parliament ballot block");
}

fn expect_ballot_error_at_height(
    state: &State,
    height: u64,
    body: ParliamentBody,
    pid: [u8; 32],
    signer: &AccountId,
    decision: ParliamentDecision,
) -> String {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = CastParliamentBallot {
        body,
        proposal_id: pid,
        decision,
    }
    .execute(signer, &mut stx)
    .expect_err("parliament ballot must fail");
    format!("{err:?}")
}

fn set_body_roster_at_height(
    state: &mut State,
    height: u64,
    epoch: u64,
    body: ParliamentBody,
    members: Vec<AccountId>,
    alternates: Vec<AccountId>,
) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    let mut bodies = stx
        .world
        .parliament_bodies()
        .get(&epoch)
        .cloned()
        .expect("parliament bodies present");
    let roster = bodies.rosters.get_mut(&body).expect("body roster present");
    roster.members = members;
    roster.alternates = alternates;
    stx.world.parliament_bodies_mut().insert(epoch, bodies);
    stx.apply();
    block.commit().expect("commit roster update block");
}

fn remove_body_roster_at_height(state: &mut State, height: u64, epoch: u64, body: ParliamentBody) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    let mut bodies = stx
        .world
        .parliament_bodies()
        .get(&epoch)
        .cloned()
        .expect("parliament bodies present");
    bodies.rosters.remove(&body).expect("body roster present");
    stx.world.parliament_bodies_mut().insert(epoch, bodies);
    stx.apply();
    block.commit().expect("commit roster removal block");
}

fn mutate_body_roster_at_height(
    state: &mut State,
    height: u64,
    epoch: u64,
    body: ParliamentBody,
    mutate: impl FnOnce(&mut ParliamentRoster),
) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    let mut bodies = stx
        .world
        .parliament_bodies()
        .get(&epoch)
        .cloned()
        .expect("parliament bodies present");
    let roster = bodies.rosters.get_mut(&body).expect("body roster present");
    mutate(roster);
    stx.world.parliament_bodies_mut().insert(epoch, bodies);
    stx.apply();
    block.commit().expect("commit roster mutation block");
}

fn mutate_snapshot_at_height(
    state: &mut State,
    height: u64,
    pid: [u8; 32],
    mutate: impl FnOnce(&mut iroha_core::state::GovernanceParliamentSnapshot),
) {
    let height = NonZeroU64::new(height).expect("non-zero height");
    let mut block = state.block(BlockHeader::new(height, None, None, None, 0, 0));
    let mut stx = block.transaction();
    let mut proposal = stx
        .world
        .governance_proposals()
        .get(&pid)
        .cloned()
        .expect("proposal present");
    let snapshot = proposal
        .parliament_snapshot
        .as_mut()
        .expect("proposal snapshot present");
    mutate(snapshot);
    stx.world.governance_proposals_mut().insert(pid, proposal);
    stx.apply();
    block.commit().expect("commit snapshot mutation block");
}

fn assert_quorum_records(state: &State, rid: &str) {
    let view = state.view();
    let approvals = view
        .world()
        .governance_stage_approvals()
        .get(rid)
        .expect("approval record");
    for body in deployment_stage_bodies() {
        assert!(approvals.quorum_met(body, 0), "{body:?} quorum recorded");
    }
}

fn seed_snapshot_proposal(
    state: &mut State,
    pid: [u8; 32],
    rid: &str,
    deploy_nonce: u64,
    roster_root_override: Option<[u8; 32]>,
) -> iroha_data_model::governance::types::ParliamentBodies {
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
        .insert(rid.to_string(), referendum);
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
            kind: sample_deploy_proposal(deploy_nonce),
            created_height: header.height().get(),
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline,
            parliament_snapshot: Some(iroha_core::state::GovernanceParliamentSnapshot {
                selection_epoch,
                beacon,
                roster_root: roster_root_override.unwrap_or_else(|| roster_root(&bodies)),
                bodies: bodies.clone(),
            }),
        },
    );
    stx.apply();
    block.commit().expect("commit seed block");
    bodies
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
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "rules plus agenda quorum must not open the referendum"
    );

    for (idx, body) in [
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
    ]
    .into_iter()
    .enumerate()
    {
        approve_at_height(
            &mut state,
            5 + u64::try_from(idx).expect("idx fits"),
            body,
            pid,
            &ALICE_ID,
        );
    }

    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
    assert_quorum_records(&state, &rid);
}

#[test]
fn body_rejection_closes_and_prevents_later_opening() {
    let mut state = setup_council_state();
    let pid = [0xCD; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    CastParliamentBallot {
        body: ParliamentBody::RulesCommittee,
        proposal_id: pid,
        decision: ParliamentDecision::Reject,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("rejection ballot executes");
    stx.apply();
    block.commit().expect("commit rejection block");

    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Closed
    );
    assert!(matches!(
        state
            .view()
            .world()
            .governance_proposals()
            .get(&pid)
            .expect("proposal")
            .status,
        iroha_core::state::GovernanceProposalStatus::Rejected
    ));

    let mut block = state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = ApproveGovernanceProposal {
        body: ParliamentBody::AgendaCouncil,
        proposal_id: pid,
    }
    .execute(&BOB_ID, &mut stx)
    .expect_err("closed proposal cannot accept later approval");
    assert!(
        format!("{err:?}").contains("no longer open"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn duplicate_rejections_do_not_count_twice_for_rejection_quorum() {
    let mut state = setup_council_state();
    state.gov.parliament_quorum_bps = 10_000;
    let pid = [0xD1; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone(), BOB_ID.clone()],
        Vec::new(),
    );

    cast_ballot_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Reject,
    );
    cast_ballot_at_height(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Reject,
    );

    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules rejection record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert_eq!(
        rules.rejections.len(),
        1,
        "duplicate rejection from one signer must not satisfy quorum"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );

    cast_ballot_at_height(
        &mut state,
        5,
        ParliamentBody::RulesCommittee,
        pid,
        &BOB_ID,
        ParliamentDecision::Reject,
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Closed
    );
    assert!(matches!(
        state
            .view()
            .world()
            .governance_proposals()
            .get(&pid)
            .expect("proposal")
            .status,
        iroha_core::state::GovernanceProposalStatus::Rejected
    ));
}

#[test]
fn duplicate_approvals_do_not_count_twice_for_body_quorum() {
    let mut state = setup_council_state();
    state.gov.parliament_quorum_bps = 10_000;
    let pid = [0xD2; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone(), BOB_ID.clone()],
        Vec::new(),
    );

    approve_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
    );
    approve_at_height(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
    );

    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules approval record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert_eq!(
        rules.approvers.len(),
        1,
        "duplicate approval from one signer must not satisfy quorum"
    );
    assert_eq!(rules.required, 2);
    assert!(!rules.quorum_met());
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}

#[test]
fn changed_approval_to_rejection_removes_approval_and_closes() {
    let mut state = setup_council_state();
    let pid = [0xD3; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone()],
        Vec::new(),
    );

    cast_ballot_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    cast_ballot_at_height(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Reject,
    );

    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules decision record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert!(
        rules.approvers.is_empty(),
        "changed decision must remove the earlier approval"
    );
    assert!(rules.rejections.contains(&*ALICE_ID));
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Closed
    );
    assert!(matches!(
        state
            .view()
            .world()
            .governance_proposals()
            .get(&pid)
            .expect("proposal")
            .status,
        iroha_core::state::GovernanceProposalStatus::Rejected
    ));
}

#[test]
fn changed_rejection_to_abstain_removes_veto_pressure_without_closing() {
    let mut state = setup_council_state();
    state.gov.parliament_quorum_bps = 10_000;
    let pid = [0xD4; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone(), BOB_ID.clone()],
        Vec::new(),
    );

    cast_ballot_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Reject,
    );
    cast_ballot_at_height(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Abstain,
    );
    cast_ballot_at_height(
        &mut state,
        5,
        ParliamentBody::RulesCommittee,
        pid,
        &BOB_ID,
        ParliamentDecision::Reject,
    );

    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules decision record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert!(rules.approvers.is_empty());
    assert!(rules.abstentions.contains(&*ALICE_ID));
    assert!(!rules.rejections.contains(&*ALICE_ID));
    assert_eq!(
        rules.rejections.len(),
        1,
        "rescinded rejection must not combine with a later signer to fabricate quorum"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
    assert!(!matches!(
        state
            .view()
            .world()
            .governance_proposals()
            .get(&pid)
            .expect("proposal")
            .status,
        iroha_core::state::GovernanceProposalStatus::Rejected
    ));
}

#[test]
fn alternates_cannot_vote_until_promoted_into_members() {
    let mut state = setup_council_state();
    let pid = [0xE1; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone()],
        vec![BOB_ID.clone()],
    );

    {
        let mut block = state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
        let mut stx = block.transaction();
        let err = CastParliamentBallot {
            body: ParliamentBody::RulesCommittee,
            proposal_id: pid,
            decision: ParliamentDecision::Approve,
        }
        .execute(&BOB_ID, &mut stx)
        .expect_err("alternate must not vote before promotion");
        assert!(
            format!("{err:?}").contains("only seated parliament members"),
            "unexpected alternate-vote error: {err:?}"
        );
        assert!(
            stx.world.governance_stage_approvals().get(&rid).is_none(),
            "rejected alternate vote must not leave a stage record"
        );
    }

    set_body_roster_at_height(
        &mut state,
        4,
        0,
        ParliamentBody::RulesCommittee,
        vec![BOB_ID.clone()],
        Vec::new(),
    );
    approve_at_height(&mut state, 5, ParliamentBody::RulesCommittee, pid, &BOB_ID);

    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules approval record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert!(rules.approvers.contains(&*BOB_ID));
    assert!(!rules.approvers.contains(&*ALICE_ID));
}

#[test]
fn body_membership_is_not_shared_across_rosters() {
    let mut state = setup_council_state();
    let pid = [0xE2; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone()],
        vec![BOB_ID.clone()],
    );
    set_body_roster_at_height(
        &mut state,
        3,
        0,
        ParliamentBody::AgendaCouncil,
        vec![BOB_ID.clone()],
        vec![ALICE_ID.clone()],
    );

    {
        let mut block = state.block(BlockHeader::new(nonzero!(4_u64), None, None, None, 0, 0));
        let mut stx = block.transaction();
        let err = ApproveGovernanceProposal {
            body: ParliamentBody::RulesCommittee,
            proposal_id: pid,
        }
        .execute(&BOB_ID, &mut stx)
        .expect_err("agenda member must not vote in rules committee");
        assert!(
            format!("{err:?}").contains("only seated parliament members"),
            "unexpected rules cross-vote error: {err:?}"
        );
    }

    {
        let mut block = state.block(BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0));
        let mut stx = block.transaction();
        let err = ApproveGovernanceProposal {
            body: ParliamentBody::AgendaCouncil,
            proposal_id: pid,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("rules member must not vote in agenda council");
        assert!(
            format!("{err:?}").contains("only seated parliament members"),
            "unexpected agenda cross-vote error: {err:?}"
        );
    }
    assert!(
        state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&rid)
            .is_none(),
        "failed cross-body votes must not record approvals"
    );

    approve_at_height(
        &mut state,
        6,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
    );
    approve_at_height(&mut state, 7, ParliamentBody::AgendaCouncil, pid, &BOB_ID);
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "two correct body approvals are still short of the full parliament gate"
    );
}

#[test]
fn runtime_upgrade_remains_closed_until_fma_quorum() {
    let mut state = setup_council_state();
    let pid = [0xE3; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal_with_kind(
        &mut state,
        pid,
        &rid,
        sample_runtime_upgrade_proposal(),
    );

    for (idx, body) in deployment_stage_bodies().into_iter().enumerate() {
        approve_at_height(
            &mut state,
            2 + u64::try_from(idx).expect("idx fits"),
            body,
            pid,
            &ALICE_ID,
        );
    }
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "runtime upgrade must not open before the FMA committee gate"
    );

    approve_at_height(&mut state, 8, ParliamentBody::FmaCommittee, pid, &BOB_ID);
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("stage approvals");
    assert!(
        approvals.quorum_met(ParliamentBody::FmaCommittee, 0),
        "FMA quorum should be recorded for runtime upgrades"
    );
}

#[test]
fn abstain_neither_approves_nor_rejects_and_later_decision_replaces_it() {
    let mut state = setup_council_state();
    let pid = [0xE4; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);
    set_body_roster_at_height(
        &mut state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        vec![ALICE_ID.clone()],
        Vec::new(),
    );

    cast_ballot_at_height(
        &mut state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Abstain,
    );
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules abstention record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert!(rules.approvers.is_empty());
    assert!(rules.rejections.is_empty());
    assert!(rules.abstentions.contains(&*ALICE_ID));
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );

    cast_ballot_at_height(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("rules approval record");
    let rules = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage");
    assert!(rules.approvers.contains(&*ALICE_ID));
    assert!(
        rules.abstentions.is_empty(),
        "later signed decision must replace the earlier abstention"
    );
}

#[test]
fn deploy_fma_ballots_are_rejected_and_cannot_substitute_or_veto() {
    let mut state = setup_council_state();
    let pid = [0xE5; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    for (idx, body) in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
    ]
    .into_iter()
    .enumerate()
    {
        approve_at_height(
            &mut state,
            2 + u64::try_from(idx).expect("idx fits"),
            body,
            pid,
            &ALICE_ID,
        );
    }
    let err = expect_ballot_error_at_height(
        &state,
        7,
        ParliamentBody::FmaCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("not required for this proposal"),
        "unexpected FMA approval error: {err}"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "deploy proposals require oversight quorum; FMA approval is not a substitute"
    );

    let err = expect_ballot_error_at_height(
        &state,
        8,
        ParliamentBody::FmaCommittee,
        pid,
        &BOB_ID,
        ParliamentDecision::Reject,
    );
    assert!(
        err.contains("not required for this proposal"),
        "unexpected FMA rejection error: {err}"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "non-required FMA rejection must not veto a deploy proposal"
    );
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("stage approvals");
    assert!(
        approvals
            .stages
            .get(&ParliamentBody::FmaCommittee)
            .is_none(),
        "non-required FMA ballots must not create stage records"
    );

    approve_at_height(
        &mut state,
        9,
        ParliamentBody::OversightCommittee,
        pid,
        &BOB_ID,
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
}

#[test]
fn post_open_stage_ballots_are_rejected_and_cannot_reclose_referendum() {
    let mut state = setup_council_state();
    let pid = [0xE8; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    for (idx, body) in deployment_stage_bodies().into_iter().enumerate() {
        approve_at_height(
            &mut state,
            2 + u64::try_from(idx).expect("idx fits"),
            body,
            pid,
            &ALICE_ID,
        );
    }
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );

    let err = expect_ballot_error_at_height(
        &state,
        8,
        ParliamentBody::RulesCommittee,
        pid,
        &BOB_ID,
        ParliamentDecision::Reject,
    );
    assert!(
        err.contains("already left parliament stage"),
        "unexpected post-open ballot error: {err}"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open,
        "late parliament rejection must not close an already-open referendum"
    );
    assert!(!matches!(
        state
            .view()
            .world()
            .governance_proposals()
            .get(&pid)
            .expect("proposal")
            .status,
        iroha_core::state::GovernanceProposalStatus::Rejected
    ));
}

#[test]
fn stage_ballots_after_referendum_window_are_rejected_without_records() {
    let mut state = setup_council_state();
    state.gov.parliament_term_blocks = 100;
    let pid = [0xE9; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    let err = expect_ballot_error_at_height(
        &state,
        18,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("window elapsed"),
        "unexpected late-window ballot error: {err}"
    );
    assert!(
        state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&rid)
            .is_none(),
        "late-window ballot must not create stage records"
    );
}

#[test]
fn missing_proposal_or_referendum_rejects_ballot_without_records() {
    let missing_pid = [0xEA; 32];
    let missing_rid = hex::encode(missing_pid);
    let missing_state = setup_council_state();
    let err = expect_ballot_error_at_height(
        &missing_state,
        2,
        ParliamentBody::RulesCommittee,
        missing_pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("governance proposal not found"),
        "unexpected missing-proposal error: {err}"
    );
    assert!(
        missing_state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&missing_rid)
            .is_none(),
        "missing proposal must not create stage records"
    );

    let mut no_referendum_state = setup_council_state();
    let no_referendum_pid = [0xEB; 32];
    let no_referendum_rid = hex::encode(no_referendum_pid);
    seed_proposal_without_referendum(&mut no_referendum_state, no_referendum_pid);
    let err = expect_ballot_error_at_height(
        &no_referendum_state,
        2,
        ParliamentBody::RulesCommittee,
        no_referendum_pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("referendum not found"),
        "unexpected missing-referendum error: {err}"
    );
    assert!(
        no_referendum_state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&no_referendum_rid)
            .is_none(),
        "missing referendum must not create stage records"
    );
}

#[test]
fn epoch_rollover_rejects_ballots_without_fresh_council_roster() {
    let mut state = setup_council_state();
    state.gov.parliament_term_blocks = 2;
    let pid = [0xEE; 32];
    let rid = hex::encode(pid);
    seed_referendum_and_proposal(&mut state, pid, &rid);

    let err = expect_ballot_error_at_height(
        &state,
        3,
        ParliamentBody::RulesCommittee,
        pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("council roster missing for current epoch"),
        "unexpected epoch-rollover error: {err}"
    );
    assert!(
        state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&rid)
            .is_none(),
        "stale previous-epoch roster must not be reused for a new epoch ballot"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}

#[test]
fn malformed_body_rosters_reject_ballots_without_records() {
    let mut missing_state = setup_council_state();
    let missing_pid = [0xE6; 32];
    let missing_rid = hex::encode(missing_pid);
    seed_referendum_and_proposal(&mut missing_state, missing_pid, &missing_rid);
    remove_body_roster_at_height(&mut missing_state, 2, 0, ParliamentBody::AgendaCouncil);
    {
        let mut block =
            missing_state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
        let mut stx = block.transaction();
        let err = CastParliamentBallot {
            body: ParliamentBody::AgendaCouncil,
            proposal_id: missing_pid,
            decision: ParliamentDecision::Approve,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("missing body roster must reject ballots");
        assert!(
            format!("{err:?}").contains("parliament roster missing"),
            "unexpected missing-roster error: {err:?}"
        );
        assert!(
            stx.world
                .governance_stage_approvals()
                .get(&missing_rid)
                .is_none(),
            "missing roster rejection must not record approvals"
        );
    }

    let mut empty_state = setup_council_state();
    let empty_pid = [0xE7; 32];
    let empty_rid = hex::encode(empty_pid);
    seed_referendum_and_proposal(&mut empty_state, empty_pid, &empty_rid);
    set_body_roster_at_height(
        &mut empty_state,
        2,
        0,
        ParliamentBody::AgendaCouncil,
        Vec::new(),
        vec![ALICE_ID.clone()],
    );
    let mut block = empty_state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = CastParliamentBallot {
        body: ParliamentBody::AgendaCouncil,
        proposal_id: empty_pid,
        decision: ParliamentDecision::Approve,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("empty body roster must reject ballots");
    assert!(
        format!("{err:?}").contains("parliament roster empty"),
        "unexpected empty-roster error: {err:?}"
    );
    assert!(
        stx.world
            .governance_stage_approvals()
            .get(&empty_rid)
            .is_none(),
        "empty roster rejection must not record approvals"
    );
}

#[test]
fn spoofed_roster_metadata_rejects_ballots_without_records() {
    let mut body_mismatch_state = setup_council_state();
    let body_mismatch_pid = [0xEC; 32];
    let body_mismatch_rid = hex::encode(body_mismatch_pid);
    seed_referendum_and_proposal(
        &mut body_mismatch_state,
        body_mismatch_pid,
        &body_mismatch_rid,
    );
    mutate_body_roster_at_height(
        &mut body_mismatch_state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        |roster| roster.body = ParliamentBody::AgendaCouncil,
    );
    let err = expect_ballot_error_at_height(
        &body_mismatch_state,
        3,
        ParliamentBody::RulesCommittee,
        body_mismatch_pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("roster body mismatch"),
        "unexpected body-mismatch error: {err}"
    );
    assert!(
        body_mismatch_state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&body_mismatch_rid)
            .is_none(),
        "spoofed body metadata must not create stage records"
    );

    let mut epoch_mismatch_state = setup_council_state();
    let epoch_mismatch_pid = [0xED; 32];
    let epoch_mismatch_rid = hex::encode(epoch_mismatch_pid);
    seed_referendum_and_proposal(
        &mut epoch_mismatch_state,
        epoch_mismatch_pid,
        &epoch_mismatch_rid,
    );
    mutate_body_roster_at_height(
        &mut epoch_mismatch_state,
        2,
        0,
        ParliamentBody::RulesCommittee,
        |roster| roster.epoch = 99,
    );
    let err = expect_ballot_error_at_height(
        &epoch_mismatch_state,
        3,
        ParliamentBody::RulesCommittee,
        epoch_mismatch_pid,
        &ALICE_ID,
        ParliamentDecision::Approve,
    );
    assert!(
        err.contains("body roster epoch mismatch"),
        "unexpected roster-epoch error: {err}"
    );
    assert!(
        epoch_mismatch_state
            .view()
            .world()
            .governance_stage_approvals()
            .get(&epoch_mismatch_rid)
            .is_none(),
        "stale embedded epoch must not create stage records"
    );
}

#[test]
fn parliament_snapshot_allows_approvals_without_council_state() {
    let mut state = setup_council_state();
    enable_parliament_module(&mut state);
    let pid = [0xBC; 32];
    let rid = hex::encode(pid);
    let bodies = seed_snapshot_proposal(&mut state, pid, &rid, 1, None);

    for (idx, body) in deployment_stage_bodies().into_iter().enumerate() {
        let signer = bodies
            .rosters
            .get(&body)
            .and_then(|roster| roster.members.first())
            .cloned()
            .expect("parliament body signer");
        approve_at_height(
            &mut state,
            2 + u64::try_from(idx).expect("idx fits"),
            body,
            pid,
            &signer,
        );
    }

    assert!(
        state.view().world().council().get(&0).is_none(),
        "council state should not be required for parliament snapshot approvals",
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Open
    );
}

#[test]
fn tampered_parliament_snapshot_commitment_rejects_ballots() {
    let mut state = setup_council_state();
    enable_parliament_module(&mut state);
    let pid = [0xBD; 32];
    let rid = hex::encode(pid);
    let bodies = seed_snapshot_proposal(&mut state, pid, &rid, 2, Some([0xFF; 32]));
    let signer = bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("rules signer");

    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = CastParliamentBallot {
        body: ParliamentBody::RulesCommittee,
        proposal_id: pid,
        decision: ParliamentDecision::Approve,
    }
    .execute(&signer, &mut stx)
    .expect_err("tampered snapshot root must reject ballots");
    assert!(
        format!("{err:?}").contains("snapshot commitment mismatch"),
        "unexpected snapshot commitment error: {err:?}"
    );
    assert!(
        stx.world.governance_stage_approvals().get(&rid).is_none(),
        "tampered snapshot must not leave approval records"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}

#[test]
fn mismatched_snapshot_epoch_rejects_ballots_before_roster_use() {
    let mut state = setup_council_state();
    enable_parliament_module(&mut state);
    let pid = [0xBE; 32];
    let rid = hex::encode(pid);
    let bodies = seed_snapshot_proposal(&mut state, pid, &rid, 3, None);
    mutate_snapshot_at_height(&mut state, 2, pid, |snapshot| {
        snapshot.bodies.selection_epoch = snapshot.selection_epoch.saturating_add(1);
    });
    let signer = bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("rules signer");

    let mut block = state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = CastParliamentBallot {
        body: ParliamentBody::RulesCommittee,
        proposal_id: pid,
        decision: ParliamentDecision::Approve,
    }
    .execute(&signer, &mut stx)
    .expect_err("snapshot epoch mismatch must reject ballots");
    assert!(
        format!("{err:?}").contains("snapshot epoch mismatch"),
        "unexpected snapshot epoch error: {err:?}"
    );
    assert!(
        stx.world.governance_stage_approvals().get(&rid).is_none(),
        "mismatched snapshot must not leave approval records"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}

#[test]
fn snapshot_roster_metadata_mismatch_rejects_ballots_even_with_matching_root() {
    let mut state = setup_council_state();
    enable_parliament_module(&mut state);
    let pid = [0xBF; 32];
    let rid = hex::encode(pid);
    let bodies = seed_snapshot_proposal(&mut state, pid, &rid, 4, None);
    mutate_snapshot_at_height(&mut state, 2, pid, |snapshot| {
        let roster = snapshot
            .bodies
            .rosters
            .get_mut(&ParliamentBody::RulesCommittee)
            .expect("rules roster present");
        roster.body = ParliamentBody::AgendaCouncil;
        snapshot.roster_root = roster_root(&snapshot.bodies);
    });
    let signer = bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("rules signer");

    let mut block = state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
    let mut stx = block.transaction();
    let err = CastParliamentBallot {
        body: ParliamentBody::RulesCommittee,
        proposal_id: pid,
        decision: ParliamentDecision::Approve,
    }
    .execute(&signer, &mut stx)
    .expect_err("snapshot roster body mismatch must reject ballots");
    assert!(
        format!("{err:?}").contains("roster body mismatch"),
        "unexpected snapshot roster-mismatch error: {err:?}"
    );
    assert!(
        stx.world.governance_stage_approvals().get(&rid).is_none(),
        "spoofed snapshot roster must not leave approval records"
    );
    assert_eq!(
        referendum_status(&state, &rid),
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}
