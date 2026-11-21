//! Account-authenticated drafts for the anonymous Parliament family's three V1 instructions.
//! A registration contains the public registration proof. Casting uses a separate signerless route.

use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
};
use iroha_data_model::{
    governance::{
        anonymous_ballot::{
            CreateAnonymousParliamentAttemptV1, RegisterParliamentCredentialV1,
            SubmitAnonymousParliamentTransitionV1,
        },
        types::MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1,
    },
    isi::{InstructionBox, framed_instruction_payload},
};
use iroha_torii_shared::{
    parliament_api::ParliamentInstructionDraftV1, route_catalog::runtime_governance,
};
use std::net::SocketAddr;

use crate::{Error, JsonBody, NoritoJson, SharedAppState, check_access, routing::conversion_error};

fn frame(instruction: InstructionBox) -> Result<JsonBody<ParliamentInstructionDraftV1>, Error> {
    let (wire_id, bytes) = framed_instruction_payload(&instruction)
        .ok_or_else(|| conversion_error("unregistered Parliament instruction".to_owned()))?;
    Ok(JsonBody(ParliamentInstructionDraftV1 {
        wire_id: wire_id.to_owned(),
        payload_hex: hex::encode(bytes),
    }))
}

/// Create one retryable attempt for exact proposal content.
pub(crate) async fn attempt(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    NoritoJson(instruction): NoritoJson<CreateAnonymousParliamentAttemptV1>,
) -> Result<JsonBody<ParliamentInstructionDraftV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        runtime_governance::GOV_ANONYMOUS_PARLIAMENT_ATTEMPT_DRAFT
            .path()
            .trim_start_matches('/'),
    )
    .await?;
    if instruction.attempt_sequence > MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1 {
        return Err(conversion_error(
            "Parliament attempt sequence exceeds the V1 retry limit".to_owned(),
        ));
    }
    if let Some(reason) = instruction
        .proposal
        .first_release_exact_json_u64_invariant_error()
    {
        return Err(conversion_error(reason.to_owned()));
    }
    frame(instruction.into())
}

/// Draft an account-signed transition against an existing attempt.
pub(crate) async fn transition(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    NoritoJson(instruction): NoritoJson<SubmitAnonymousParliamentTransitionV1>,
) -> Result<JsonBody<ParliamentInstructionDraftV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        runtime_governance::GOV_ANONYMOUS_PARLIAMENT_TRANSITION_DRAFT
            .path()
            .trim_start_matches('/'),
    )
    .await?;
    instruction
        .validate_static()
        .map_err(|reason| conversion_error(reason.to_owned()))?;
    frame(instruction.into())
}

/// Draft a proof-carrying credential registration for a binding seat.
pub(crate) async fn credential(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    NoritoJson(instruction): NoritoJson<RegisterParliamentCredentialV1>,
) -> Result<JsonBody<ParliamentInstructionDraftV1>, Error> {
    check_access(
        &app,
        &headers,
        Some(remote.ip()),
        runtime_governance::GOV_ANONYMOUS_PARLIAMENT_CREDENTIAL_DRAFT
            .path()
            .trim_start_matches('/'),
    )
    .await?;
    if instruction.governance_attempt_id.as_bytes() == &[0; 32]
        || instruction.election_attempt_id.as_bytes() == &[0; 32]
    {
        return Err(conversion_error(
            "governance and election attempt identifiers must be nonzero".to_owned(),
        ));
    }
    frame(instruction.into())
}
