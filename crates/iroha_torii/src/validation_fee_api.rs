//! Typed Torii surface for Parliament-governed validation-fee state.

use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
};
use iroha_core::state::{StateReadOnly as _, WorldReadOnly as _};
use iroha_data_model::{
    governance::types::{AtWindow, ProposalKind},
    isi::{
        InstructionBox, frame_instruction_payload,
        governance::{ProposeValidationFeePayoutLifecycle, ProposeValidationFeePolicy, VotingMode},
    },
    validation_fee::{ValidationFeePolicyRegistryV1, ValidationFeePolicySnapshotCommitmentV1},
};
use iroha_torii_shared::validation_fee_api::{
    VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES,
    VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES, VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
    VALIDATION_FEE_PROPOSAL_API_VERSION_V1, ValidationFeeCurrentPolicyProofRequestV1,
    ValidationFeeCurrentPolicyProofV1, ValidationFeeParliamentSnapshotV1,
    ValidationFeeProposalDetailV1, ValidationFeeProposalDraftPayloadV1,
    ValidationFeeProposalDraftRequestV1, ValidationFeeProposalDraftResponseV1,
    ValidationFeeProposalInstructionDraftV1, ValidationFeeProposalListV1,
    ValidationFeeProposalRecordV1, ValidationFeeProposalReferendumV1,
    ValidationFeeProposalStatusV1, validation_fee_policy_proof_page_tip,
};
use mv::storage::StorageReadOnly as _;

use crate::{
    Error, JsonBody, NoritoBody, NoritoJson, SharedAppState, check_access,
    utils::extractors::NoritoOnly,
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

fn public_proposal_record(
    proposal_id: [u8; 32],
    proposal: &iroha_core::state::GovernanceProposalRecord,
    referendum: iroha_core::state::GovernanceReferendumRecord,
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
        created_height: proposal.created_height,
        status,
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
            selection_epoch: snapshot.selection_epoch,
            beacon: snapshot.beacon,
            roster_root: snapshot.roster_root,
            bodies: snapshot.bodies.clone(),
        },
        finalization_evidence: proposal.finalization_evidence,
        enacted_at_height: proposal.enacted_at_height,
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
        proposals.push(public_proposal_record(*proposal_id, proposal, referendum)?);
    }
    proposals.sort_by(|left, right| {
        left.created_height
            .cmp(&right.created_height)
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
    let referendum = world
        .governance_referenda()
        .get(&proposal_id)
        .copied()
        .ok_or_else(|| inconsistent("validation-fee proposal has no exact retained referendum"))?;
    Ok(JsonBody(ValidationFeeProposalDetailV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal: public_proposal_record(proposal_id_bytes, proposal, referendum)?,
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
    let proposal_kind = request.proposal.proposal_kind();
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
            }
            .into()
        }
    };
    Ok((proposal_kind, instruction))
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
    let proposal_id = proposal_kind.fingerprint();
    let wire_id = iroha_data_model::isi::Instruction::id(&*instruction).to_string();
    let payload = iroha_data_model::isi::Instruction::dyn_encode(&*instruction);
    let framed = frame_instruction_payload(&wire_id, &payload).map_err(|error| {
        inconsistent(format!(
            "failed to frame native validation-fee proposal instruction: {error}"
        ))
    })?;
    Ok(JsonBody(ValidationFeeProposalDraftResponseV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal_id: hex::encode(proposal_id),
        proposal_kind,
        tx_instructions: vec![ValidationFeeProposalInstructionDraftV1 {
            wire_id,
            payload_hex: hex::encode(framed),
        }],
    }))
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
