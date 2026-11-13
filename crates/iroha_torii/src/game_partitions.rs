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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransferProofKind {
    Intent,
    Reservation,
    Export,
    Import,
    Cancellation,
}
impl TransferProofKind {
    fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "intent" => Ok(Self::Intent),
            "reservation" => Ok(Self::Reservation),
            "export" => Ok(Self::Export),
            "import" => Ok(Self::Import),
            "cancellation" => Ok(Self::Cancellation),
            _ => Err(routing::conversion_error(
                "game transfer proof kind must be intent, reservation, export, import or cancellation".into(),
            )),
        }
    }
}

fn transfer_value<T>(
    value: Result<Option<T>, String>,
    format: crate::utils::ResponseFormat,
) -> Result<AxResponse, Error>
where
    T: norito::NoritoSerialize + norito::json::JsonSerialize + Send + 'static,
{
    let value = value
        .map_err(|message| Error::Query(iroha_data_model::ValidationFail::InternalError(message)))?
        .ok_or_else(|| Error::AppNotFound {
            code: "game_transfer_record_not_found",
            message: "the requested original transfer record is absent at this root".into(),
        })?;
    if matches!(format, crate::utils::ResponseFormat::Norito) {
        Ok(crate::NoritoBody(value).into_response())
    } else {
        routing::pretty_json_response(&value)
    }
}

fn transfer_read(
    view: &impl iroha_core::state::StateReadOnly,
    world: Hash,
    transfer: Hash,
    kind: Option<TransferProofKind>,
    format: crate::utils::ResponseFormat,
) -> Result<AxResponse, Error> {
    use iroha_core::smartcontracts::isi::game_partition as game;
    match kind {
        None => transfer_value(
            game::native_game_transfer_snapshot(view, world, transfer),
            format,
        ),
        Some(TransferProofKind::Intent) => transfer_value(
            game::game_unit_export_intent_proof(view, world, transfer),
            format,
        ),
        Some(TransferProofKind::Reservation) => transfer_value(
            game::game_unit_reservation_proof(view, world, transfer),
            format,
        ),
        Some(TransferProofKind::Export) => transfer_value(
            game::reserved_game_unit_export_proof(view, world, transfer),
            format,
        ),
        Some(TransferProofKind::Import) => transfer_value(
            game::game_unit_import_ack_proof(view, world, transfer),
            format,
        ),
        Some(TransferProofKind::Cancellation) => transfer_value(
            game::game_unit_export_cancel_proof(view, world, transfer),
            format,
        ),
    }
}

pub async fn transfer_snapshot(
    State(app): State<SharedAppState>,
    axum::extract::Path((world, transfer)): axum::extract::Path<(String, String)>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    transfer_response(app, world, transfer, None, headers, remote).await
}

pub async fn transfer_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path((world, transfer, kind)): axum::extract::Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    transfer_response(
        app,
        world,
        transfer,
        Some(TransferProofKind::parse(&kind)?),
        headers,
        remote,
    )
    .await
}

async fn transfer_response(
    app: SharedAppState,
    world: String,
    transfer: String,
    kind: Option<TransferProofKind>,
    headers: axum::http::HeaderMap,
    remote: std::net::SocketAddr,
) -> Result<AxResponse, Error> {
    let route = if kind.is_some() {
        "/v1/game-partitions/{world}/transfers/{transfer}/proof/{kind}"
    } else {
        "/v1/game-partitions/{world}/transfers/{transfer}/state"
    };
    let (world, transfer) = (hash_selector(&world)?, hash_selector(&transfer)?);
    let _principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        route,
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response =
        routing::run_admitted_blocking(admission, "game transfer proof worker failed", move || {
            transfer_read(&state.view(), world, transfer, kind, format)
        })
        .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        route,
        response,
        true,
    )
    .await
}

#[cfg(test)]
mod transfer_tests {
    use super::*;

    #[test]
    fn transfer_proof_selector_has_only_exact_protocol_stages() {
        for (value, expected) in [
            ("intent", TransferProofKind::Intent),
            ("reservation", TransferProofKind::Reservation),
            ("export", TransferProofKind::Export),
            ("import", TransferProofKind::Import),
            ("cancellation", TransferProofKind::Cancellation),
        ] {
            assert_eq!(TransferProofKind::parse(value).unwrap(), expected);
        }
        for value in ["", "snapshot", "ack", "IMPORT", "../state", "import/intent"] {
            assert!(TransferProofKind::parse(value).is_err());
        }
    }

    #[test]
    fn transfer_read_never_invents_absent_discovery_or_certified_records() {
        let state = iroha_core::state::State::new_for_testing(
            iroha_core::state::World::new(),
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        );
        for kind in [
            None,
            Some(TransferProofKind::Intent),
            Some(TransferProofKind::Reservation),
            Some(TransferProofKind::Export),
            Some(TransferProofKind::Import),
            Some(TransferProofKind::Cancellation),
        ] {
            let result = transfer_read(
                &state.view(),
                Hash::prehashed([1; 32]),
                Hash::prehashed([2; 32]),
                kind,
                crate::utils::ResponseFormat::Norito,
            );
            assert!(matches!(
                result,
                Err(Error::AppNotFound {
                    code: "game_transfer_record_not_found",
                    ..
                })
            ));
        }
        assert!(matches!(
            transfer_value::<u64>(
                Err("original source unavailable".into()),
                crate::utils::ResponseFormat::Norito
            ),
            Err(Error::Query(_))
        ));
    }
}
