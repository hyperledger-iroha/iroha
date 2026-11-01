//! Bounded public reads for the native independent-root game-unit prototype.
//!
//! Snapshots are discovery only. Import authority is always the authenticated original
//! export receipt; a current-state response is never accepted as an execution proof.

use super::*;
use iroha_crypto::Hash;

fn hash_selector(value: &str) -> Result<Hash, Error> {
    use std::str::FromStr as _;
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(routing::conversion_error(
            "expected a full 32-byte hexadecimal hash".into(),
        ));
    }
    Hash::from_str(value)
        .map_err(|_| routing::conversion_error("invalid game partition hash".into()))
}

pub async fn snapshot(
    State(app): State<SharedAppState>,
    axum::extract::Path(world): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    const ROUTE: &str = "/v1/game-partitions/{world}/state";
    let world = hash_selector(&world)?;
    let _principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        ROUTE,
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "game partition snapshot worker failed",
        move || {
            let value = iroha_core::smartcontracts::isi::game_partition::game_partition_snapshot(
                &state.view(),
                world,
            )
            .map_err(|message| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(message))
            })?
            .ok_or_else(|| Error::AppNotFound {
                code: "game_partition_not_found",
                message: "no installed game world at this root".into(),
            })?;
            if matches!(format, crate::utils::ResponseFormat::Norito) {
                Ok(crate::NoritoBody(value).into_response())
            } else {
                crate::routing::pretty_json_response(&value)
            }
        },
    )
    .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}

pub async fn export_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path((world, unit)): axum::extract::Path<(String, String)>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    const ROUTE: &str = "/v1/game-partitions/{world}/exports/{receipt}/proof";
    let (world, unit) = (hash_selector(&world)?, hash_selector(&unit)?);
    let _principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        ROUTE,
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "game partition receipt worker failed",
        move || {
            let proof =
                iroha_core::smartcontracts::isi::game_partition::game_partition_export_proof(
                    &state.view(),
                    world,
                    unit,
                )
                .map_err(|message| {
                    Error::Query(iroha_data_model::ValidationFail::InternalError(message))
                })?
                .ok_or_else(|| Error::AppNotFound {
                    code: "game_unit_export_not_found",
                    message: "no certified original export exists for this unit".into(),
                })?;
            if matches!(format, crate::utils::ResponseFormat::Norito) {
                Ok(crate::NoritoBody(proof).into_response())
            } else {
                crate::routing::pretty_json_response(&proof)
            }
        },
    )
    .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_bounded_full_hashes() {
        assert!(hash_selector(&"ab".repeat(32)).is_ok());
        for value in ["", "1", "../state", &"00".repeat(33), &"gg".repeat(32)] {
            assert!(hash_selector(value).is_err());
        }
    }
}
