//! Governance threshold tests: ratio and turnout logic.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use core::num::NonZeroU64;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        GovernancePipeline, GovernanceProposalRecord, GovernanceProposalStatus,
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        GovernanceStageApprovals, State, StateTransaction, World, WorldReadOnly,
    },
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
        ProposalKind,
    },
    isi::{
        error::{InstructionExecutionError, MathError},
        governance::FinalizeReferendum,
    },
    nexus::DataSpaceId,
    smart_contract::ContractAddress,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
const DEPLOY_PARLIAMENT_BODIES: [ParliamentBody; 6] = [
    ParliamentBody::RulesCommittee,
    ParliamentBody::AgendaCouncil,
    ParliamentBody::InterestPanel,
    ParliamentBody::ReviewPanel,
    ParliamentBody::PolicyJury,
    ParliamentBody::OversightCommittee,
];
fn checked_random_governance_threshold_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked governance threshold keypair")
}
fn threshold_contract_address(nonce: u64) -> ContractAddress {
    ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .expect("threshold proposal contract address")
}
fn seed_open_plain_referendum(
    stx: &mut StateTransaction<'_, '_>,
    proposal_id: [u8; 32],
    h_start: u64,
    h_end: u64,
) -> String {
    let referendum_id = hex::encode(proposal_id);
    let referendum = GovernanceReferendumRecord {
        h_start,
        h_end,
        status: GovernanceReferendumStatus::Open,
        mode: GovernanceReferendumMode::Plain,
    };
    let pipeline = GovernancePipeline::seeded(h_start, Some(&referendum), &stx.gov);
    stx.world
        .governance_referenda_mut()
        .insert(referendum_id.clone(), referendum);
    stx.world.governance_proposals_mut().insert(
        proposal_id,
        GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                contract_address: threshold_contract_address(u64::from(proposal_id[0])),
                code_hash_hex: ContractCodeHash::from_hex_str(&hex::encode(proposal_id))
                    .expect("code hash"),
                abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode([0x11; 32]))
                    .expect("ABI hash"),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: h_start,
            status: GovernanceProposalStatus::Proposed,
            pipeline,
            parliament_snapshot: None,
            finalization_evidence: None,
            enacted_at_height: None,
        },
    );
    let mut approvals = GovernanceStageApprovals::default();
    for body in DEPLOY_PARLIAMENT_BODIES {
        approvals
            .ensure_stage(body, 0, 1, stx.gov.parliament_quorum_bps)
            .record(ALICE_ID.clone());
    }
    assert!(
        DEPLOY_PARLIAMENT_BODIES
            .into_iter()
            .all(|body| approvals.quorum_met(body, 0))
    );
    stx.world
        .governance_stage_approvals_mut()
        .insert(referendum_id.clone(), approvals);
    referendum_id
}
#[test]
fn governance_threshold_fixture_uses_checked_randomness() {
    let _key_pair = checked_random_governance_threshold_keypair();
}
#[test]
fn ratio_threshold_rejects_even_if_approve_gt_reject() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Configure strict ratio: 3/4 (75%) so 2/(2+1)=0.66.. fails
    let mut cfg = state.gov.clone();
    cfg.approval_threshold_q_num = 3;
    cfg.approval_threshold_q_den = 4;
    state.set_gov(cfg);
    // Finalization occurs after the inclusive [1, 1] voting window.
    let (_pk, _sk) = checked_random_governance_threshold_keypair().into_parts();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let proposal_id = [0xAB; 32];
    let rid = seed_open_plain_referendum(&mut stx, proposal_id, 1, 1);
    // Locks: approve weight=2 (sqrt(4)), reject weight=1 (sqrt(1)); duration 0 → factor 1
    let mut map = iroha_core::state::GovernanceLocksForReferendum::default();
    map.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 4_u64.into(),
            slashed: 0_u64.into(),
            expiry_height: 100,
            direction: 0,
            duration_blocks: 0,
            custody: None,
        },
    );
    map.locks.insert(
        BOB_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: BOB_ID.clone(),
            amount: 1_u64.into(),
            slashed: 0_u64.into(),
            expiry_height: 100,
            direction: 1,
            duration_blocks: 0,
            custody: None,
        },
    );
    stx.world.governance_locks_mut().insert(rid.clone(), map);
    // Finalize should reject due to ratio < 75%
    let instr = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id,
    };
    instr.execute(&ALICE_ID, &mut stx).expect("finalize ok");
    let evs = stx.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalRejected(_)))
    )));
    let proposal = stx
        .world
        .governance_proposals()
        .get(&proposal_id)
        .expect("proposal remains recorded");
    assert_eq!(proposal.status, GovernanceProposalStatus::Rejected);
    let evidence = proposal
        .finalization_evidence
        .expect("rejection retains finalization evidence");
    assert_eq!(evidence.proposal_id, proposal_id);
    assert_eq!(evidence.referendum_id, proposal_id);
    assert_eq!(evidence.finalized_at_height, 1);
    assert_eq!(
        (evidence.approve, evidence.reject, evidence.abstain),
        (2, 1, 0)
    );
    assert!(!evidence.approved);
    assert_eq!(
        stx.world
            .governance_referenda()
            .get(&rid)
            .expect("referendum remains recorded")
            .status,
        GovernanceReferendumStatus::Closed
    );
}
#[test]
fn min_turnout_rejects_when_below_threshold() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Set high min_turnout to force rejection
    let mut cfg = state.gov.clone();
    cfg.min_turnout = 1_000;
    state.set_gov(cfg);
    // Finalization occurs after the inclusive [1, 1] voting window.
    let (_pk, _sk) = checked_random_governance_threshold_keypair().into_parts();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let proposal_id = [0xCD; 32];
    let rid = seed_open_plain_referendum(&mut stx, proposal_id, 1, 1);
    // Minimal approve weight=1
    let mut map = iroha_core::state::GovernanceLocksForReferendum::default();
    map.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 1_u64.into(),
            slashed: 0_u64.into(),
            expiry_height: 100,
            direction: 0,
            duration_blocks: 0,
            custody: None,
        },
    );
    stx.world.governance_locks_mut().insert(rid.clone(), map);
    // Finalize should reject due to turnout < min_turnout
    let instr = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id,
    };
    instr.execute(&ALICE_ID, &mut stx).expect("finalize ok");
    let evs = stx.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalRejected(_)))
    )));
    let proposal = stx
        .world
        .governance_proposals()
        .get(&proposal_id)
        .expect("proposal remains recorded");
    assert_eq!(proposal.status, GovernanceProposalStatus::Rejected);
    let evidence = proposal
        .finalization_evidence
        .expect("rejection retains finalization evidence");
    assert_eq!(evidence.finalized_at_height, 1);
    assert_eq!(
        (evidence.approve, evidence.reject, evidence.abstain),
        (1, 0, 0)
    );
    assert_eq!(evidence.min_turnout, 1_000);
    assert!(!evidence.approved);
    assert_eq!(
        stx.world
            .governance_referenda()
            .get(&rid)
            .expect("referendum remains recorded")
            .status,
        GovernanceReferendumStatus::Closed
    );
}
#[test]
fn finalize_referendum_rejects_tally_overflow_without_side_effects() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    let mut cfg = state.gov.clone();
    cfg.conviction_step_blocks = 1;
    cfg.max_conviction = u64::MAX;
    state.set_gov(cfg);
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let proposal_id = [0xEF; 32];
    let rid = seed_open_plain_referendum(&mut stx, proposal_id, 1, 1);
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    for owner in [ALICE_ID.clone(), BOB_ID.clone()] {
        locks.locks.insert(
            owner.clone(),
            iroha_core::state::GovernanceLockRecord {
                owner,
                amount: u128::MAX.into(),
                slashed: 0_u64.into(),
                expiry_height: u64::MAX,
                direction: 0,
                duration_blocks: u64::MAX - 1,
                custody: None,
            },
        );
    }
    stx.world.governance_locks_mut().insert(rid.clone(), locks);
    let err = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("overflowing tally must fail");
    assert!(
        matches!(&err, InstructionExecutionError::Math(MathError::Overflow)),
        "unexpected finalization error: {err}"
    );
    assert!(stx.world.take_external_events().is_empty());
    let stored = stx
        .world
        .governance_locks()
        .get(&rid)
        .expect("locks remain present");
    assert_eq!(stored.locks.len(), 2);
    assert!(
        stored
            .locks
            .values()
            .all(|record| record.amount.scale() == 0
                && record.amount.as_numeric().try_mantissa_u128() == Some(u128::MAX))
    );
    let proposal = stx
        .world
        .governance_proposals()
        .get(&proposal_id)
        .expect("proposal remains present");
    assert_eq!(proposal.status, GovernanceProposalStatus::Proposed);
    assert!(proposal.finalization_evidence.is_none());
    assert_eq!(
        stx.world
            .governance_referenda()
            .get(&rid)
            .expect("referendum remains present")
            .status,
        GovernanceReferendumStatus::Open
    );
}
