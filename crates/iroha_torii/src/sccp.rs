//! SCCP v1 public read API (`specs/sccp.md` §6).
//!
//! Every honest peer serves identical bytes derived only from committed state; wallets and
//! destinations never trust them and verify every bundle themselves. Responses are Norito or
//! JSON by `Accept`. Proof routes answer `409 sccp_attestation_pending` until the chosen
//! subject is attested.
//!
//! TODO(ws35): attestation, history, bridge-key and governance views, `ETag` headers on proof
//! bundles, and the Kura fallback for pruned signatures.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse as _, Response},
};
use iroha_core::{
    smartcontracts::isi::sccp::{
        read::{self, SccpAttestationChoiceV1, SccpReadError},
        store,
    },
    state::StateReadOnly,
};
use iroha_data_model::bridge::SccpNetworkV1;
use norito::json::{Map, Value};

use crate::{JsonBody, NoritoQuery, SharedAppState, utils::extractors::ExtractAccept};

/// `?attestation=own|latest|<height>` of the proof routes (default `own`).
#[derive(Debug, Default, Clone, crate::json_macros::JsonDeserialize)]
pub(crate) struct AttestationQuery {
    /// Which subject attests the leaf.
    #[norito(default)]
    pub attestation: Option<String>,
}

/// `?after_generation=G&limit=N` of the rotation-chain route (decimal strings).
#[derive(Debug, Default, Clone, crate::json_macros::JsonDeserialize)]
pub(crate) struct RotationQuery {
    /// Generation to rotate from (default 1).
    #[norito(default)]
    pub after_generation: Option<String>,
    /// Page size, at most 16.
    #[norito(default)]
    pub limit: Option<String>,
}

fn parse_number<T: core::str::FromStr>(text: Option<&str>, default: T) -> Result<T, Response> {
    text.map_or(Ok(default), |text| {
        text.parse().map_err(|_| {
            error(
                StatusCode::BAD_REQUEST,
                "sccp_invalid_query",
                format!("`{text}` is not a decimal number"),
            )
        })
    })
}

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    let mut body = Map::new();
    body.insert("code".into(), Value::from(code.to_owned()));
    body.insert("message".into(), Value::from(message.into()));
    (status, JsonBody(Value::Object(body))).into_response()
}

fn read_error(read: &SccpReadError) -> Response {
    match read {
        SccpReadError::NotFound(_) => {
            error(StatusCode::NOT_FOUND, "sccp_not_found", read.to_string())
        }
        SccpReadError::Pending(_) => error(
            StatusCode::CONFLICT,
            "sccp_attestation_pending",
            read.to_string(),
        ),
        SccpReadError::Invalid(_) => error(
            StatusCode::BAD_REQUEST,
            "sccp_invalid_query",
            read.to_string(),
        ),
    }
}

fn format(accept: Option<&ExtractAccept>) -> Result<crate::utils::ResponseFormat, Response> {
    crate::utils::negotiate_response_format(accept.map(|value| &value.0))
}

fn respond<T>(accept: Option<&ExtractAccept>, value: T) -> Response
where
    T: norito::json::JsonSerialize + norito::core::NoritoSerialize + 'static,
{
    match format(accept) {
        Ok(format) => crate::utils::respond_with_format(value, format),
        Err(response) => response,
    }
}

fn parse_word(text: &str) -> Result<[u8; 32], Response> {
    let bytes = hex::decode(text.strip_prefix("0x").unwrap_or(text))
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok());
    bytes.ok_or_else(|| {
        error(
            StatusCode::BAD_REQUEST,
            "sccp_invalid_query",
            "expected a 32-byte hex identifier",
        )
    })
}

fn parse_network(text: &str) -> Result<SccpNetworkV1, Response> {
    SccpNetworkV1::from_profile_key(text)
        .filter(|network| network.is_external())
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "sccp_invalid_query",
                format!("`{text}` is not an external SCCP network"),
            )
        })
}

fn parse_choice(query: &AttestationQuery) -> Result<SccpAttestationChoiceV1, Response> {
    query
        .attestation
        .as_deref()
        .map_or(Ok(SccpAttestationChoiceV1::Own), |text| {
            SccpAttestationChoiceV1::parse(text).ok_or_else(|| {
                error(
                    StatusCode::BAD_REQUEST,
                    "sccp_invalid_query",
                    "attestation must be `own`, `latest` or a height",
                )
            })
        })
}

/// `GET /v1/sccp/capabilities`.
pub(crate) async fn handler_capabilities(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let capabilities = read::capabilities(&app.state.view());
    respond(accept.as_ref(), capabilities)
}

/// `GET /v1/sccp/registry`: every route with its escrow, stranded balance and revisions.
pub(crate) async fn handler_registry(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let routes: Vec<_> = {
        let view = app.state.view();
        store::routes::iter(view.world())
            .map(|(_, route)| route.clone())
            .collect()
    };
    respond(accept.as_ref(), routes)
}

/// `GET /v1/sccp/messages/{message_id}`: the outbound record.
pub(crate) async fn handler_message(
    State(app): State<SharedAppState>,
    Path(message_id): Path<String>,
    accept: Option<ExtractAccept>,
) -> Response {
    let message_id = match parse_word(&message_id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let record = {
        let view = app.state.view();
        store::outbound_messages::get(view.world(), &message_id).cloned()
    };
    match record {
        Some(record) => respond(accept.as_ref(), record),
        None => read_error(&SccpReadError::NotFound("outbound message".into())),
    }
}

/// `GET /v1/sccp/messages/{message_id}/proof?attestation=`.
pub(crate) async fn handler_message_proof(
    State(app): State<SharedAppState>,
    Path(message_id): Path<String>,
    NoritoQuery(query): NoritoQuery<AttestationQuery>,
    accept: Option<ExtractAccept>,
) -> Response {
    let (message_id, choice) = match (parse_word(&message_id), parse_choice(&query)) {
        (Ok(id), Ok(choice)) => (id, choice),
        (Err(response), _) | (_, Err(response)) => return response,
    };
    let bundle = read::message_proof(&app.state.view(), &message_id, choice);
    match bundle {
        Ok(bundle) => respond(accept.as_ref(), bundle),
        Err(failure) => read_error(&failure),
    }
}

/// `GET /v1/sccp/controls/{network}/{revision}/{control_nonce}/proof?attestation=`.
pub(crate) async fn handler_control_proof(
    State(app): State<SharedAppState>,
    Path((network, revision, control_nonce)): Path<(String, u32, u64)>,
    NoritoQuery(query): NoritoQuery<AttestationQuery>,
    accept: Option<ExtractAccept>,
) -> Response {
    let (network, choice) = match (parse_network(&network), parse_choice(&query)) {
        (Ok(network), Ok(choice)) => (network, choice),
        (Err(response), _) | (_, Err(response)) => return response,
    };
    let bundle = read::control_proof(&app.state.view(), network, revision, control_nonce, choice);
    match bundle {
        Ok(bundle) => respond(accept.as_ref(), bundle),
        Err(failure) => read_error(&failure),
    }
}

/// `GET /v1/sccp/light-clients`: every installed light client with its head and freeze state.
pub(crate) async fn handler_light_clients(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let light_clients = read::light_clients(app.state.view().world());
    respond(accept.as_ref(), light_clients)
}

/// `GET /v1/sccp/light-clients/{network}/sets`: the stored consensus sets of one light client.
pub(crate) async fn handler_light_client_sets(
    State(app): State<SharedAppState>,
    Path(network): Path<String>,
    accept: Option<ExtractAccept>,
) -> Response {
    match parse_network(&network) {
        Ok(network) => respond(
            accept.as_ref(),
            read::light_client_sets(app.state.view().world(), network),
        ),
        Err(response) => response,
    }
}

/// `GET /v1/sccp/governance`: the nonzero per-subject governance revisions.
pub(crate) async fn handler_governance(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let revisions = read::governance_revisions(app.state.view().world());
    respond(accept.as_ref(), revisions)
}

/// `GET /v1/sccp/governance/proposals`: every open SCCP proposal, oldest first, with its
/// admissibility and newest Parliament attempt.
pub(crate) async fn handler_governance_proposals(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let proposals = read::governance_proposals(app.state.view().world());
    respond(accept.as_ref(), proposals)
}

/// `GET /v1/sccp/rosters/current`.
pub(crate) async fn handler_roster_current(
    State(app): State<SharedAppState>,
    accept: Option<ExtractAccept>,
) -> Response {
    let roster = {
        let view = app.state.view();
        let generation = *store::roster_current::get(view.world());
        read::roster_view(view.world(), view.network_id().as_bytes(), generation)
    };
    match roster {
        Some(roster) => respond(accept.as_ref(), roster),
        None => read_error(&SccpReadError::NotFound("no roster generation yet".into())),
    }
}

/// `GET /v1/sccp/rosters/{generation}`.
pub(crate) async fn handler_roster(
    State(app): State<SharedAppState>,
    Path(generation): Path<u64>,
    accept: Option<ExtractAccept>,
) -> Response {
    let roster = {
        let view = app.state.view();
        read::roster_view(view.world(), view.network_id().as_bytes(), generation)
    };
    match roster {
        Some(roster) => respond(accept.as_ref(), roster),
        None => read_error(&SccpReadError::NotFound(format!("generation {generation}"))),
    }
}

/// `GET /v1/sccp/rosters/rotations?after_generation=&limit=`.
pub(crate) async fn handler_rotations(
    State(app): State<SharedAppState>,
    NoritoQuery(query): NoritoQuery<RotationQuery>,
    accept: Option<ExtractAccept>,
) -> Response {
    let (after_generation, limit) = match (
        parse_number(query.after_generation.as_deref(), 1_u64),
        parse_number(query.limit.as_deref(), read::MAX_ROTATION_STEPS),
    ) {
        (Ok(after_generation), Ok(limit)) => (after_generation, limit),
        (Err(response), _) | (_, Err(response)) => return response,
    };
    let chain = read::rotation_chain(&app.state.view(), after_generation, limit);
    respond(accept.as_ref(), chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_and_networks_parse_strictly() {
        assert_eq!(parse_word(&"ab".repeat(32)).ok(), Some([0xab; 32]));
        assert_eq!(
            parse_word(&format!("0x{}", "01".repeat(32))).ok(),
            Some([1; 32])
        );
        assert!(parse_word("abcd").is_err());
        assert_eq!(
            parse_network("ethereum-mainnet").ok(),
            Some(SccpNetworkV1::EthereumMainnet)
        );
        assert!(parse_network("sora-taira").is_err());
        assert!(parse_network("eth").is_err());
    }

    #[test]
    fn numbers_parse_with_defaults() {
        assert_eq!(parse_number(None, 1_u64).ok(), Some(1));
        assert_eq!(parse_number(Some("7"), 1_u64).ok(), Some(7));
        assert!(parse_number(Some("seven"), 1_u64).is_err());
    }

    #[test]
    fn attestation_choices_default_to_own() {
        assert_eq!(
            parse_choice(&AttestationQuery::default()).ok(),
            Some(SccpAttestationChoiceV1::Own)
        );
        assert_eq!(
            parse_choice(&AttestationQuery {
                attestation: Some("latest".into())
            })
            .ok(),
            Some(SccpAttestationChoiceV1::Latest)
        );
        assert!(
            parse_choice(&AttestationQuery {
                attestation: Some("soon".into())
            })
            .is_err()
        );
    }

    #[test]
    fn read_errors_map_to_http_statuses() {
        assert_eq!(
            read_error(&SccpReadError::NotFound("x".into())).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            read_error(&SccpReadError::Pending("x".into())).status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            read_error(&SccpReadError::Invalid("x".into())).status(),
            StatusCode::BAD_REQUEST
        );
    }
}
