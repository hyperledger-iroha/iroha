//! Challenge-bound, data-only complete World publication at the native applied cut.

use super::*;
use crate::native_projection_response::encode;
use iroha_core::{
    state::{AllocationBudget, StateReadOnly},
    sumeragi::certified_chain::{CertifiedChain, QcVerification},
};
use iroha_data_model::asset::AssetDefinitionId;
use iroha_torii_shared::kagemusha_state::{
    KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1, KagemushaAuthorityStateRefV1,
};

const ROUTE: &str = "/v1/kagemusha/authority-state/{asset_definition_id}";

/// The API-token, challenge and resource gates precede every native capture.
pub(super) async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::Path(asset): axum::extract::Path<String>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let result = handle(app, asset, headers, remote).await;
    Ok(finalize_bridge_finality_attestation_response(result))
}

async fn handle(
    app: SharedAppState,
    asset: String,
    headers: HeaderMap,
    remote: std::net::SocketAddr,
) -> Result<AxResponse, Error> {
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let challenge = bridge_finality_challenge(&headers)?;
    let asset_id: AssetDefinitionId = asset
        .parse()
        .map_err(|_| conversion_error("invalid canonical asset definition ID".into()))?;
    if asset_id.to_string() != asset {
        return Err(conversion_error(
            "asset definition ID must be canonical".into(),
        ));
    }
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(&headers, Some(remote.ip()), ROUTE, principal);
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    // Reserve one complete configured operation from the real aggregate query
    // pool before spawning. The same permit follows output through its last byte owner.
    let bytes = app.query_fanout_working_set_bytes;
    let memory = app
        .query_fanout_inflight
        .try_acquire_parts([u64::try_from(bytes).map_err(|_| capacity())?])
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(bytes);
    let max_response = app
        .torii_proxy_max_response_bytes
        .min(KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1);
    let driver = app.sumeragi.as_ref().ok_or_else(unavailable)?;
    if driver.restart_required() {
        return Err(unavailable());
    }
    let status = driver.status_dto().ok_or_else(unavailable)?;
    if status.unanchored || status.abstaining || status.halted.is_some() {
        return Err(unavailable());
    }
    let identity = driver.identity().clone();
    if status.signer.as_ref() != Some(identity.node_id.public_key()) {
        return Err(unavailable());
    }
    let fingerprint = iroha_crypto::Hash::new_from_chunks(&[
        app.build_status.version.as_bytes(),
        app.build_status.git_commit_sha.as_bytes(),
    ]);
    let state = app.state.clone();
    let signer = app.torii_proxy_bridge_signer.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "native authority state worker failed",
        move || {
            let _memory = memory;
            let view = state.view();
            let height = u64::try_from(view.height()).map_err(|_| unavailable())?;
            if height < 2 {
                return Err(unavailable());
            }
            let chain = CertifiedChain::new(&view).map_err(|_| unavailable())?;
            let certified = chain.certified(height).map_err(|_| unavailable())?;
            if certified.verification() != QcVerification::Verified {
                return Err(unavailable());
            }
            let tip = certified.into_committed();
            // Proof frame copies and their finite committee/signer vectors are
            // prepaid from the same operation budget before the existing builder.
            let genesis = chain.committed(1).map_err(|_| unavailable())?;
            let proof_bytes = norito::canonical_frame_len(genesis.block().as_ref())
                .and_then(|len| {
                    norito::canonical_frame_len(tip.block().as_ref()).map(|tip_len| (len, tip_len))
                })
                .map_err(|_| unavailable())?;
            let proof_bytes = proof_bytes
                .0
                .checked_add(proof_bytes.1)
                .and_then(|len| len.checked_add(16 * 1024))
                .ok_or_else(capacity)?;
            let _proof_charge = budget
                .try_reserve_bytes(proof_bytes)
                .map_err(|_| capacity())?;
            let attestation = iroha_core::sumeragi::finality::build_attestation(
                &view,
                status,
                &identity,
                fingerprint,
                height,
                challenge,
                &signer,
            )
            .map_err(|_| unavailable())?;
            drop(chain);
            drop(view);
            let mut body = state
                .with_native_world_state_snapshot_v1(
                    &tip,
                    &asset_id,
                    &budget,
                    |snapshot, definition, incarnation, registry| {
                        let payload = KagemushaAuthorityStateRefV1::new(
                            &attestation,
                            snapshot,
                            definition,
                            incarnation,
                            registry,
                        );
                        encode(&payload, format, max_response, &budget, unavailable)
                            .map_err(|_| "native authority state serialization refused".to_owned())
                    },
                )
                .map_err(|_| unavailable())?;
            body.memory = Some(_memory);
            let content_type = match format {
                ResponseFormat::Norito => "application/x-norito",
                ResponseFormat::Json => "application/json",
            };
            let mut response = AxResponse::new(Body::from(Bytes::from_owner(body)));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static(content_type),
            );
            Ok(response)
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

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "kagemusha_authority_state_unavailable",
        message: "Current certified native authority state is unavailable.".into(),
    }
}
fn capacity() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
    ))
}
