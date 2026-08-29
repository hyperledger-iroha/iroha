//! Typed Torii surface for Parliament-governed validation-fee state.
use crate::{
    Error, JsonBody, NoritoBody, NoritoJson, NoritoQuery, SharedAppState, check_access,
    require_runtime_governance_account, utils::extractors::NoritoOnly,
};
use axum::{
    extract::{ConnectInfo, Extension, Path, State},
    http::HeaderMap,
};
use iroha_core::state::{StateReadOnly as _, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    governance::types::{
        GovernanceAttemptStatusV1, GovernanceCertificateId, GovernanceCertificateV1,
        ProposalContentId, ProposalKind,
    },
    hijiri::{HijiriAccountRiskV1, HijiriParametersV1},
    isi::{
        InstructionBox,
        governance::{ProposeValidationFeePayoutLifecycle, ProposeValidationFeePolicy},
    },
    validation_fee::{
        ValidationFeeChargingMode, ValidationFeePolicyRegistryV1,
        ValidationFeePolicySnapshotCommitmentV1,
    },
};
use iroha_torii_shared::validation_fee_api::{
    VALIDATION_FEE_BASE_MINOR_UNITS_V1, VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES,
    VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES, VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
    VALIDATION_FEE_PROPOSAL_API_VERSION_V1, VALIDATION_FEE_PROPOSAL_PAGE_MAX_LIMIT_V1,
    ValidationFeeCurrentPolicyProofRequestV1, ValidationFeeCurrentPolicyProofV1,
    ValidationFeeHijiriQuoteBaseV1, ValidationFeeHijiriQuoteRequestV1,
    ValidationFeeHijiriQuoteResponseV1, ValidationFeeProposalDetailQueryV1,
    ValidationFeeProposalDetailV1, ValidationFeeProposalDraftPayloadV1,
    ValidationFeeProposalDraftRequestV1, ValidationFeeProposalDraftResponseV1,
    ValidationFeeProposalInstructionDraftV1, ValidationFeeProposalListQueryV1,
    ValidationFeeProposalListV1, ValidationFeeProposalRecordV1, ValidationFeeProposalStatusV1,
    decode_validation_fee_proposal_cursor_v1, encode_validation_fee_proposal_cursor_v1,
    evaluate_hijiri_quote_v1, validation_fee_hijiri_quote_execution_height,
    validation_fee_policy_proof_page_tip,
};
use mv::storage::StorageReadOnly as _;
use std::ops::Bound::{Excluded, Unbounded};
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
fn quote_unavailable(message: impl Into<String>) -> Error {
    Error::AppConflict {
        code: "validation_fee_hijiri_quote_unavailable",
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
fn retained_proposal_operator(proposal_kind: &ProposalKind) -> Result<&AccountId, Error> {
    match proposal_kind {
        ProposalKind::ValidationFeePolicy(payload) => Ok(&payload.proposal_operator),
        ProposalKind::ValidationFeePayoutLifecycle(payload) => Ok(&payload.proposal_operator),
        ProposalKind::DeployContract(_)
        | ProposalKind::RuntimeUpgrade(_)
        | ProposalKind::SccpRouteGovernance(_)
        | ProposalKind::SorafsProviderGovernance(_)
        | ProposalKind::MusubiRegistryGovernance(_)
        | ProposalKind::ContractLifecycleGovernance(_)
        | ProposalKind::ContractEmergencyHold(_) => Err(inconsistent(
            "non-validation-fee proposal reached the typed validation-fee projection",
        )),
    }
}
fn public_proposal_record(
    world: &impl WorldReadOnly,
    registry: Option<&ValidationFeePolicyRegistryV1>,
    proposal_id: [u8; 32],
    proposal: &iroha_core::state::GovernanceProposalRecord,
) -> Result<
    (
        ValidationFeeProposalRecordV1,
        Option<GovernanceCertificateV1>,
    ),
    Error,
> {
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
    if retained_proposal_operator(&proposal.kind)? != &proposal.proposer {
        return Err(inconsistent(
            "validation-fee proposal operator differs from its retained proposer",
        ));
    }
    let proposal_content_id = ProposalContentId::new(proposal_id);
    let mut attempts = world
        .parliament_attempts()
        .iter()
        .filter(|(_, attempt)| attempt.proposal_content_id() == proposal_content_id)
        .map(|(_, attempt)| attempt)
        .collect::<Vec<_>>();
    attempts.sort_unstable_by_key(|attempt| attempt.attempt().sequence);
    for (expected_sequence, attempt) in attempts.iter().enumerate() {
        let expected_sequence = u32::try_from(expected_sequence)
            .map_err(|_| inconsistent("validation-fee proposal attempt sequence exceeds u32"))?;
        if attempt.attempt().sequence != expected_sequence {
            return Err(inconsistent(
                "validation-fee proposal attempt history is not an exact contiguous sequence",
            ));
        }
        attempt.validate().map_err(|error| {
            inconsistent(format!(
                "validation-fee proposal retained an invalid Parliament attempt: {error}"
            ))
        })?;
        attempt
            .validate_proposal_bindings_v1(&proposal.kind)
            .map_err(|error| {
                inconsistent(format!(
                    "validation-fee proposal retained a Parliament attempt with mismatched proposal bindings: {error}"
                ))
            })?;
    }
    let latest_attempt = attempts.last().copied();
    let expected_status = latest_attempt.map_or(
        iroha_core::state::GovernanceProposalStatus::Proposed,
        |attempt| match attempt.attempt().status {
            GovernanceAttemptStatusV1::Active | GovernanceAttemptStatusV1::Certified => {
                iroha_core::state::GovernanceProposalStatus::Proposed
            }
            GovernanceAttemptStatusV1::Rejected => {
                iroha_core::state::GovernanceProposalStatus::Rejected
            }
            GovernanceAttemptStatusV1::Enacted => {
                iroha_core::state::GovernanceProposalStatus::Enacted
            }
            GovernanceAttemptStatusV1::Superseded => {
                iroha_core::state::GovernanceProposalStatus::Superseded
            }
            GovernanceAttemptStatusV1::ExecutionFailed => {
                iroha_core::state::GovernanceProposalStatus::ExecutionFailed
            }
        },
    );
    if proposal.status != expected_status {
        return Err(inconsistent(
            "validation-fee proposal status differs from its latest Parliament attempt",
        ));
    }
    let certificate = latest_attempt.and_then(|attempt| attempt.certificate().cloned());
    let authorization = retained_registry_authorization(registry, proposal_id)?;
    if proposal.status == iroha_core::state::GovernanceProposalStatus::Enacted {
        let enacted_attempt_count = attempts
            .iter()
            .filter(|attempt| attempt.attempt().status == GovernanceAttemptStatusV1::Enacted)
            .count();
        let attempt = latest_attempt.ok_or_else(|| {
            inconsistent("enacted validation-fee proposal has no Parliament attempt")
        })?;
        let certificate = certificate.as_ref().ok_or_else(|| {
            inconsistent("enacted validation-fee proposal has no Parliament certificate")
        })?;
        let authorization = authorization.ok_or_else(|| {
            inconsistent("enacted validation-fee proposal has no protected registry authorization")
        })?;
        if enacted_attempt_count != 1
            || attempt.terminal_height() != Some(certificate.enact_at_height)
            || authorization.invariant_error().is_some()
            || authorization.proposal_fingerprint != proposal_id
            || authorization.proposal_operator != proposal.proposer
            || authorization.governance_certificate_id
                != GovernanceCertificateId::derive_v1(certificate)
            || &authorization.governance_certificate != certificate
            || authorization.enacted_at_height != certificate.enact_at_height
        {
            return Err(inconsistent(
                "protected validation-fee authorization differs from the unique enacted Parliament attempt",
            ));
        }
    } else if authorization.is_some() {
        return Err(inconsistent(
            "unenacted validation-fee proposal unexpectedly has protected registry authorization",
        ));
    }
    let status = match proposal.status {
        iroha_core::state::GovernanceProposalStatus::Proposed => {
            ValidationFeeProposalStatusV1::Proposed
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
        iroha_core::state::GovernanceProposalStatus::ExecutionFailed => {
            ValidationFeeProposalStatusV1::ExecutionFailed
        }
    };
    Ok((
        ValidationFeeProposalRecordV1 {
            proposer: proposal.proposer.clone(),
            kind: proposal.kind.clone(),
            created_height: proposal.created_height,
            status,
        },
        certificate,
    ))
}
fn current_validation_fee_registry(
    world: &impl WorldReadOnly,
) -> Result<Option<ValidationFeePolicyRegistryV1>, Error> {
    let parameter_id = ValidationFeePolicyRegistryV1::parameter_id();
    let Some(custom) = world.parameters().custom().get(&parameter_id) else {
        return Ok(None);
    };
    let registry = ValidationFeePolicyRegistryV1::from_custom_parameter(custom)
        .ok_or_else(|| inconsistent("protected validation-fee registry cannot be decoded"))?;
    registry
        .validate()
        .map_err(|error| inconsistent(format!("protected registry is invalid: {error}")))?;
    Ok(Some(registry))
}
fn retained_registry_authorization<'a>(
    registry: Option<&'a ValidationFeePolicyRegistryV1>,
    proposal_id: [u8; 32],
) -> Result<
    Option<&'a iroha_data_model::validation_fee::ValidationFeeParliamentAuthorizationV1>,
    Error,
> {
    let Some(registry) = registry else {
        return Ok(None);
    };
    let mut found = None;
    for entry in &registry.registered_policies {
        let authorizations = std::iter::once(&entry.parliament_authorization).chain(
            entry
                .payout_lifecycle
                .iter()
                .map(|reference| &reference.parliament_authorization),
        );
        for authorization in authorizations {
            if authorization.proposal_fingerprint == proposal_id {
                if found.replace(authorization).is_some() {
                    return Err(inconsistent(
                        "protected registry contains duplicate proposal certificate bindings",
                    ));
                }
            }
        }
    }
    Ok(found)
}
fn bounded_validation_fee_proposal_keys<'a>(
    indexed: impl Iterator<Item = (&'a (u64, [u8; 32]), &'a ())>,
    limit: usize,
) -> (Vec<(u64, [u8; 32])>, bool) {
    let mut keys = indexed
        .take(limit.saturating_add(1))
        .map(|(key, ())| *key)
        .collect::<Vec<_>>();
    let has_more = keys.len() > limit;
    if has_more {
        keys.pop();
    }
    (keys, has_more)
}
/// Return one bounded page of typed validation-fee Parliament proposals.
pub(crate) async fn handler_proposals(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    NoritoQuery(query): NoritoQuery<ValidationFeeProposalListQueryV1>,
) -> Result<JsonBody<ValidationFeeProposalListV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/proposals",
    )
    .await?;
    if query.limit == 0 || query.limit > VALIDATION_FEE_PROPOSAL_PAGE_MAX_LIMIT_V1 {
        return Err(bad_request(format!(
            "limit must be between 1 and {VALIDATION_FEE_PROPOSAL_PAGE_MAX_LIMIT_V1}"
        )));
    }
    let after = query
        .cursor
        .as_deref()
        .map(decode_validation_fee_proposal_cursor_v1)
        .transpose()
        .map_err(bad_request)?;
    let world = app.state.world_view();
    let registry = current_validation_fee_registry(&world)?;
    let index = world.validation_fee_proposal_index();
    let indexed: Box<dyn Iterator<Item = (&(u64, [u8; 32]), &())> + '_> = match after {
        Some(after) => Box::new(index.range((Excluded(after), Unbounded))),
        None => Box::new(index.iter()),
    };
    let limit = usize::try_from(query.limit).expect("bounded u32 page limit fits usize");
    let (page_keys, has_more) = bounded_validation_fee_proposal_keys(indexed, limit);
    let mut proposals = Vec::with_capacity(limit);
    let mut last_key = None;
    for (created_height, proposal_id) in page_keys {
        let proposal = world
            .governance_proposals()
            .get(&proposal_id)
            .ok_or_else(|| inconsistent("validation-fee proposal index references no proposal"))?;
        if proposal.created_height != created_height
            || !matches!(
                proposal.kind,
                ProposalKind::ValidationFeePolicy(_)
                    | ProposalKind::ValidationFeePayoutLifecycle(_)
            )
        {
            return Err(inconsistent(
                "validation-fee proposal index does not match its exact typed proposal",
            ));
        }
        let (record, _) = public_proposal_record(&world, registry.as_ref(), proposal_id, proposal)?;
        proposals.push(record);
        last_key = Some((created_height, proposal_id));
    }
    let next_cursor = if has_more {
        last_key.map(|(created_height, proposal_id)| {
            encode_validation_fee_proposal_cursor_v1(created_height, proposal_id)
        })
    } else {
        None
    };
    Ok(JsonBody(ValidationFeeProposalListV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        limit: query.limit,
        proposals,
        next_cursor,
    }))
}
/// Return one typed validation-fee Parliament proposal.
pub(crate) async fn handler_proposal_detail(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    Path(proposal_id): Path<String>,
    NoritoQuery(_query): NoritoQuery<ValidationFeeProposalDetailQueryV1>,
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
    let registry = current_validation_fee_registry(&world)?;
    let (proposal, governance_certificate) =
        public_proposal_record(&world, registry.as_ref(), proposal_id_bytes, proposal)?;
    Ok(JsonBody(ValidationFeeProposalDetailV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal,
        current_height: u64::try_from(app.state.committed_height())
            .unwrap_or(u64::MAX)
            .to_string(),
        governance_certificate,
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
    let proposal_kind = request.proposal.proposal_kind(&request.proposal_operator);
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
            }
            .into()
        }
    };
    Ok((proposal_kind, instruction))
}
fn framed_instruction_draft(
    instruction: &InstructionBox,
) -> Result<ValidationFeeProposalInstructionDraftV1, Error> {
    let (wire_id, framed) = iroha_data_model::isi::framed_instruction_payload(instruction)
        .ok_or_else(|| inconsistent("native validation-fee instruction has no V1 wire ID"))?;
    Ok(ValidationFeeProposalInstructionDraftV1 {
        wire_id: wire_id.to_owned(),
        payload_hex: hex::encode(framed),
    })
}
/// Build one exact native validation-fee proposal instruction for local signing.
pub(crate) async fn handler_proposal_draft(
    State(app): State<SharedAppState>,
    Extension(verified): Extension<crate::app_auth::VerifiedCanonicalRequest>,
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
    require_runtime_governance_account(
        &request.proposal_operator,
        &verified.account,
        "validation-fee proposal draft",
    )?;
    let (proposal_kind, instruction) = canonical_draft_instruction(&request)?;
    let proposal_id = proposal_kind.fingerprint();
    let instruction = framed_instruction_draft(&instruction)?;
    Ok(JsonBody(ValidationFeeProposalDraftResponseV1 {
        version: VALIDATION_FEE_PROPOSAL_API_VERSION_V1,
        proposal_operator: request.proposal_operator,
        proposal_id: hex::encode(proposal_id),
        proposal_kind,
        tx_instructions: vec![instruction],
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn proposal_page_traversal_reads_only_limit_plus_one_index_rows() {
        let rows = (0_u64..10_000)
            .map(|created_height| {
                let mut proposal_id = [0_u8; 32];
                proposal_id[..8].copy_from_slice(&created_height.to_be_bytes());
                ((created_height, proposal_id), ())
            })
            .collect::<Vec<_>>();
        let visited = Cell::new(0_usize);
        let indexed = rows
            .iter()
            .inspect(|_| visited.set(visited.get().saturating_add(1)))
            .map(|(key, value)| (key, value));
        let (keys, has_more) = bounded_validation_fee_proposal_keys(indexed, 3);
        assert_eq!(visited.get(), 4, "one lookahead row is the exact bound");
        assert_eq!(keys.len(), 3);
        assert!(has_more);
        assert_eq!(keys[0].0, 0);
        assert_eq!(keys[2].0, 2);
    }
    #[test]
    fn proposal_list_cannot_reintroduce_a_full_governance_scan() {
        let source = include_str!("validation_fee_api.rs");
        let start = source
            .find("fn bounded_validation_fee_proposal_keys")
            .expect("bounded proposal-key projection");
        let tail = &source[start..];
        let end = tail
            .find("/// Return one typed validation-fee Parliament proposal.")
            .expect("proposal-list handler terminator");
        let implementation = &tail[..end];
        assert!(implementation.contains("validation_fee_proposal_index()"));
        assert!(implementation.contains("bounded_validation_fee_proposal_keys(indexed, limit)"));
        assert!(!implementation.contains("governance_proposals().iter()"));
        assert!(!implementation.contains(".sort"));
    }
    #[test]
    fn hijiri_quote_selects_the_policy_at_the_checked_next_height() {
        let activation_height = 1_000_u64;
        let evaluated_state_height = activation_height - 1;
        assert_eq!(
            validation_fee_hijiri_quote_execution_height(evaluated_state_height),
            Ok(activation_height)
        );
        assert!(validation_fee_hijiri_quote_execution_height(u64::MAX).is_err());

        let source = include_str!("validation_fee_api.rs");
        let start = source
            .rfind("pub(crate) async fn handler_hijiri_quote")
            .expect("Hijiri quote handler");
        let implementation = &source[start..];
        let end = implementation
            .find("/// Return one bounded finality page")
            .expect("Hijiri quote handler terminator");
        let implementation = &implementation[..end];
        assert!(implementation.contains("validation_fee_hijiri_quote_execution_height"));
        assert!(implementation.contains("effective_entry_at_height(quoted_execution_height)"));
        assert!(!implementation.contains("effective_entry_at_height(evaluated_state_height)"));
        assert!(implementation.contains("policy_entry.policy.network_id"));
        assert!(implementation.contains("*app.state.network_id_ref()"));
        assert!(implementation.contains("is_live_multisig_signatory_in_world"));
        assert!(implementation.contains("validation_fee_hijiri_quote_account_mismatch"));
        assert!(implementation.contains("let state_view = app.state.view()"));
        assert!(implementation.contains("state_view.world()"));
        assert!(implementation.contains("account-risk parameter changed its reserved identity"));
        assert!(!implementation.contains(".flatten()"));
        let authorization = implementation
            .find("if !is_authorized")
            .expect("member-aware quote authorization");
        let risk_lookup = implementation
            .find("let account_risk_parameter_id")
            .expect("account-risk lookup");
        assert!(
            authorization < risk_lookup,
            "cross-account authorization must precede account-risk key derivation and lookup"
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
/// Return one same-snapshot, evaluated-only Hijiri validation-fee quote.
pub(crate) async fn handler_hijiri_quote(
    State(app): State<SharedAppState>,
    Extension(verified): Extension<crate::app_auth::VerifiedCanonicalRequest>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    NoritoOnly(request): NoritoOnly<ValidationFeeHijiriQuoteRequestV1>,
) -> Result<NoritoBody<ValidationFeeHijiriQuoteResponseV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/validation-fee/hijiri/quote",
    )
    .await?;
    request.validate().map_err(bad_request)?;
    let state_view = app.state.view();
    let is_authorized = request.account_id == verified.account
        || match crate::routing::is_live_multisig_signatory_in_world(
            state_view.world(),
            &request.account_id,
            &verified.account,
        ) {
            Ok(is_signatory) => is_signatory,
            Err(error) => {
                iroha_logger::warn!(
                    requested_account = %request.account_id,
                    authenticated_account = %verified.account,
                    ?error,
                    "cross-account Hijiri quote authorization could not validate a live multisig controller"
                );
                false
            }
        };
    if !is_authorized {
        return Err(Error::AppForbidden {
            code: "validation_fee_hijiri_quote_account_mismatch",
            message: "authenticated account must equal the requested Hijiri quote account or be a direct signatory of that live multisig controller".to_owned(),
        });
    }

    let evaluated_state_height = u64::try_from(state_view.height())
        .map_err(|_| inconsistent("ledger height does not fit the public Hijiri fee quote"))?;
    let quoted_execution_height =
        validation_fee_hijiri_quote_execution_height(evaluated_state_height)
            .map_err(inconsistent)?;
    let custom_parameters = state_view.world().parameters().custom();

    let registry_parameter = custom_parameters
        .get(&ValidationFeePolicyRegistryV1::parameter_id())
        .ok_or_else(|| quote_unavailable("validation-fee policy registry is not configured"))?;
    let registry = ValidationFeePolicyRegistryV1::from_custom_parameter(registry_parameter)
        .ok_or_else(|| inconsistent("protected validation-fee registry cannot be decoded"))?;
    registry
        .validate()
        .map_err(|error| inconsistent(format!("protected registry is invalid: {error}")))?;
    let policy_entry = registry
        .effective_entry_at_height(quoted_execution_height)
        .ok_or_else(|| {
            quote_unavailable("no validation-fee policy is active at the quoted execution height")
        })?;
    if policy_entry.policy.network_id != *app.state.network_id_ref() {
        return Err(inconsistent(
            "active validation-fee policy targets a different exact network",
        ));
    }
    if policy_entry.policy.charging_mode == ValidationFeeChargingMode::Disabled {
        return Err(quote_unavailable(
            "validation-fee charging is disabled at the quoted execution height",
        ));
    }

    let hijiri_parameter = custom_parameters
        .get(&HijiriParametersV1::parameter_id())
        .ok_or_else(|| quote_unavailable("global Hijiri fee parameters are not configured"))?;
    let parameters = HijiriParametersV1::from_custom_parameter(hijiri_parameter)
        .map_err(|error| inconsistent(format!("global Hijiri parameter is invalid: {error}")))?
        .ok_or_else(|| inconsistent("global Hijiri parameter changed its reserved identity"))?;

    let account_risk_parameter_id = HijiriAccountRiskV1::parameter_id_for(&request.account_id)
        .map_err(|error| {
            inconsistent(format!(
                "Hijiri account-risk parameter id cannot be derived: {error}"
            ))
        })?;
    let account_risk = match custom_parameters.get(&account_risk_parameter_id) {
        None => None,
        Some(parameter) => Some(
            HijiriAccountRiskV1::from_custom_parameter(parameter)
                .map_err(|error| {
                    inconsistent(format!("Hijiri account-risk parameter is invalid: {error}"))
                })?
                .ok_or_else(|| {
                    inconsistent("Hijiri account-risk parameter changed its reserved identity")
                })?,
        ),
    };

    let base = ValidationFeeHijiriQuoteBaseV1::try_new(
        evaluated_state_height,
        quoted_execution_height,
        policy_entry.policy.policy_version,
        policy_entry.policy_hash,
        policy_entry.policy.ds_asset_id.to_string(),
        policy_entry.policy.treasury_account_id.to_string(),
        policy_entry.policy.ds_scale,
        VALIDATION_FEE_BASE_MINOR_UNITS_V1,
    )
    .map_err(|error| {
        inconsistent(format!(
            "active validation-fee quote base is invalid: {error}"
        ))
    })?;
    let response = evaluate_hijiri_quote_v1(
        base,
        &request.account_id,
        &parameters,
        account_risk.as_ref(),
        request.qualifying_transfer_count,
    )
    .map_err(|error| inconsistent(format!("Hijiri validation-fee quote failed: {error}")))?;
    Ok(NoritoBody(response))
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
    let finality_encoded_bytes = norito::core::encoded_frame_len(&finality_chain)
        .map_err(|error| inconsistent(format!("finality chain cannot be encoded: {error}")))?;
    if finality_encoded_bytes > VALIDATION_FEE_POLICY_PROOF_MAX_FINALITY_CHAIN_BYTES {
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
    let response_encoded_bytes = norito::core::encoded_frame_len(&response)
        .map_err(|error| inconsistent(format!("policy proof cannot be encoded: {error}")))?;
    if response_encoded_bytes > VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES {
        return Err(Error::AppConflict {
            code: "validation_fee_policy_proof_too_large",
            message: "The validation-fee proof exceeds the response byte budget.".to_owned(),
        });
    }
    Ok(NoritoBody(response))
}
