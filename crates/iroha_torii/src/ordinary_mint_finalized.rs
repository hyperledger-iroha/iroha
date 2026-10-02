//! Separate payer-authenticated ordinary finalized debit read, from actual World and Kura.
//! A pending read has no funding effect and never routes through the OEM command registry.
use super::*;
use crate::native_projection_response::{capacity, encode};
use iroha_core::{
    smartcontracts::isi::kagemusha::KagemushaReserveOperationRecordV1,
    state::{AllocationBudget, StateReadOnly, WorldReadOnly},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1, KagemushaOrdinaryTopUpFinalizedOriginalV1,
};
use iroha_primitives::time::NativeContinuousReading;
use iroha_torii_shared::ordinary_mint_finalized::{
    ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1, ORDINARY_MINT_FINALIZED_ROUTE_V1,
    OrdinaryMintFinalizedReadV1,
};
use mv::storage::StorageReadOnly as _;
use sha2::{Digest as _, Sha256};

/// Every simultaneously owned maximum: three source/verification representations, the charged
/// canonical output, and eight finite selector/authentication representations. This read uses
/// the existing KAGEMUSHA weighted command lane, whose startup capacity includes this term.
pub(super) fn maximum_working_set_bytes() -> Option<usize> {
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
        .checked_mul(4)?
        .checked_add(ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1.checked_mul(8)?)
}

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "ordinary_mint_finality_unavailable",
        message: "Exact certified ordinary Mint finality is unavailable.".into(),
    }
}
fn require_budget(started: NativeContinuousReading) -> Result<(), Error> {
    if started.elapsed().map_err(|_| unavailable())? > std::time::Duration::from_secs(10) {
        return Err(unavailable());
    }
    Ok(())
}
fn target(method: &Method, uri: &Uri, headers: &HeaderMap) -> Result<(), Error> {
    let mut content = headers.get_all(axum::http::header::CONTENT_TYPE).iter();
    if method != Method::POST
        || uri.path() != ORDINARY_MINT_FINALIZED_ROUTE_V1
        || uri.query().is_some()
        || content.next().is_none_or(|v| v != utils::NORITO_MIME_TYPE)
        || content.next().is_some()
        || headers
            .get(axum::http::header::ACCEPT)
            .is_none_or(|v| v != utils::NORITO_MIME_TYPE)
    {
        return Err(conversion_error(
            "ordinary finalized read requires its exact canonical signed POST target/codec".into(),
        ));
    }
    validate_bounded_content_length(headers, ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1)
        .map_err(|_| conversion_error("ordinary finalized read body length rejected".into()))?;
    Ok(())
}
fn require_live_payer(
    app: &SharedAppState,
    request: &OrdinaryMintFinalizedReadV1,
) -> Result<(), Error> {
    if &request.network_id != app.state.network_id_ref()
        || app
            .state
            .view()
            .world()
            .accounts()
            .get(&request.payer)
            .is_none()
    {
        return Err(Error::AppUnauthorized {
            code: "ordinary_mint_finality_payer_rejected",
            message: "Current signed payer account is unavailable.".into(),
        });
    }
    Ok(())
}
fn require_current_signed_payer(
    app: &SharedAppState,
    selected: &OrdinaryMintFinalizedReadV1,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Result<(), Error> {
    require_live_payer(app, selected)?;
    let verified = app_auth::verify_canonical_network_request(
        &app.state,
        app.state.network_id_ref(),
        headers,
        method,
        uri,
        body,
        Some(&selected.payer),
    )?
    .ok_or_else(|| Error::AppUnauthorized {
        code: "canonical_authentication_required",
        message: "Current canonical signed payer authentication is required.".into(),
    })?;
    if verified.account != selected.payer {
        return Err(unavailable());
    }
    Ok(())
}
fn pending() -> AxResponse {
    let mut response = AxResponse::new(Body::empty());
    *response.status_mut() = StatusCode::ACCEPTED;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}
fn require_record(
    record: &iroha_core::smartcontracts::isi::kagemusha::KagemushaOrdinaryTopUpRecordV1,
    request: &OrdinaryMintFinalizedReadV1,
) -> Result<(), Error> {
    record.validate_basic().map_err(|_| unavailable())?;
    if record.payer != request.payer
        || record.pool.network_id != request.network_id
        || record.operation_id != request.operation_id
        || record.request_original_sha256 != request.request_original_sha256
        || <[u8; 32]>::from(Sha256::digest(&record.issuer_decision_original))
            != request.issuer_decision_original_sha256
    {
        return Err(conversion_error(
            "ordinary finalized read differs from the exact immutable payer/request/decision"
                .into(),
        ));
    }
    Ok(())
}

/// The current account signature covers the whole exact request/target; no bearer read grant.
pub(super) async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: axum::http::Request<Body>,
) -> Result<AxResponse, Error> {
    let started = NativeContinuousReading::now().map_err(|_| unavailable())?;
    let (parts, body) = request.into_parts();
    let headers = parts.headers;
    target(&parts.method, &parts.uri, &headers)?;
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        ORDINARY_MINT_FINALIZED_ROUTE_V1,
        principal,
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let working = maximum_working_set_bytes().ok_or_else(capacity)?;
    let memory = app
        .kagemusha_command_memory_inflight
        .try_acquire_parts([u64::try_from(working).map_err(|_| capacity())?])
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(working);
    // Charge body, decode/reencode and selected complete original before any private allocation.
    let _intake = budget
        .try_reserve_bytes(ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1 * 8)
        .map_err(|_| capacity())?;
    let body = axum::body::to_bytes(body, ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1)
        .await
        .map_err(|_| {
            conversion_error("ordinary finalized read exceeds finite body bound".into())
        })?;
    let selected = OrdinaryMintFinalizedReadV1::decode_original(&body).map_err(|_| {
        conversion_error("ordinary finalized read canonical selector rejected".into())
    })?;
    require_current_signed_payer(&app, &selected, &headers, &parts.method, &parts.uri, &body)?;
    require_budget(started)?;
    let transaction_hash = {
        let view = app.state.view();
        let Some(operation) = view
            .world()
            .kagemusha_reserve_operations()
            .get(&selected.operation_id)
        else {
            return Ok(pending());
        };
        let KagemushaReserveOperationRecordV1::OrdinaryTopUp(record) = operation else {
            return Err(conversion_error(
                "ordinary Mint operation family differs".into(),
            ));
        };
        require_record(record, &selected)?;
        iroha_crypto::HashOf::<iroha_data_model::transaction::SignedTransaction>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(record.reserve_receipt.transaction_hash))
    };
    let Some((status, resolved)) = pipeline_status_local_entry_checked(&app, &transaction_hash)?
    else {
        return Ok(pending());
    };
    if resolved != "state" || status.kind != PipelineStatusKind::Applied {
        return Ok(pending());
    }
    let height = status.block_height.ok_or_else(unavailable)?.get();
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    require_budget(started)?;
    let state = Arc::clone(&app.state);
    let worker_selected = selected.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "ordinary finalized read worker failed",
        move || {
            let _memory = memory;
            // Full Kura block/proof construction and its output each keep a prepaid finite ceiling.
            let _source = budget
                .try_reserve_bytes(
                    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
                        .checked_mul(3)
                        .ok_or_else(capacity)?,
                )
                .map_err(|_| capacity())?;
            let view = state.view();
            let Some(operation) = view
                .world()
                .kagemusha_reserve_operations()
                .get(&worker_selected.operation_id)
            else {
                return Err(unavailable());
            };
            let KagemushaReserveOperationRecordV1::OrdinaryTopUp(record) = operation else {
                return Err(unavailable());
            };
            require_record(record, &worker_selected)?;
            if view
                .world()
                .kagemusha_mint_credit_operations()
                .get(&record.credit_id)
                .copied()
                != Some(record.operation_id)
                || view
                    .world()
                    .kagemusha_issuance_operations()
                    .get(&record.issuance_commitment)
                    .copied()
                    != Some(record.operation_id)
            {
                return Err(unavailable());
            }
            require_budget(started)?;
            let finality = iroha_core::query::native_receipts::kagemusha_operation_finality(
                &view,
                height,
                record.operation_id,
            )
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
            if finality.reserve_receipt_witness.receipt != record.reserve_receipt
                || finality.finality_proof.height() != height
            {
                return Err(unavailable());
            }
            let original = KagemushaOrdinaryTopUpFinalizedOriginalV1 {
                version: 1,
                request_original: record.request_original.clone(),
                issuer_decision_original: record.issuer_decision_original.clone(),
                finality,
            };
            original.validate_originals().map_err(|_| unavailable())?;
            // Independently genuine checkpoint derives from this same actual retained Kura cut.
            let anchor = iroha_data_model::isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1 {
                network_id: *view.network_id(),
                checkpoint: iroha_core::sumeragi::finality::build_checkpoint(&view, height)
                    .map_err(|_| unavailable())?,
            };
            original
                .validate_against(&anchor)
                .map_err(|_| unavailable())?;
            require_budget(started)?;
            let mut encoded = encode(
                &original,
                ResponseFormat::Norito,
                KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1,
                &budget,
                unavailable,
            )?;
            require_budget(started)?;
            encoded.memory = Some(_memory);
            let mut response = AxResponse::new(Body::from(Bytes::from_owner(encoded)));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static(utils::NORITO_MIME_TYPE),
            );
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            );
            Ok(response)
        },
    )
    .await?;
    require_budget(started)?;
    require_current_signed_payer(&app, &selected, &headers, &parts.method, &parts.uri, &body)?;
    let response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ORDINARY_MINT_FINALIZED_ROUTE_V1,
        response,
        true,
    )
    .await?;
    require_budget(started)?;
    require_current_signed_payer(&app, &selected, &headers, &parts.method, &parts.uri, &body)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finalized_read_requires_exact_account_signed_target_and_single_canonical_codec() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(utils::NORITO_MIME_TYPE),
        );
        headers.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(utils::NORITO_MIME_TYPE),
        );
        let uri = ORDINARY_MINT_FINALIZED_ROUTE_V1.parse::<Uri>().unwrap();
        target(&Method::POST, &uri, &headers).unwrap();
        assert!(target(&Method::GET, &uri, &headers).is_err());
        assert!(
            target(
                &Method::POST,
                &format!("{}?operation=other", ORDINARY_MINT_FINALIZED_ROUTE_V1)
                    .parse()
                    .unwrap(),
                &headers
            )
            .is_err()
        );
        headers.append(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(utils::NORITO_MIME_TYPE),
        );
        assert!(target(&Method::POST, &uri, &headers).is_err());
    }
    #[test]
    fn finalized_read_uses_prepaid_command_capacity_and_keeps_last_byte_ownership() {
        let working = maximum_working_set_bytes().unwrap();
        let aggregate =
            usize::try_from(defaults::torii::QUERY_FANOUT_MAX_RETAINED_BYTES.get()).unwrap();
        let query = query_memory_geometry(aggregate, 64_000_000, 1).unwrap();
        assert!(query.fanout_working_set_bytes < working);
        let pool =
            ByteWeightedMemoryPool::new(kagemusha_command_memory_pool_bytes(64_000_000).unwrap())
                .unwrap();
        let weight = u64::try_from(working).unwrap();
        let initial = pool.available_bytes();
        let permit = pool.try_acquire_parts([weight]).unwrap();
        let memory = QueryFanoutMemoryReservation::new(permit);
        let retained = memory.clone();
        drop(memory);
        assert_eq!(pool.available_bytes(), initial - weight);
        drop(retained);
        assert_eq!(pool.available_bytes(), initial);
        assert!(pool.can_reserve_parts([weight]));
    }

    #[test]
    fn pending_read_is_empty_and_never_a_finalized_data_original() {
        let response = pending();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert!(
            !response
                .headers()
                .contains_key(axum::http::header::CONTENT_TYPE)
        );
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "private, no-store"
        );
    }
}
