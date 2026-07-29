//! Typed Torii surface for Parliament-governed validation-fee state.

use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
};
use iroha_core::state::{StateReadOnly as _, WorldReadOnly as _};
use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{
    account::AccountId,
    governance::types::{AtWindow, ParliamentBody, ProposalKind},
    isi::{
        InstructionBox, frame_instruction_payload,
        governance::{
            CastPlainBallot, ProposeValidationFeePayoutLifecycle, ProposeValidationFeePolicy,
            VotingMode,
        },
    },
    validation_fee::{
        ValidationFeePlainElectorateRulesV1, ValidationFeePlainElectorateSnapshotV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicySnapshotCommitmentV1,
    },
};
use iroha_torii_shared::validation_fee_api::{
    VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES,
    VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES, VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
    VALIDATION_FEE_PROPOSAL_API_VERSION_V1, ValidationFeeCurrentPolicyProofRequestV1,
    ValidationFeeCurrentPolicyProofV1, ValidationFeeParliamentBodyProgressV1,
    ValidationFeeParliamentSnapshotV1, ValidationFeePlainBallotDirectionV1,
    ValidationFeePlainBallotDraftRequestV1, ValidationFeePlainBallotDraftResponseV1,
    ValidationFeeProposalDetailQueryV1, ValidationFeeProposalDetailV1,
    ValidationFeeProposalDraftPayloadV1, ValidationFeeProposalDraftRequestV1,
    ValidationFeeProposalDraftResponseV1, ValidationFeeProposalInstructionDraftV1,
    ValidationFeeProposalListV1, ValidationFeeProposalLockV1, ValidationFeeProposalLocksV1,
    ValidationFeeProposalPipelineStageV1, ValidationFeeProposalPipelineV1,
    ValidationFeeProposalRecordV1, ValidationFeeProposalReferendumV1,
    ValidationFeeProposalStatusV1, ValidationFeeProposalTallyV1,
    validation_fee_policy_proof_page_tip,
};
use mv::storage::StorageReadOnly as _;

use crate::{
    Error, JsonBody, NoritoBody, NoritoJson, NoritoQuery, SharedAppState, check_access,
    gov::validation_fee_plain_electorate_rules, utils::extractors::NoritoOnly,
};

fn inconsistent(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "validation_fee_state_inconsistent",
        message: message.into(),
    }
}

fn bad_request(message: impl Into<String>) -> Error {
    Error::AppQueryValidation {
        code: "validation_fee_request_invalid",
        message: message.into(),
    }
}

fn not_found(message: impl Into<String>) -> Error {
    Error::AppNotFound {
        code: "validation_fee_proposal_not_found",
        message: message.into(),
    }
}

fn parse_proposal_id(value: &str) -> Result<[u8; 32], Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(bad_request(
            "proposal_id must be exactly 64 lowercase hexadecimal digits",
        ));
    }
    let bytes = hex::decode(value)
        .map_err(|_| bad_request("proposal_id must be exactly 64 lowercase hexadecimal digits"))?;
    bytes
        .try_into()
        .map_err(|_| bad_request("proposal_id must decode to exactly 32 bytes"))
}

fn retained_plain_electorate_rules(
    proposal_kind: &ProposalKind,
) -> Result<&ValidationFeePlainElectorateRulesV1, Error> {
    let rules = match proposal_kind {
        ProposalKind::ValidationFeePolicy(payload) => &payload.plain_electorate_rules,
        ProposalKind::ValidationFeePayoutLifecycle(payload) => &payload.plain_electorate_rules,
        ProposalKind::DeployContract(_)
        | ProposalKind::RuntimeUpgrade(_)
        | ProposalKind::SccpRouteGovernance(_) => {
            return Err(inconsistent(
                "non-validation-fee proposal reached the typed validation-fee projection",
            ));
        }
    };
    if let Some(reason) = rules.invariant_error() {
        return Err(inconsistent(format!(
            "validation-fee proposal retained invalid PLAIN electorate rules: {reason}"
        )));
    }
    Ok(rules)
}

const VALIDATION_FEE_PARLIAMENT_BODIES: [ParliamentBody; 7] = [
    ParliamentBody::RulesCommittee,
    ParliamentBody::AgendaCouncil,
    ParliamentBody::InterestPanel,
    ParliamentBody::ReviewPanel,
    ParliamentBody::PolicyJury,
    ParliamentBody::OversightCommittee,
    ParliamentBody::FmaCommittee,
];

fn validate_retained_parliament_snapshot(
    proposal: &iroha_core::state::GovernanceProposalRecord,
) -> Result<(), Error> {
    let snapshot = proposal
        .parliament_snapshot
        .as_ref()
        .ok_or_else(|| inconsistent("validation-fee proposal has no Parliament snapshot"))?;
    if snapshot.bodies.selection_epoch != snapshot.selection_epoch {
        return Err(inconsistent(
            "validation-fee Parliament snapshot selection epoch is inconsistent",
        ));
    }
    for body in VALIDATION_FEE_PARLIAMENT_BODIES {
        let roster = snapshot.bodies.rosters.get(&body).ok_or_else(|| {
            inconsistent(format!(
                "validation-fee Parliament snapshot is missing {body:?}"
            ))
        })?;
        if roster.body != body || roster.epoch != snapshot.selection_epoch {
            return Err(inconsistent(format!(
                "validation-fee Parliament roster for {body:?} is inconsistent"
            )));
        }
        let members = roster
            .members
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let alternates = roster
            .alternates
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        if members.len() != roster.members.len()
            || alternates.len() != roster.alternates.len()
            || !members.is_disjoint(&alternates)
        {
            return Err(inconsistent(format!(
                "validation-fee Parliament roster for {body:?} has duplicate membership"
            )));
        }
    }
    let encoded = norito::to_bytes(&snapshot.bodies)
        .map_err(|_| inconsistent("validation-fee Parliament snapshot cannot be encoded"))?;
    let digest = Blake2b512::digest(encoded);
    let mut roster_root = [0_u8; 32];
    roster_root.copy_from_slice(&digest[..32]);
    if roster_root != snapshot.roster_root {
        return Err(inconsistent(
            "validation-fee Parliament snapshot roster root is inconsistent",
        ));
    }
    Ok(())
}

fn retained_plain_electorate_snapshot<'a>(
    proposal_id: [u8; 32],
    proposal: &iroha_core::state::GovernanceProposalRecord,
    referendum: iroha_core::state::GovernanceReferendumRecord,
    approvals: Option<&'a iroha_core::state::GovernanceStageApprovals>,
    rules: &ValidationFeePlainElectorateRulesV1,
) -> Result<Option<&'a ValidationFeePlainElectorateSnapshotV1>, Error> {
    let snapshot =
        approvals.and_then(|records| records.validation_fee_plain_electorate_snapshot.as_ref());
    if referendum.status == iroha_core::state::GovernanceReferendumStatus::Proposed {
        if snapshot.is_some() {
            return Err(inconsistent(
                "validation-fee proposal froze its PLAIN electorate before referendum start",
            ));
        }
        return Ok(None);
    }
    let approvals = approvals.ok_or_else(|| {
        inconsistent("validation-fee proposal has no retained Parliament approval gate")
    })?;
    let snapshot = snapshot.ok_or_else(|| {
        inconsistent("validation-fee proposal has no frozen PLAIN electorate snapshot")
    })?;
    if let Some(reason) = snapshot.context_error(proposal_id, &proposal.proposer, rules) {
        return Err(inconsistent(format!(
            "validation-fee proposal retained an invalid PLAIN electorate snapshot: {reason}"
        )));
    }
    if snapshot.captured_at_height != referendum.h_start
        || approvals.approval_gate_height != Some(snapshot.approval_gate_height)
    {
        return Err(inconsistent(
            "validation-fee PLAIN electorate snapshot anchors differ from retained governance state",
        ));
    }
    Ok(Some(snapshot))
}

fn validate_retained_proposal_state<'a>(
    proposal_id: [u8; 32],
    proposal: &iroha_core::state::GovernanceProposalRecord,
    referendum: iroha_core::state::GovernanceReferendumRecord,
    approvals: Option<&'a iroha_core::state::GovernanceStageApprovals>,
    rules: &ValidationFeePlainElectorateRulesV1,
) -> Result<Option<&'a ValidationFeePlainElectorateSnapshotV1>, Error> {
    validate_retained_parliament_snapshot(proposal)?;
    let span = referendum
        .h_end
        .checked_sub(referendum.h_start)
        .and_then(|distance| distance.checked_add(1))
        .ok_or_else(|| inconsistent("validation-fee referendum window is invalid"))?;
    if span != rules.ballot_duration_blocks {
        return Err(inconsistent(
            "validation-fee referendum window differs from retained PLAIN electorate rules",
        ));
    }
    let electorate =
        retained_plain_electorate_snapshot(proposal_id, proposal, referendum, approvals, rules)?;

    let Some(evidence) = proposal.finalization_evidence.as_ref() else {
        if referendum.status == iroha_core::state::GovernanceReferendumStatus::Closed
            || proposal.status != iroha_core::state::GovernanceProposalStatus::Proposed
        {
            return Err(inconsistent(
                "validation-fee proposal status differs from its missing finalization evidence",
            ));
        }
        return Ok(electorate);
    };
    if referendum.status != iroha_core::state::GovernanceReferendumStatus::Closed
        || evidence.proposal_id != proposal_id
        || evidence.referendum_id != proposal_id
        || evidence.finalized_at_height != referendum.h_end
        || evidence.mode != VotingMode::Plain
        || evidence.min_turnout != rules.min_turnout
        || evidence.approval_threshold_numerator != rules.approval_threshold_numerator
        || evidence.approval_threshold_denominator != rules.approval_threshold_denominator
    {
        return Err(inconsistent(
            "validation-fee finalization evidence differs from retained proposal state",
        ));
    }
    let turnout = evidence
        .approve
        .checked_add(evidence.reject)
        .and_then(|value| value.checked_add(evidence.abstain))
        .ok_or_else(|| inconsistent("validation-fee finalization turnout overflow"))?;
    let expected_approval = if turnout >= rules.min_turnout {
        let approve = evidence
            .approve
            .checked_mul(u128::from(rules.approval_threshold_denominator))
            .ok_or_else(|| inconsistent("validation-fee finalization approval overflow"))?;
        let required = turnout
            .checked_mul(u128::from(rules.approval_threshold_numerator))
            .ok_or_else(|| inconsistent("validation-fee finalization threshold overflow"))?;
        approve >= required
    } else {
        false
    };
    if evidence.approved != expected_approval {
        return Err(inconsistent(
            "validation-fee finalization decision does not match its checked tally",
        ));
    }
    let status_matches = match proposal.status {
        iroha_core::state::GovernanceProposalStatus::Approved
        | iroha_core::state::GovernanceProposalStatus::Enacted
        | iroha_core::state::GovernanceProposalStatus::Superseded => evidence.approved,
        iroha_core::state::GovernanceProposalStatus::Rejected => !evidence.approved,
        iroha_core::state::GovernanceProposalStatus::Proposed => false,
    };
    if !status_matches {
        return Err(inconsistent(
            "validation-fee proposal status differs from its finalization decision",
        ));
    }
    Ok(electorate)
}

fn parliament_body_progress(
    proposal: &iroha_core::state::GovernanceProposalRecord,
    approvals: Option<&iroha_core::state::GovernanceStageApprovals>,
    quorum_bps: u16,
    account_id: Option<&iroha_data_model::account::AccountId>,
) -> Result<Vec<ValidationFeeParliamentBodyProgressV1>, Error> {
    let snapshot = proposal
        .parliament_snapshot
        .as_ref()
        .ok_or_else(|| inconsistent("validation-fee proposal has no Parliament snapshot"))?;
    let mut progress = Vec::with_capacity(7);
    for body in [
        iroha_data_model::governance::types::ParliamentBody::RulesCommittee,
        iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
        iroha_data_model::governance::types::ParliamentBody::InterestPanel,
        iroha_data_model::governance::types::ParliamentBody::ReviewPanel,
        iroha_data_model::governance::types::ParliamentBody::PolicyJury,
        iroha_data_model::governance::types::ParliamentBody::OversightCommittee,
        iroha_data_model::governance::types::ParliamentBody::FmaCommittee,
    ] {
        let roster = snapshot.bodies.rosters.get(&body).ok_or_else(|| {
            inconsistent(format!(
                "validation-fee Parliament snapshot is missing {body:?}"
            ))
        })?;
        let stage = approvals.and_then(|records| records.stages.get(&body));
        if let Some(stage) = stage {
            let expected_required =
                iroha_core::state::council_quorum_threshold(roster.members.len(), stage.quorum_bps);
            let decision_sets_are_disjoint = stage.approvers.is_disjoint(&stage.rejections)
                && stage.approvers.is_disjoint(&stage.abstentions)
                && stage.rejections.is_disjoint(&stage.abstentions);
            let decisions_are_members = stage
                .approvers
                .iter()
                .chain(&stage.rejections)
                .chain(&stage.abstentions)
                .all(|account| roster.members.contains(account));
            if stage.epoch != snapshot.selection_epoch
                || stage.quorum_bps == 0
                || stage.quorum_bps > 10_000
                || stage.required != expected_required
                || !decision_sets_are_disjoint
                || !decisions_are_members
            {
                return Err(inconsistent(format!(
                    "validation-fee Parliament decisions for {body:?} are inconsistent"
                )));
            }
        }
        let required = stage.map_or_else(
            || iroha_core::state::council_quorum_threshold(roster.members.len(), quorum_bps),
            |record| record.required,
        );
        let current_account_decision = account_id.and_then(|account| {
            stage.and_then(|record| {
                if record.approvers.contains(account) {
                    Some("APPROVE".to_owned())
                } else if record.rejections.contains(account) {
                    Some("REJECT".to_owned())
                } else if record.abstentions.contains(account) {
                    Some("ABSTAIN".to_owned())
                } else {
                    None
                }
            })
        });
        let approve = stage.map_or(0, |record| {
            u32::try_from(record.approvers.len()).unwrap_or(u32::MAX)
        });
        let reject = stage.map_or(0, |record| {
            u32::try_from(record.rejections.len()).unwrap_or(u32::MAX)
        });
        let abstain = stage.map_or(0, |record| {
            u32::try_from(record.abstentions.len()).unwrap_or(u32::MAX)
        });
        progress.push(ValidationFeeParliamentBodyProgressV1 {
            body,
            members: roster.members.clone(),
            alternates: roster.alternates.clone(),
            required: required.to_string(),
            approve: approve.to_string(),
            reject: reject.to_string(),
            abstain: abstain.to_string(),
            approval_quorum_met: approve >= required,
            rejection_quorum_met: required > 0 && reject >= required,
            current_account_decision,
        });
    }
    Ok(progress)
}

fn integer_sqrt_u128(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x0 = n;
    let mut x1 = u128::midpoint(x0, n / x0);
    while x1 < x0 {
        x0 = x1;
        x1 = u128::midpoint(x0, n / x0);
    }
    x0
}

fn live_plain_tally(
    locks: Option<&iroha_core::state::GovernanceLocksForReferendum>,
    referendum_end: u64,
    rules: &ValidationFeePlainElectorateRulesV1,
    electorate: Option<&ValidationFeePlainElectorateSnapshotV1>,
) -> Result<(u128, u128, u128), Error> {
    let mut approve = 0_u128;
    let mut reject = 0_u128;
    let mut abstain = 0_u128;
    let Some(locks) = locks else {
        return Ok((approve, reject, abstain));
    };
    let electorate = electorate.ok_or_else(|| {
        inconsistent("validation-fee citizen locks exist without a frozen PLAIN electorate")
    })?;
    for (owner, record) in &locks.locks {
        if &record.owner != owner
            || record.amount != rules.ballot_amount
            || record.duration_blocks != rules.ballot_duration_blocks
            || !electorate.contains(owner)
        {
            return Err(inconsistent(
                "validation-fee citizen lock differs from retained PLAIN electorate rules or frozen membership",
            ));
        }
        if record.expiry_height < referendum_end {
            return Err(inconsistent(
                "validation-fee citizen lock expires before the retained referendum end",
            ));
        }
        let units = record.amount.to_string().parse::<u128>().map_err(|_| {
            inconsistent("validation-fee citizen ballot amount is outside the integer tally domain")
        })?;
        let factor = 1_u64
            .saturating_add(record.duration_blocks / rules.conviction_step_blocks)
            .min(rules.max_conviction);
        let weight = integer_sqrt_u128(units)
            .checked_mul(u128::from(factor))
            .ok_or_else(|| inconsistent("validation-fee citizen tally overflow"))?;
        let target = match record.direction {
            0 => &mut approve,
            1 => &mut reject,
            2 => &mut abstain,
            _ => {
                return Err(inconsistent(
                    "validation-fee citizen lock has an invalid ballot direction",
                ));
            }
        };
        *target = target
            .checked_add(weight)
            .ok_or_else(|| inconsistent("validation-fee citizen tally overflow"))?;
    }
    Ok((approve, reject, abstain))
}

fn public_pipeline(
    proposal: &iroha_core::state::GovernanceProposalRecord,
) -> ValidationFeeProposalPipelineV1 {
    ValidationFeeProposalPipelineV1 {
        stages: proposal
            .pipeline
            .stages
            .iter()
            .map(|stage| ValidationFeeProposalPipelineStageV1 {
                stage: format!("{:?}", stage.stage),
                started_at: stage.started_at.to_string(),
                deadline: stage.deadline.map(|height| height.to_string()),
                completed_at: stage.completed_at.map(|height| height.to_string()),
                failure: stage.failure.map(|failure| format!("{failure:?}")),
            })
            .collect(),
    }
}

fn public_locks(
    locks: Option<&iroha_core::state::GovernanceLocksForReferendum>,
) -> ValidationFeeProposalLocksV1 {
    let locks = locks
        .into_iter()
        .flat_map(|records| records.locks.iter())
        .map(|(account_id, record)| {
            let direction = match record.direction {
                0 => "Aye",
                1 => "Nay",
                2 => "Abstain",
                _ => "Unknown",
            };
            (
                account_id.clone(),
                ValidationFeeProposalLockV1 {
                    owner: record.owner.clone(),
                    amount: record.amount.to_string(),
                    slashed: record.slashed.to_string(),
                    expiry_height: record.expiry_height.to_string(),
                    direction: direction.to_owned(),
                    duration_blocks: record.duration_blocks.to_string(),
                },
            )
        })
        .collect();
    ValidationFeeProposalLocksV1 { locks }
}

fn public_proposal_record(
    proposal_id: [u8; 32],
    proposal: &iroha_core::state::GovernanceProposalRecord,
    referendum: iroha_core::state::GovernanceReferendumRecord,
    approvals: Option<&iroha_core::state::GovernanceStageApprovals>,
) -> Result<ValidationFeeProposalRecordV1, Error> {
    if !matches!(
        proposal.kind,
        ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
    ) {
        return Err(inconsistent(
            "non-validation-fee proposal reached the typed validation-fee projection",
        ));
    }
    if proposal.kind.fingerprint() != proposal_id {
        return Err(inconsistent(
            "validation-fee proposal identifier differs from its native fingerprint",
        ));
    }
    if referendum.mode != iroha_core::state::GovernanceReferendumMode::Plain {
        return Err(inconsistent(
            "validation-fee proposal retained a non-plain referendum",
        ));
    }
    let rules = retained_plain_electorate_rules(&proposal.kind)?;
    let electorate =
        validate_retained_proposal_state(proposal_id, proposal, referendum, approvals, rules)?;
    let snapshot = proposal
        .parliament_snapshot
        .as_ref()
        .ok_or_else(|| inconsistent("validation-fee proposal has no Parliament snapshot"))?;
    let status = match proposal.status {
        iroha_core::state::GovernanceProposalStatus::Proposed => {
            ValidationFeeProposalStatusV1::Proposed
        }
        iroha_core::state::GovernanceProposalStatus::Approved => {
            ValidationFeeProposalStatusV1::Approved
        }
        iroha_core::state::GovernanceProposalStatus::Rejected => {
            ValidationFeeProposalStatusV1::Rejected
        }
        iroha_core::state::GovernanceProposalStatus::Enacted => {
            ValidationFeeProposalStatusV1::Enacted
        }
        iroha_core::state::GovernanceProposalStatus::Superseded => {
            ValidationFeeProposalStatusV1::Superseded
        }
    };
    let (opened, closed) = match referendum.status {
        iroha_core::state::GovernanceReferendumStatus::Proposed => (false, false),
        iroha_core::state::GovernanceReferendumStatus::Open => (true, false),
        iroha_core::state::GovernanceReferendumStatus::Closed => (true, true),
    };
    Ok(ValidationFeeProposalRecordV1 {
        proposal_id: hex::encode(proposal_id),
        proposer: proposal.proposer.clone(),
        proposal_kind: proposal.kind.clone(),
        created_height: proposal.created_height.to_string(),
        status,
        pipeline: public_pipeline(proposal),
        referendum: ValidationFeeProposalReferendumV1 {
            window: AtWindow {
                lower: referendum.h_start,
                upper: referendum.h_end,
            },
            mode: VotingMode::Plain,
            opened,
            closed,
        },
        parliament_snapshot: ValidationFeeParliamentSnapshotV1 {
            selection_epoch: snapshot.selection_epoch.to_string(),
            beacon: snapshot.beacon,
            roster_root: snapshot.roster_root,
            bodies: snapshot.bodies.clone(),
        },
        plain_electorate_snapshot: electorate.cloned(),
        finalization_evidence: proposal.finalization_evidence,
        enacted_at_height: proposal.enacted_at_height.map(|height| height.to_string()),
    })
}

/// Return all typed validation-fee Parliament proposals.
pub(crate) async fn handler_proposals(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
) -> Result<JsonBody<ValidationFeeProposalListV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/proposals",
    )
    .await?;
    let world = app.state.world_view();
    let mut proposals = Vec::new();
    for (proposal_id, proposal) in world.governance_proposals().iter() {
        if !matches!(
            proposal.kind,
            ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
        ) {
            continue;
        }
        let referendum_id = hex::encode(proposal_id);
        let referendum = world
            .governance_referenda()
            .get(&referendum_id)
            .copied()
            .ok_or_else(|| {
                inconsistent("validation-fee proposal has no exact retained referendum")
            })?;
        let approvals = world.governance_stage_approvals().get(&referendum_id);
        proposals.push(public_proposal_record(
            *proposal_id,
            proposal,
            referendum,
            approvals,
        )?);
    }
    proposals.sort_by(|left, right| {
        left.created_height
            .parse::<u64>()
            .expect("Core-created proposal height is canonical")
            .cmp(
                &right
                    .created_height
                    .parse::<u64>()
                    .expect("Core-created proposal height is canonical"),
            )
            .then_with(|| left.proposal_id.cmp(&right.proposal_id))
    });
    Ok(JsonBody(ValidationFeeProposalListV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposals,
    }))
}

/// Return one typed validation-fee Parliament proposal.
pub(crate) async fn handler_proposal_detail(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    Path(proposal_id): Path<String>,
    NoritoQuery(query): NoritoQuery<ValidationFeeProposalDetailQueryV1>,
) -> Result<JsonBody<ValidationFeeProposalDetailV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/proposals/{proposal_id}",
    )
    .await?;
    let proposal_id_bytes = parse_proposal_id(&proposal_id)?;
    let world = app.state.world_view();
    let proposal = world
        .governance_proposals()
        .get(&proposal_id_bytes)
        .ok_or_else(|| not_found("validation-fee proposal was not found"))?;
    if !matches!(
        proposal.kind,
        ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
    ) {
        return Err(not_found("validation-fee proposal was not found"));
    }
    let plain_electorate_rules = retained_plain_electorate_rules(&proposal.kind)?;
    let referendum = world
        .governance_referenda()
        .get(&proposal_id)
        .copied()
        .ok_or_else(|| inconsistent("validation-fee proposal has no exact retained referendum"))?;
    let approvals = world.governance_stage_approvals().get(&proposal_id);
    let electorate = validate_retained_proposal_state(
        proposal_id_bytes,
        proposal,
        referendum,
        approvals,
        plain_electorate_rules,
    )?;
    let gov = app.state.governance_snapshot();
    let body_progress = parliament_body_progress(
        proposal,
        approvals,
        gov.parliament_quorum_bps,
        query.account_id.as_ref(),
    )?;
    let live_tally = live_plain_tally(
        world.governance_locks().get(&proposal_id),
        referendum.h_end,
        plain_electorate_rules,
        electorate,
    )?;
    let (approve, reject, abstain, approved) = if let Some(evidence) =
        proposal.finalization_evidence.as_ref()
    {
        if evidence.mode != VotingMode::Plain
            || evidence.min_turnout != plain_electorate_rules.min_turnout
            || evidence.approval_threshold_numerator
                != plain_electorate_rules.approval_threshold_numerator
            || evidence.approval_threshold_denominator
                != plain_electorate_rules.approval_threshold_denominator
        {
            return Err(inconsistent(
                "validation-fee finalization evidence differs from retained PLAIN electorate rules",
            ));
        }
        (
            evidence.approve,
            evidence.reject,
            evidence.abstain,
            Some(evidence.approved),
        )
    } else {
        let (approve, reject, abstain) = live_tally;
        (approve, reject, abstain, None)
    };
    let turnout = approve
        .checked_add(reject)
        .and_then(|value| value.checked_add(abstain))
        .ok_or_else(|| inconsistent("validation-fee citizen tally overflow"))?;
    Ok(JsonBody(ValidationFeeProposalDetailV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal: public_proposal_record(proposal_id_bytes, proposal, referendum, approvals)?,
        current_height: u64::try_from(app.state.committed_height())
            .unwrap_or(u64::MAX)
            .to_string(),
        body_progress,
        tally: ValidationFeeProposalTallyV1 {
            approve: approve.to_string(),
            reject: reject.to_string(),
            abstain: abstain.to_string(),
            turnout: turnout.to_string(),
            min_turnout: plain_electorate_rules.min_turnout.to_string(),
            approval_threshold_numerator: plain_electorate_rules
                .approval_threshold_numerator
                .to_string(),
            approval_threshold_denominator: plain_electorate_rules
                .approval_threshold_denominator
                .to_string(),
            approved,
        },
        locks: public_locks(world.governance_locks().get(&proposal_id)),
    }))
}

fn canonical_draft_instruction(
    request: &ValidationFeeProposalDraftRequestV1,
) -> Result<(ProposalKind, InstructionBox), Error> {
    if request.version != VALIDATION_FEE_PROPOSAL_API_VERSION_V1 {
        return Err(bad_request(
            "unsupported validation-fee proposal draft version",
        ));
    }
    if request
        .referendum_window
        .is_some_and(|window| window.upper < window.lower)
    {
        return Err(bad_request("validation-fee referendum window is reversed"));
    }
    if request.mode == Some(VotingMode::Zk) {
        return Err(bad_request(
            "validation-fee governance supports plain referendum voting only",
        ));
    }
    if let Some(reason) = request.plain_electorate_rules.invariant_error() {
        return Err(bad_request(format!(
            "invalid validation-fee PLAIN electorate rules: {reason}"
        )));
    }
    let proposal_kind = request
        .proposal
        .proposal_kind(&request.plain_electorate_rules);
    let instruction: InstructionBox = match &request.proposal {
        ValidationFeeProposalDraftPayloadV1::Policy {
            policy,
            payout_lifecycle_proposal_id,
        } => {
            if let Some(reason) = policy.policy_invariant_error() {
                return Err(bad_request(format!(
                    "invalid validation-fee policy: {reason}"
                )));
            }
            match (
                policy.treasury_payout_binding.as_ref(),
                payout_lifecycle_proposal_id,
            ) {
                (None, None) => {}
                (Some(_), Some(id)) if *id != [0; 32] => {}
                (Some(_), _) => {
                    return Err(bad_request(
                        "payout-enabled policy requires a non-zero lifecycle proposal id",
                    ));
                }
                (None, Some(_)) => {
                    return Err(bad_request(
                        "policy without a payout binding cannot select a lifecycle proposal",
                    ));
                }
            }
            ProposeValidationFeePolicy {
                policy: policy.clone(),
                payout_lifecycle_proposal_id: *payout_lifecycle_proposal_id,
                referendum_window: request.referendum_window,
                mode: Some(VotingMode::Plain),
                plain_electorate_rules: request.plain_electorate_rules.clone(),
            }
            .into()
        }
        ValidationFeeProposalDraftPayloadV1::PayoutLifecycle { payout_binding } => {
            if let Some(reason) = payout_binding.invariant_error() {
                return Err(bad_request(format!(
                    "invalid validation-fee payout lifecycle: {reason}"
                )));
            }
            let lifecycle_seal = payout_binding.lifecycle_seal().map_err(|error| {
                bad_request(format!(
                    "validation-fee payout lifecycle cannot be encoded: {error}"
                ))
            })?;
            if lifecycle_seal == [0; 32] {
                return Err(bad_request(
                    "validation-fee payout lifecycle derives an invalid zero seal",
                ));
            }
            ProposeValidationFeePayoutLifecycle {
                payout_binding: payout_binding.clone(),
                referendum_window: request.referendum_window,
                mode: Some(VotingMode::Plain),
                plain_electorate_rules: request.plain_electorate_rules.clone(),
            }
            .into()
        }
    };
    Ok((proposal_kind, instruction))
}

fn validate_draft_plain_electorate_rules(
    requested: &ValidationFeePlainElectorateRulesV1,
    gov: &iroha_config::parameters::actual::Governance,
) -> Result<(), Error> {
    if !gov.plain_voting_enabled {
        return Err(inconsistent(
            "validation-fee proposal drafting requires active PLAIN governance voting",
        ));
    }
    if gov.voting_asset_id != gov.citizenship_asset_id {
        return Err(inconsistent(
            "validation-fee PLAIN governance requires one shared voting and citizenship asset",
        ));
    }
    let expected = validation_fee_plain_electorate_rules(gov);
    if let Some(reason) = expected.invariant_error() {
        return Err(inconsistent(format!(
            "active governance cannot form valid validation-fee PLAIN electorate rules: {reason}"
        )));
    }
    if requested != &expected {
        return Err(bad_request(
            "validation-fee PLAIN electorate rules differ from active governance policy",
        ));
    }
    Ok(())
}

fn validate_draft_referendum_window(
    window: Option<AtWindow>,
    current_tip: u64,
    min_enactment_delay: u64,
    configured_window_span: u64,
) -> Result<(), String> {
    let Some(window) = window else {
        // An omitted lifecycle window is resolved atomically by Core from the
        // actual proposal-inclusion height.
        return Ok(());
    };
    let earliest_lower = current_tip
        .checked_add(1)
        .and_then(|height| height.checked_add(min_enactment_delay))
        .ok_or_else(|| {
            "validation-fee referendum staging height overflows the block-height domain".to_owned()
        })?;
    if window.lower < earliest_lower {
        return Err(format!(
            "validation-fee referendum window lower must be at least {earliest_lower} (current tip + one inclusion block + configured minimum delay)"
        ));
    }
    let actual_span = window
        .upper
        .checked_sub(window.lower)
        .and_then(|distance| distance.checked_add(1))
        .ok_or_else(|| "validation-fee referendum window is reversed or overflows".to_owned())?;
    let required_span = configured_window_span.max(1);
    if actual_span != required_span {
        return Err(format!(
            "validation-fee referendum window must span exactly {required_span} blocks"
        ));
    }
    Ok(())
}

fn framed_instruction_draft(
    instruction: &InstructionBox,
) -> Result<ValidationFeeProposalInstructionDraftV1, Error> {
    let wire_id = iroha_data_model::isi::Instruction::id(&**instruction).to_string();
    let payload = iroha_data_model::isi::Instruction::dyn_encode(&**instruction);
    let framed = frame_instruction_payload(&wire_id, &payload).map_err(|error| {
        inconsistent(format!(
            "failed to frame native validation-fee instruction: {error}"
        ))
    })?;
    Ok(ValidationFeeProposalInstructionDraftV1 {
        wire_id,
        payload_hex: hex::encode(framed),
    })
}

/// Build one exact native validation-fee proposal instruction for local signing.
pub(crate) async fn handler_proposal_draft(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<ValidationFeeProposalDraftRequestV1>,
) -> Result<JsonBody<ValidationFeeProposalDraftResponseV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/proposals/draft",
    )
    .await?;
    let (proposal_kind, instruction) = canonical_draft_instruction(&request)?;
    let gov = app.state.governance_snapshot();
    validate_draft_plain_electorate_rules(&request.plain_electorate_rules, &gov)?;
    let current_tip = u64::try_from(app.state.committed_height())
        .map_err(|_| inconsistent("ledger height does not fit validation-fee draft timing"))?;
    validate_draft_referendum_window(
        request.referendum_window,
        current_tip,
        gov.min_enactment_delay,
        gov.window_span,
    )
    .map_err(bad_request)?;
    let proposal_id = proposal_kind.fingerprint();
    let instruction = framed_instruction_draft(&instruction)?;
    Ok(JsonBody(ValidationFeeProposalDraftResponseV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal_id: hex::encode(proposal_id),
        proposal_kind,
        tx_instructions: vec![instruction],
    }))
}

fn canonical_plain_ballot(
    proposal_id: &str,
    owner: AccountId,
    direction: ValidationFeePlainBallotDirectionV1,
    rules: &ValidationFeePlainElectorateRulesV1,
) -> CastPlainBallot {
    CastPlainBallot {
        referendum_id: proposal_id.to_owned(),
        owner,
        amount: rules.ballot_amount.clone(),
        duration_blocks: rules.ballot_duration_blocks,
        direction: direction.native_code(),
    }
}

/// Build one exact proposal-bound PLAIN ballot instruction for local signing.
pub(crate) async fn handler_plain_ballot_draft(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    Path(proposal_id): Path<String>,
    NoritoJson(request): NoritoJson<ValidationFeePlainBallotDraftRequestV1>,
) -> Result<JsonBody<ValidationFeePlainBallotDraftResponseV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft",
    )
    .await?;
    if request.version != VALIDATION_FEE_PROPOSAL_API_VERSION_V1 {
        return Err(bad_request(
            "unsupported validation-fee PLAIN ballot draft version",
        ));
    }

    let proposal_id_bytes = parse_proposal_id(&proposal_id)?;
    let world = app.state.world_view();
    let proposal = world
        .governance_proposals()
        .get(&proposal_id_bytes)
        .ok_or_else(|| not_found("validation-fee proposal was not found"))?;
    if proposal.kind.fingerprint() != proposal_id_bytes {
        return Err(inconsistent(
            "validation-fee proposal identifier differs from its native fingerprint",
        ));
    }
    let rules = retained_plain_electorate_rules(&proposal.kind)?;
    let referendum = world
        .governance_referenda()
        .get(&proposal_id)
        .copied()
        .ok_or_else(|| inconsistent("validation-fee proposal has no exact retained referendum"))?;
    let approvals = world
        .governance_stage_approvals()
        .get(&proposal_id)
        .ok_or_else(|| {
            inconsistent("validation-fee proposal has no retained Parliament approval gate")
        })?;
    let electorate = validate_retained_proposal_state(
        proposal_id_bytes,
        proposal,
        referendum,
        Some(approvals),
        rules,
    )?
    .ok_or_else(|| inconsistent("open validation-fee referendum has no frozen PLAIN electorate"))?;
    if referendum.mode != iroha_core::state::GovernanceReferendumMode::Plain {
        return Err(inconsistent(
            "validation-fee proposal retained a non-plain referendum",
        ));
    }
    let next_height = u64::try_from(app.state.committed_height())
        .map_err(|_| inconsistent("ledger height does not fit validation-fee ballot timing"))?
        .checked_add(1)
        .ok_or_else(|| inconsistent("validation-fee ballot inclusion height overflows"))?;
    if !matches!(
        referendum.status,
        iroha_core::state::GovernanceReferendumStatus::Open
    ) || next_height < referendum.h_start
        || next_height > referendum.h_end
    {
        return Err(bad_request(
            "validation-fee referendum is not open at the next possible inclusion height",
        ));
    }

    if world
        .governance_locks()
        .get(&proposal_id)
        .is_some_and(|locks| locks.locks.contains_key(&request.owner))
    {
        return Err(bad_request(
            "validation-fee referendum accepts one effective ballot per account",
        ));
    }
    if !electorate.contains(&request.owner) {
        return Err(bad_request(
            "validation-fee ballot owner is not in the frozen PLAIN electorate",
        ));
    }
    let snapshot = proposal
        .parliament_snapshot
        .as_ref()
        .ok_or_else(|| inconsistent("validation-fee proposal has no Parliament snapshot"))?;
    if VALIDATION_FEE_PARLIAMENT_BODIES.into_iter().any(|body| {
        !approvals.quorum_met(body, snapshot.selection_epoch)
            || approvals.rejection_quorum_met(body, snapshot.selection_epoch)
    }) {
        return Err(inconsistent(
            "validation-fee proposal does not retain current seven-body Parliament approval",
        ));
    }

    let ballot = canonical_plain_ballot(
        &proposal_id,
        request.owner.clone(),
        request.direction,
        rules,
    );
    let amount = ballot.amount.to_string();
    let duration_blocks = ballot.duration_blocks.to_string();
    let instruction: InstructionBox = ballot.into();
    Ok(JsonBody(ValidationFeePlainBallotDraftResponseV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal_id,
        owner: request.owner,
        amount,
        duration_blocks,
        direction: request.direction,
        tx_instructions: vec![framed_instruction_draft(&instruction)?],
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn explicit_draft_window_accounts_for_next_block_and_exact_span() {
        let stale = AtWindow {
            lower: 700,
            upper: 4_299,
        };
        let error = validate_draft_referendum_window(Some(stale), 100, 600, 3_600)
            .expect_err("tip+600 omits the proposal inclusion block");
        assert!(error.contains("at least 701"));

        validate_draft_referendum_window(
            Some(AtWindow {
                lower: 701,
                upper: 4_300,
            }),
            100,
            600,
            3_600,
        )
        .expect("next-block-safe exact Taira window");

        let error = validate_draft_referendum_window(
            Some(AtWindow {
                lower: 701,
                upper: 4_299,
            }),
            100,
            600,
            3_600,
        )
        .expect_err("short referendum window must fail closed");
        assert!(error.contains("exactly 3600 blocks"));
    }

    #[test]
    fn omitted_draft_window_is_left_for_atomic_core_resolution() {
        validate_draft_referendum_window(None, u64::MAX, 600, 3_600)
            .expect("omitted lifecycle window does not precompute against a stale tip");
    }

    #[test]
    fn draft_electorate_rules_must_exactly_match_live_governance() {
        let mut gov = iroha_config::parameters::actual::Governance {
            plain_voting_enabled: true,
            min_turnout: 1,
            ..iroha_config::parameters::actual::Governance::default()
        };
        gov.citizenship_asset_id = gov.voting_asset_id.clone();
        let expected = validation_fee_plain_electorate_rules(&gov);
        assert_eq!(expected.max_members, 256);
        assert!(expected.invariant_error().is_none());
        validate_draft_plain_electorate_rules(&expected, &gov)
            .expect("exact live electorate rules");

        let mut different = expected;
        different.max_members = 255;
        validate_draft_plain_electorate_rules(&different, &gov)
            .expect_err("an otherwise valid rule set must not differ from live governance");
    }

    #[test]
    fn typed_plain_ballot_forces_retained_amount_duration_and_direction() {
        let mut gov = iroha_config::parameters::actual::Governance {
            plain_voting_enabled: true,
            min_turnout: 1,
            min_bond_amount: 150_u64.into(),
            window_span: 3_600,
            ..iroha_config::parameters::actual::Governance::default()
        };
        gov.citizenship_asset_id = gov.voting_asset_id.clone();
        let rules = validation_fee_plain_electorate_rules(&gov);
        let owner = iroha_test_samples::ALICE_ID.clone();
        for (direction, native_code) in [
            (ValidationFeePlainBallotDirectionV1::Aye, 0),
            (ValidationFeePlainBallotDirectionV1::Nay, 1),
            (ValidationFeePlainBallotDirectionV1::Abstain, 2),
        ] {
            let ballot = canonical_plain_ballot(&"11".repeat(32), owner.clone(), direction, &rules);
            assert_eq!(ballot.owner, owner);
            assert_eq!(ballot.amount, 150_u64.into());
            assert_eq!(ballot.duration_blocks, 3_600);
            assert_eq!(ballot.direction, native_code);

            let instruction: InstructionBox = ballot.clone().into();
            let draft = framed_instruction_draft(&instruction).expect("frame exact native ballot");
            let framed = hex::decode(draft.payload_hex).expect("decode framed ballot hex");
            let decoded =
                iroha_data_model::isi::decode_instruction_from_pair(&draft.wire_id, &framed)
                    .expect("decode exact native ballot");
            assert_eq!(
                decoded
                    .as_any()
                    .downcast_ref::<CastPlainBallot>()
                    .expect("typed CastPlainBallot"),
                &ballot
            );
        }
    }

    #[test]
    fn live_plain_tally_requires_exact_frozen_membership_and_lock_shape() {
        let mut gov = iroha_config::parameters::actual::Governance {
            plain_voting_enabled: true,
            min_turnout: 1,
            min_bond_amount: 150_u64.into(),
            window_span: 3_600,
            ..iroha_config::parameters::actual::Governance::default()
        };
        gov.citizenship_asset_id = gov.voting_asset_id.clone();
        gov.conviction_step_blocks = 100;
        gov.max_conviction = 6;
        let rules = validation_fee_plain_electorate_rules(&gov);
        let proposer = iroha_test_samples::ALICE_ID.clone();
        let member = iroha_test_samples::BOB_ID.clone();
        let nonmember = iroha_test_samples::CARPENTER_ID.clone();
        let capture_height = 100;
        let approval_gate_height = 50;
        let mut members = vec![
            iroha_data_model::validation_fee::ValidationFeePlainElectorateMemberV1 {
                account_id: proposer.clone(),
                bonded_height: approval_gate_height,
                bonded_amount: rules.citizenship_amount.clone(),
            },
            iroha_data_model::validation_fee::ValidationFeePlainElectorateMemberV1 {
                account_id: member.clone(),
                bonded_height: approval_gate_height + 1,
                bonded_amount: rules.citizenship_amount.clone(),
            },
        ];
        members.sort_by(|left, right| left.account_id.cmp(&right.account_id));
        let electorate = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
            [0x11; 32],
            proposer.clone(),
            capture_height,
            approval_gate_height,
            members,
        )
        .expect("canonical frozen electorate");
        let referendum_end = capture_height + rules.ballot_duration_blocks - 1;
        let lock = iroha_core::state::GovernanceLockRecord {
            owner: member.clone(),
            amount: rules.ballot_amount.clone(),
            slashed: iroha_primitives::numeric::Quantity::zero(),
            expiry_height: referendum_end,
            direction: 0,
            duration_blocks: rules.ballot_duration_blocks,
        };
        let locks = iroha_core::state::GovernanceLocksForReferendum {
            locks: BTreeMap::from([(member.clone(), lock.clone())]),
        };

        assert_eq!(
            live_plain_tally(Some(&locks), referendum_end, &rules, Some(&electorate))
                .expect("exact frozen member lock"),
            (72, 0, 0)
        );
        assert!(
            live_plain_tally(Some(&locks), referendum_end, &rules, None).is_err(),
            "locks without a frozen electorate must fail closed"
        );

        let nonmember_locks = iroha_core::state::GovernanceLocksForReferendum {
            locks: BTreeMap::from([(
                nonmember.clone(),
                iroha_core::state::GovernanceLockRecord {
                    owner: nonmember,
                    ..lock.clone()
                },
            )]),
        };
        assert!(
            live_plain_tally(
                Some(&nonmember_locks),
                referendum_end,
                &rules,
                Some(&electorate)
            )
            .is_err(),
            "a lock outside the frozen electorate must fail closed"
        );

        let expired_locks = iroha_core::state::GovernanceLocksForReferendum {
            locks: BTreeMap::from([(
                member,
                iroha_core::state::GovernanceLockRecord {
                    expiry_height: referendum_end - 1,
                    ..lock
                },
            )]),
        };
        assert!(
            live_plain_tally(
                Some(&expired_locks),
                referendum_end,
                &rules,
                Some(&electorate)
            )
            .is_err(),
            "an impossible early-expiry validation-fee lock must fail closed"
        );
    }
}

fn registry_at_height(
    current: Option<ValidationFeePolicyRegistryV1>,
    height: u64,
) -> Result<Option<ValidationFeePolicyRegistryV1>, Error> {
    let Some(mut registry) = current else {
        return Ok(None);
    };
    registry
        .validate()
        .map_err(|error| inconsistent(format!("protected registry is invalid: {error}")))?;
    registry
        .registered_policies
        .retain(|entry| entry.parliament_authorization.enacted_at_height <= height);
    if registry.registered_policies.is_empty() {
        Ok(None)
    } else {
        registry.validate().map_err(|error| {
            inconsistent(format!("historical protected registry is invalid: {error}"))
        })?;
        Ok(Some(registry))
    }
}

/// Return one bounded finality page for the current validation-fee registry.
pub(crate) async fn handler_current_policy_proof(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    NoritoOnly(request): NoritoOnly<ValidationFeeCurrentPolicyProofRequestV1>,
) -> Result<NoritoBody<ValidationFeeCurrentPolicyProofV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/policy/current/proof",
    )
    .await?;
    if request.version != VALIDATION_FEE_POLICY_PROOF_VERSION_V1
        || request.trusted_checkpoint_height == 0
    {
        return Err(bad_request(
            "validation-fee proof version or checkpoint height is invalid",
        ));
    }

    let state_view = app.state.view();
    let observed_ledger_tip_height = u64::try_from(state_view.height())
        .map_err(|_| inconsistent("ledger height does not fit the public validation-fee proof"))?;
    if request.trusted_checkpoint_height > observed_ledger_tip_height {
        return Err(bad_request(
            "trusted checkpoint is newer than the observed ledger tip",
        ));
    }
    let evaluated_height = validation_fee_policy_proof_page_tip(
        request.trusted_checkpoint_height,
        observed_ledger_tip_height,
    )
    .ok_or_else(|| bad_request("trusted checkpoint cannot begin a finality page"))?;
    let parameter_id = ValidationFeePolicyRegistryV1::parameter_id();
    let current_registry = match state_view.world().parameters().custom().get(&parameter_id) {
        None => None,
        Some(custom) => Some(
            ValidationFeePolicyRegistryV1::from_custom_parameter(custom).ok_or_else(|| {
                inconsistent("protected validation-fee registry parameter cannot be decoded")
            })?,
        ),
    };
    let registry = registry_at_height(current_registry, evaluated_height)?;
    let expected_commitment =
        ValidationFeePolicySnapshotCommitmentV1::from_registry(evaluated_height, registry.as_ref());
    drop(state_view);

    let policy_witness = app
        .kura
        .validation_fee_policy_witness_proof_v1(evaluated_height)
        .map_err(|error| {
            inconsistent(format!(
                "evaluated validation-fee witness proof is invalid: {error}"
            ))
        })?
        .ok_or_else(|| inconsistent("evaluated block has no retained policy witness proof"))?;
    if policy_witness.commitment().map_err(inconsistent)? != expected_commitment {
        return Err(inconsistent(
            "retained policy witness differs from the historical protected registry",
        ));
    }

    let proof_count = evaluated_height
        .checked_sub(request.trusted_checkpoint_height)
        .and_then(|gap| gap.checked_add(1))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| bad_request("trusted checkpoint is newer than the evaluated block"))?;
    let mut finality_chain = Vec::with_capacity(proof_count);
    for height in request.trusted_checkpoint_height..=evaluated_height {
        finality_chain.push(
            iroha_core::bridge::build_finality_proof(app.state.as_ref(), height).map_err(
                |error| {
                    inconsistent(format!(
                        "finality proof at height {height} is unavailable: {error}"
                    ))
                },
            )?,
        );
    }
    let finality_bytes = norito::to_bytes(&finality_chain)
        .map_err(|error| inconsistent(format!("finality chain cannot be encoded: {error}")))?;
    if finality_bytes.len() > VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES {
        return Err(Error::AppConflict {
            code: "validation_fee_finality_page_too_large",
            message: "The bounded finality page exceeds the response byte budget.".to_owned(),
        });
    }
    let evaluated = finality_chain
        .last()
        .ok_or_else(|| inconsistent("finality chain is empty"))?;
    let evaluated_context_id = evaluated.finality_artifact.context_id();
    let evaluated_block_hash = evaluated.finality_artifact.block_hash;
    let response = ValidationFeeCurrentPolicyProofV1 {
        version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
        registry,
        policy_witness,
        finality_chain,
        evaluated_context_id,
        evaluated_block_height: evaluated_height,
        evaluated_block_hash: hex::encode(evaluated_block_hash.as_ref()),
        observed_ledger_tip_height,
        more_available: evaluated_height < observed_ledger_tip_height,
    };
    let response_bytes = norito::to_bytes(&response)
        .map_err(|error| inconsistent(format!("policy proof cannot be encoded: {error}")))?;
    if response_bytes.len() > VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES {
        return Err(Error::AppConflict {
            code: "validation_fee_policy_proof_too_large",
            message: "The validation-fee proof exceeds the response byte budget.".to_owned(),
        });
    }
    Ok(NoritoBody(response))
}
