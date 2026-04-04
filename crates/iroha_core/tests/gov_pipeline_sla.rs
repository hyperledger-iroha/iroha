//! Governance pipeline SLA enforcement tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GovernancePipeline, GovernanceProposalRecord, GovernanceProposalStatus,
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        GovernanceStage, GovernanceStageApprovals, GovernanceStageFailure, State, World,
        WorldReadOnly,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
        ProposalKind,
    },
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn deploy_payload(code_hex: &str, abi_hex: &str) -> DeployContractProposal {
    DeployContractProposal {
        contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
            .parse()
            .expect("contract address"),
        code_hash_hex: ContractCodeHash::from_hex_str(code_hex).expect("code hash"),
        abi_hash_hex: ContractAbiHash::from_hex_str(abi_hex).expect("abi hash"),
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    }
}

#[test]
fn pipeline_rejects_when_referendum_missing_at_study_deadline() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    state.gov.min_enactment_delay = 0;
    state.gov.pipeline_study_sla_blocks = 1;
    state.gov.pipeline_review_sla_blocks = 10;
    state.gov.pipeline_decision_sla_blocks = 10;
    state.gov.pipeline_enactment_sla_blocks = 10;

    let mut block1 = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut tx1 = block1.transaction();
    let pid = [0xAA; 32];
    let rid = hex::encode(pid);
    let abi_hex = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    tx1.world.governance_proposals_mut().insert(
        pid,
        GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(deploy_payload(&"11".repeat(32), &abi_hex)),
            created_height: 1,
            status: GovernanceProposalStatus::Proposed,
            pipeline: GovernancePipeline::seeded(1, None, &tx1.gov),
            parliament_snapshot: None,
        },
    );
    let mut approvals = GovernanceStageApprovals::default();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
    ] {
        let approval_stage = approvals.ensure_stage(body, 0, 1, tx1.gov.parliament_quorum_bps);
        approval_stage.record(ALICE_ID.clone());
    }
    tx1.world
        .governance_stage_approvals_mut()
        .insert(rid, approvals);
    tx1.apply();
    block1.commit().expect("commit first block");

    let block2 = state.block(BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0));
    let rec = block2
        .world
        .governance_proposals()
        .get(&pid)
        .cloned()
        .expect("proposal present");
    assert!(matches!(rec.status, GovernanceProposalStatus::Rejected));
    let study_stage = rec
        .pipeline
        .stages
        .iter()
        .find(|stage| matches!(stage.stage, GovernanceStage::Study))
        .expect("study stage present");
    assert!(
        matches!(
            study_stage.failure,
            Some(
                GovernanceStageFailure::MissingReferendum | GovernanceStageFailure::DeadlineMissed
            )
        ),
        "study stage failure must be recorded"
    );
}

#[test]
fn pipeline_marks_enactment_overdue() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    state.gov.min_enactment_delay = 0;
    state.gov.pipeline_rules_sla_blocks = 1;
    state.gov.pipeline_agenda_sla_blocks = 1;
    state.gov.pipeline_study_sla_blocks = 1;
    state.gov.pipeline_review_sla_blocks = 1;
    state.gov.pipeline_decision_sla_blocks = 1;
    state.gov.pipeline_enactment_sla_blocks = 1;

    let mut block1 = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut tx1 = block1.transaction();
    let pid = [0xBB; 32];
    let rid = hex::encode(pid);
    let abi_hex = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    tx1.world.governance_referenda_mut().insert(
        rid.clone(),
        GovernanceReferendumRecord {
            h_start: 2,
            h_end: 2,
            status: GovernanceReferendumStatus::Closed,
            mode: GovernanceReferendumMode::Zk,
        },
    );
    let referendum_snapshot = tx1.world.governance_referenda().get(&rid).copied();
    tx1.world.governance_proposals_mut().insert(
        pid,
        GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(deploy_payload(&"22".repeat(32), &abi_hex)),
            created_height: 1,
            status: GovernanceProposalStatus::Approved,
            pipeline: GovernancePipeline::seeded(1, referendum_snapshot.as_ref(), &tx1.gov),
            parliament_snapshot: None,
        },
    );
    let mut approvals = GovernanceStageApprovals::default();
    for (body, epoch) in [
        (ParliamentBody::RulesCommittee, 0),
        (ParliamentBody::AgendaCouncil, 0),
        (ParliamentBody::InterestPanel, 0),
        (ParliamentBody::ReviewPanel, 0),
        (ParliamentBody::PolicyJury, 0),
    ] {
        let approval_stage = approvals.ensure_stage(body, epoch, 1, tx1.gov.parliament_quorum_bps);
        approval_stage.record(ALICE_ID.clone());
    }
    tx1.world
        .governance_stage_approvals_mut()
        .insert(rid.clone(), approvals);
    tx1.apply();
    block1.commit().expect("commit first block");

    let block2 = state.block(BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0));
    let rec = block2
        .world
        .governance_proposals()
        .get(&pid)
        .cloned()
        .expect("proposal present");
    assert!(matches!(rec.status, GovernanceProposalStatus::Rejected));
    let enact_stage = rec
        .pipeline
        .stages
        .iter()
        .find(|stage| matches!(stage.stage, GovernanceStage::Enact))
        .expect("enact stage present");
    assert!(
        matches!(
            enact_stage.failure,
            Some(GovernanceStageFailure::EnactmentOverdue)
        ),
        "enactment stage should record overdue failure"
    );
}

#[test]
fn pipeline_rejects_when_rules_quorum_missing() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    state.gov.min_enactment_delay = 0;
    state.gov.pipeline_rules_sla_blocks = 1;
    state.gov.pipeline_agenda_sla_blocks = 5;
    state.gov.pipeline_study_sla_blocks = 10;
    state.gov.pipeline_review_sla_blocks = 10;
    state.gov.pipeline_decision_sla_blocks = 10;
    state.gov.pipeline_enactment_sla_blocks = 10;

    let mut block1 = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut tx1 = block1.transaction();
    let pid = [0xCC; 32];
    let abi_hex = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    tx1.world.governance_proposals_mut().insert(
        pid,
        GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(deploy_payload(&"44".repeat(32), &abi_hex)),
            created_height: 1,
            status: GovernanceProposalStatus::Proposed,
            pipeline: GovernancePipeline::seeded(1, None, &tx1.gov),
            parliament_snapshot: None,
        },
    );
    tx1.apply();
    block1.commit().expect("commit first block");

    let block2 = state.block(BlockHeader::new(nonzero!(4_u64), None, None, None, 0, 0));
    let rec = block2
        .world
        .governance_proposals()
        .get(&pid)
        .cloned()
        .expect("proposal present");
    assert!(matches!(rec.status, GovernanceProposalStatus::Rejected));
    let rules_stage = rec
        .pipeline
        .stages
        .iter()
        .find(|stage| matches!(stage.stage, GovernanceStage::Rules))
        .expect("rules stage present");
    assert!(
        matches!(
            rules_stage.failure,
            Some(GovernanceStageFailure::DeadlineMissed)
        ),
        "rules stage should record a deadline failure"
    );
}
