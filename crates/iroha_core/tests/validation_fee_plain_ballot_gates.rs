//! Proposal-bound PLAIN ballot gates for validation-fee governance.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[path = "common/governance.rs"]
mod governance_fixture;
use core::num::NonZeroU64;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        CitizenshipRecord, GovernancePipeline, GovernanceProposalRecord, GovernanceProposalStatus,
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        GovernanceStageApprovals, State, StateTransaction, World, WorldReadOnly,
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::{Domain, DomainId},
    governance::types::{ParliamentBody, ProposalKind, ValidationFeePolicyProposal},
    isi::{
        Grant,
        governance::{CastPlainBallot, SlashGovernanceLock},
    },
    permission::Permission,
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, ValidationFeeChargingMode,
        ValidationFeePlainElectorateEligibilityRuleV1, ValidationFeePlainElectorateMemberV1,
        ValidationFeePlainElectorateRulesV1, ValidationFeePlainElectorateSnapshotV1,
        ValidationFeePolicyV1,
    },
};
use iroha_executor_data_model::permission::governance::CanSlashGovernanceLock;
use iroha_primitives::numeric::{NumericSpec, Quantity};
use mv::storage::StorageReadOnly;
const GATE_HEIGHT: u64 = 5;
const BALLOT_HEIGHT: u64 = 10;
const BALLOT_AMOUNT: u64 = 150;
const BALLOT_DURATION: u64 = 3_600;
const CITIZENSHIP_AMOUNT: u64 = 10_000;
fn account(seed: u8) -> AccountId {
    let key_pair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic key pair");
    AccountId::new(key_pair.public_key().clone())
}
fn plain_rules(
    voting_asset_id: AssetDefinitionId,
    bond_escrow_account: AccountId,
    slash_receiver_account: AccountId,
) -> ValidationFeePlainElectorateRulesV1 {
    ValidationFeePlainElectorateRulesV1 {
        voting_asset_id,
        bond_escrow_account,
        slash_receiver_account,
        ballot_amount: BALLOT_AMOUNT.into(),
        ballot_duration_blocks: BALLOT_DURATION,
        citizenship_amount: CITIZENSHIP_AMOUNT.into(),
        max_members: VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
        conviction_step_blocks: 2,
        max_conviction: 3,
        min_turnout: 1,
        approval_threshold_numerator: 1,
        approval_threshold_denominator: 2,
        eligibility_rule:
            ValidationFeePlainElectorateEligibilityRuleV1::ProposalOperatorAtOrBeforeGateOthersAfterGate,
    }
}
fn ballot(
    referendum_id: &str,
    owner: &AccountId,
    amount: u64,
    duration_blocks: u64,
    direction: u8,
) -> CastPlainBallot {
    CastPlainBallot {
        referendum_id: referendum_id.to_owned(),
        owner: owner.clone(),
        amount: amount.into(),
        duration_blocks,
        direction,
    }
}
fn rejection_message(
    ballot: CastPlainBallot,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> String {
    ballot
        .execute(authority, state_transaction)
        .expect_err("ballot must be rejected")
        .to_string()
}
#[test]
fn validation_fee_plain_ballots_use_the_retained_proposal_contract() {
    let proposer = account(1);
    let other_at_gate = account(2);
    let late_nay_voter = account(3);
    let late_abstain_voter = account(4);
    let bond_escrow = account(5);
    let parliament_signer = account(6);
    let domain_id = DomainId::try_new("validation_fee", "universal").expect("domain");
    let voting_asset_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("asset name"),
    );
    let changed_live_voting_asset_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "changed_live_xor".parse().expect("asset name"),
    );
    let accounts = [
        proposer.clone(),
        other_at_gate.clone(),
        late_nay_voter.clone(),
        late_abstain_voter.clone(),
        bond_escrow.clone(),
        parliament_signer.clone(),
    ];
    let domain = Domain::new(domain_id).build(&proposer);
    let asset_definition = AssetDefinition::new(
        voting_asset_id.clone(),
        "xor".to_owned(),
        NumericSpec::fractional(u32::from(VALIDATION_FEE_DS_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&proposer);
    let assets = accounts.iter().cloned().map(|owner| {
        let amount: u64 = if owner == bond_escrow { 0 } else { 500 };
        Asset::new(
            AssetId::new(voting_asset_id.clone(), owner),
            Quantity::from(amount),
        )
    });
    let world = World::with_assets(
        [domain],
        accounts
            .iter()
            .cloned()
            .map(|id| Account::new(id).build(&proposer))
            .collect::<Vec<_>>(),
        [asset_definition],
        assets.collect::<Vec<_>>(),
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let retained_rules = plain_rules(
        voting_asset_id.clone(),
        bond_escrow.clone(),
        parliament_signer.clone(),
    );
    let mut proposal_time_governance = state.gov.clone();
    proposal_time_governance.voting_asset_id = voting_asset_id.clone();
    proposal_time_governance.citizenship_asset_id = voting_asset_id.clone();
    proposal_time_governance.bond_escrow_account = bond_escrow.clone();
    proposal_time_governance.slash_receiver_account = parliament_signer.clone();
    proposal_time_governance.citizenship_escrow_account = bond_escrow.clone();
    proposal_time_governance.citizenship_bond_amount = retained_rules.citizenship_amount.clone();
    proposal_time_governance.min_bond_amount = retained_rules.ballot_amount.clone();
    proposal_time_governance.window_span = retained_rules.ballot_duration_blocks;
    proposal_time_governance.conviction_step_blocks = retained_rules.conviction_step_blocks;
    proposal_time_governance.max_conviction = retained_rules.max_conviction;
    proposal_time_governance.min_turnout = retained_rules.min_turnout;
    proposal_time_governance.approval_threshold_q_num = retained_rules.approval_threshold_numerator;
    proposal_time_governance.approval_threshold_q_den =
        retained_rules.approval_threshold_denominator;
    proposal_time_governance.plain_voting_enabled = true;
    proposal_time_governance.parliament_term_blocks = 100;
    state.set_gov(proposal_time_governance);
    let policy = ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: *state.network_id_ref(),
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: voting_asset_id.clone(),
        ds_scale: VALIDATION_FEE_DS_SCALE,
        fee: Quantity::zero(),
        treasury_account_id: bond_escrow.clone(),
        charging_mode: ValidationFeeChargingMode::Disabled,
        effective_from_height: BALLOT_HEIGHT + BALLOT_DURATION,
        expires_after_height: None,
        exemption_classes: Vec::new(),
        treasury_payout_binding: None,
    };
    assert_eq!(policy.policy_invariant_error(), None);
    let proposal_kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        policy,
        payout_lifecycle_proposal_id: None,
        plain_electorate_rules: retained_rules.clone(),
    });
    let proposal_id = proposal_kind.fingerprint();
    let referendum_id = hex::encode(proposal_id);
    // Proposal-bound ballot validation must not be reinterpreted through mutable
    // live governance state after the proposal has been fingerprinted.
    let mut changed_live_governance = state.gov.clone();
    changed_live_governance.plain_voting_enabled = false;
    changed_live_governance.citizenship_bond_amount = 20_000_u64.into();
    changed_live_governance.min_bond_amount = (BALLOT_AMOUNT + 1).into();
    changed_live_governance.voting_asset_id = changed_live_voting_asset_id;
    changed_live_governance.bond_escrow_account = parliament_signer.clone();
    changed_live_governance.slash_receiver_account = other_at_gate.clone();
    changed_live_governance.window_span = 99;
    changed_live_governance.conviction_step_blocks = 99;
    changed_live_governance.max_conviction = 1;
    changed_live_governance.min_turnout = 999;
    changed_live_governance.approval_threshold_q_num = 3;
    changed_live_governance.approval_threshold_q_den = 4;
    state.set_gov(changed_live_governance);
    let header = BlockHeader::new(
        NonZeroU64::new(BALLOT_HEIGHT).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();
    state_transaction.world.governance_proposals_mut().insert(
        proposal_id,
        GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind: proposal_kind,
            created_height: GATE_HEIGHT,
            status: GovernanceProposalStatus::Proposed,
            pipeline: GovernancePipeline::default(),
            parliament_snapshot: governance_fixture::single_member_parliament_snapshot(
                &parliament_signer,
                GATE_HEIGHT,
            ),
            finalization_evidence: None,
            enacted_at_height: None,
        },
    );
    state_transaction.world.governance_referenda_mut().insert(
        referendum_id.clone(),
        GovernanceReferendumRecord {
            h_start: BALLOT_HEIGHT,
            h_end: BALLOT_HEIGHT + BALLOT_DURATION - 1,
            status: GovernanceReferendumStatus::Proposed,
            mode: GovernanceReferendumMode::Plain,
        },
    );
    for (owner, bonded_height) in [
        (&proposer, GATE_HEIGHT),
        (&other_at_gate, GATE_HEIGHT),
        (&late_nay_voter, GATE_HEIGHT + 1),
        (&late_abstain_voter, GATE_HEIGHT + 1),
    ] {
        state_transaction.world.citizens_mut().insert(
            owner.clone(),
            CitizenshipRecord::new(
                owner.clone(),
                retained_rules.citizenship_amount.clone(),
                bonded_height,
            ),
        );
    }
    let mut electorate_members = [
        (&proposer, GATE_HEIGHT),
        (&late_nay_voter, GATE_HEIGHT + 1),
        (&late_abstain_voter, GATE_HEIGHT + 1),
    ]
    .into_iter()
    .map(
        |(account_id, bonded_height)| ValidationFeePlainElectorateMemberV1 {
            account_id: account_id.clone(),
            bonded_height,
            bonded_amount: retained_rules.citizenship_amount.clone(),
        },
    )
    .collect::<Vec<_>>();
    electorate_members.sort_by(|left, right| left.account_id.cmp(&right.account_id));
    let electorate = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
        proposal_id,
        proposer.clone(),
        BALLOT_HEIGHT,
        GATE_HEIGHT,
        electorate_members,
    )
    .expect("canonical proposal-bound PLAIN electorate snapshot");
    assert_eq!(
        electorate.context_error(proposal_id, &proposer, &retained_rules),
        None
    );
    assert!(!electorate.contains(&other_at_gate));
    let electorate_root = electorate.roster_root;
    let mut approvals = GovernanceStageApprovals::default();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        approvals
            .ensure_stage(body, 0, 1, 10_000)
            .record(parliament_signer.clone());
    }
    approvals.approval_gate_height = Some(GATE_HEIGHT);
    approvals.validation_fee_plain_electorate_snapshot = Some(electorate);
    state_transaction
        .world
        .governance_stage_approvals_mut()
        .insert(referendum_id.clone(), approvals);
    for (candidate, expected) in [
        (
            ballot(
                &referendum_id,
                &late_nay_voter,
                BALLOT_AMOUNT - 1,
                BALLOT_DURATION,
                0,
            ),
            "amount or duration differs",
        ),
        (
            ballot(
                &referendum_id,
                &late_nay_voter,
                BALLOT_AMOUNT + 1,
                BALLOT_DURATION,
                0,
            ),
            "amount or duration differs",
        ),
        (
            ballot(
                &referendum_id,
                &late_nay_voter,
                BALLOT_AMOUNT,
                BALLOT_DURATION - 1,
                0,
            ),
            "amount or duration differs",
        ),
        (
            ballot(
                &referendum_id,
                &late_nay_voter,
                BALLOT_AMOUNT,
                BALLOT_DURATION + 1,
                0,
            ),
            "amount or duration differs",
        ),
        (
            ballot(
                &referendum_id,
                &late_nay_voter,
                BALLOT_AMOUNT,
                BALLOT_DURATION,
                3,
            ),
            "direction must be Aye, Nay, or Abstain",
        ),
    ] {
        let error = rejection_message(candidate, &late_nay_voter, &mut state_transaction);
        assert!(error.contains(expected), "unexpected rejection: {error}");
    }
    // Mutable citizen changes after the boundary cannot expand or shrink the
    // proposal-bound electorate.
    state_transaction.world.citizens_mut().insert(
        other_at_gate.clone(),
        CitizenshipRecord::new(
            other_at_gate.clone(),
            retained_rules.citizenship_amount.clone(),
            GATE_HEIGHT + 1,
        ),
    );
    let error = rejection_message(
        ballot(
            &referendum_id,
            &other_at_gate,
            BALLOT_AMOUNT,
            BALLOT_DURATION,
            0,
        ),
        &other_at_gate,
        &mut state_transaction,
    );
    assert!(
        error.contains("not in the frozen PLAIN electorate"),
        "an ordinary citizen excluded at the gate must remain ineligible: {error}"
    );
    state_transaction.world.citizens_mut().insert(
        proposer.clone(),
        CitizenshipRecord::new(
            proposer.clone(),
            retained_rules.citizenship_amount.clone(),
            GATE_HEIGHT + 1,
        ),
    );
    assert!(
        state_transaction
            .world
            .citizens_mut()
            .remove(late_abstain_voter.clone())
            .is_some(),
        "late abstain voter must exist in the mutable citizen roster"
    );
    let retained_electorate = state_transaction
        .world
        .governance_stage_approvals()
        .get(&referendum_id)
        .and_then(|retained| retained.validation_fee_plain_electorate_snapshot.as_ref())
        .expect("retained frozen PLAIN electorate");
    assert_eq!(retained_electorate.roster_root, electorate_root);
    assert!(retained_electorate.contains(&proposer));
    assert!(retained_electorate.contains(&late_abstain_voter));
    assert!(!retained_electorate.contains(&other_at_gate));
    for (owner, direction) in [
        (&proposer, 0),
        (&late_nay_voter, 1),
        (&late_abstain_voter, 2),
    ] {
        ballot(
            &referendum_id,
            owner,
            BALLOT_AMOUNT,
            BALLOT_DURATION,
            direction,
        )
        .execute(owner, &mut state_transaction)
        .expect("proposal-bound exact ballot must be accepted");
    }
    let error = rejection_message(
        ballot(&referendum_id, &proposer, BALLOT_AMOUNT, BALLOT_DURATION, 0),
        &proposer,
        &mut state_transaction,
    );
    assert!(
        error.contains("one effective ballot per account"),
        "validation-fee re-voting must be rejected: {error}"
    );
    let locks = state_transaction
        .world
        .governance_locks()
        .get(&referendum_id)
        .expect("accepted ballots must create proposal-bound locks");
    assert_eq!(locks.locks.len(), 3);
    for (owner, expected_direction) in [
        (&proposer, 0),
        (&late_nay_voter, 1),
        (&late_abstain_voter, 2),
    ] {
        let lock = locks.locks.get(owner).expect("accepted voter lock");
        assert_eq!(lock.amount, retained_rules.ballot_amount);
        assert_eq!(lock.duration_blocks, retained_rules.ballot_duration_blocks);
        assert_eq!(lock.direction, expected_direction);
        let custody = &lock.custody;
        assert!(custody.escrowed);
        assert_eq!(custody.asset_definition_id, voting_asset_id);
        assert_eq!(custody.bond_escrow_account, bond_escrow);
        assert_eq!(custody.slash_receiver_account, parliament_signer);
    }
    for owner in [&proposer, &late_nay_voter, &late_abstain_voter] {
        let voter_asset_id = AssetId::new(voting_asset_id.clone(), owner.clone());
        let balance = state_transaction
            .world
            .asset(&voter_asset_id)
            .expect("voter balance after immutable-custody lock");
        assert_eq!(**balance, Quantity::from(500 - BALLOT_AMOUNT));
    }
    let escrow_asset_id = AssetId::new(voting_asset_id.clone(), bond_escrow.clone());
    let escrow_balance = state_transaction
        .world
        .asset(&escrow_asset_id)
        .expect("proposal-bound escrow balance");
    assert_eq!(**escrow_balance, Quantity::from(3 * BALLOT_AMOUNT));
    let changed_live_escrow_asset_id =
        AssetId::new(voting_asset_id.clone(), parliament_signer.clone());
    let changed_live_escrow_balance = state_transaction
        .world
        .asset(&changed_live_escrow_asset_id)
        .expect("changed live escrow account balance");
    assert_eq!(**changed_live_escrow_balance, Quantity::from(500_u64));
    assert_eq!(
        state_transaction
            .world
            .governance_referenda()
            .get(&referendum_id)
            .expect("referendum")
            .status,
        GovernanceReferendumStatus::Open
    );
    let mut malformed_locks = state_transaction
        .world
        .governance_locks()
        .get(&referendum_id)
        .cloned()
        .expect("accepted validation-fee locks");
    malformed_locks
        .locks
        .get_mut(&proposer)
        .expect("proposal operator lock")
        .custody
        .bond_escrow_account = parliament_signer.clone();
    state_transaction
        .world
        .governance_locks_mut()
        .insert(referendum_id.clone(), malformed_locks);
    let permission: Permission = CanSlashGovernanceLock {
        referendum_id: referendum_id.clone(),
    }
    .into();
    Grant::account_permission(permission, proposer.clone())
        .execute(&proposer, &mut state_transaction)
        .expect("grant manual slash permission");
    let escrow_before = state_transaction
        .world
        .asset(&AssetId::new(voting_asset_id.clone(), bond_escrow.clone()))
        .expect("proposal-bound escrow")
        .as_ref()
        .clone();
    let receiver_before = state_transaction
        .world
        .asset(&AssetId::new(
            voting_asset_id.clone(),
            parliament_signer.clone(),
        ))
        .expect("proposal-bound slash receiver")
        .as_ref()
        .clone();
    let error = SlashGovernanceLock {
        referendum_id: referendum_id.clone(),
        owner: proposer.clone(),
        amount: 1_u64.into(),
        reason: "malformed retained custody".to_owned(),
    }
    .execute(&proposer, &mut state_transaction)
    .expect_err("validation-fee slash must reject mismatched immutable custody");
    assert!(
        error
            .to_string()
            .contains("differs from its immutable proposal rules"),
        "unexpected malformed-custody rejection: {error}"
    );
    assert_eq!(
        state_transaction
            .world
            .asset(&AssetId::new(voting_asset_id.clone(), bond_escrow))
            .expect("escrow remains after rejected slash")
            .as_ref()
            .clone(),
        escrow_before
    );
    assert_eq!(
        state_transaction
            .world
            .asset(&AssetId::new(voting_asset_id, parliament_signer))
            .expect("slash receiver remains after rejected slash")
            .as_ref()
            .clone(),
        receiver_before
    );
}
