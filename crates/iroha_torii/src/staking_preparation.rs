//! Bounded read-only staking monetary plan preparation.

use super::*;
use iroha_data_model::nexus::PublicLanePreparationRequestV1;

/// Resolve exact current monetary inputs from one committed state snapshot.
pub(super) async fn handler_staking_preparation(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    crate::utils::extractors::Norito(request): crate::utils::extractors::Norito<
        PublicLanePreparationRequestV1,
    >,
) -> Result<Response, Error> {
    validate_api_token(app.as_ref(), &headers)?;
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        iroha_torii_shared::route_catalog::core::NEXUS_STAKING_PREPARATION_POST.path(),
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response =
        routing::run_admitted_blocking(admission, "staking preparation worker failed", move || {
            let view = state.view();
            let payload =
                iroha_core::smartcontracts::isi::staking::preparation::prepare_public_lane_plan(
                    &view, request,
                )
                .map_err(|error| {
                    Error::Query(iroha_data_model::ValidationFail::InternalError(
                        error.to_string(),
                    ))
                })?;
            Ok(crate::utils::respond_with_format(payload, format))
        })
        .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        "v1/nexus/staking/prepare",
        response,
        true,
    )
    .await
}
