//! State-root and state-proof responses backed only by exact Sumeragi-v2 finality.

use super::*;

/// Closed response shared by both authenticated ledger-state endpoints.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
pub(super) struct StateFinalityResponse {
    /// Requested one-based committed block height.
    pub(super) height: u64,
    /// Canonical header hash authenticated by both State and v2 finality.
    pub(super) block_hash: HashOf<BlockHeader>,
    /// Exact post-state root authenticated by the Sumeragi-v2 CommitQC.
    pub(super) state_root: iroha_crypto::Hash,
    /// Canonical header matched to both committed State and durable Kura evidence.
    pub(super) block_header: BlockHeader,
    /// Exact current Sumeragi-v2 finality artifact verified by Kura.
    pub(super) finality_artifact:
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
}

fn not_found() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::NotFound,
    ))
}

fn internal_error(message: impl Into<String>) -> Error {
    Error::Query(iroha_data_model::ValidationFail::InternalError(
        message.into(),
    ))
}

fn load(app: &AppState, height: u64) -> Result<StateFinalityResponse, Error> {
    let height_nz = NonZeroU64::new(height)
        .ok_or_else(|| conversion_error("height must be at least 1".to_owned()))?;
    let height_usize = NonZeroUsize::new(
        height_nz
            .get()
            .try_into()
            .map_err(|_| conversion_error("height exceeds host pointer width".to_owned()))?,
    )
    .ok_or_else(|| conversion_error("height must be at least 1".to_owned()))?;
    // This lookup binds the Kura body to State's committed hash journal. A
    // height-only Kura lookup could otherwise observe a staged, uncommitted body.
    let block = app
        .state
        .block_by_height(height_usize)
        .ok_or_else(not_found)?;
    let block_header = block.header();
    let block_hash = block.hash();
    if block_header.height() != height_nz {
        return Err(internal_error(format!(
            "committed State block at height {height} carries header height {}",
            block_header.height()
        )));
    }
    if !block.has_results() {
        return Err(internal_error(format!(
            "committed State block {height} has no execution results"
        )));
    }
    // Kura's reader validates the immutable record, canonical header and
    // complete wire bindings, roster PoPs, and CommitQC cryptography.
    let finality_artifact = app
        .kura
        .v2_finality_artifact(height)
        .map_err(|error| {
            internal_error(format!(
                "invalid durable Sumeragi-v2 finality for committed block {height}: {error}"
            ))
        })?
        .ok_or_else(not_found)?;
    if finality_artifact.height != height || finality_artifact.block_hash != block_hash {
        return Err(internal_error(format!(
            "durable Sumeragi-v2 finality does not match committed State block {height}"
        )));
    }
    finality_artifact
        .validate_for_header(&block_header)
        .map_err(|error| {
            internal_error(format!(
                "durable Sumeragi-v2 finality/header association failed for committed block {height}: {error}"
            ))
        })?;
    let state_root = finality_artifact
        .commit_qc
        .execution_commitment
        .post_state_root;
    Ok(StateFinalityResponse {
        height,
        block_hash,
        state_root,
        block_header,
        finality_artifact,
    })
}

fn response(
    app: &AppState,
    height: u64,
    headers: &axum::http::HeaderMap,
) -> Result<Response, Error> {
    let accept = headers.get(axum::http::header::ACCEPT);
    let format = match crate::utils::negotiate_response_format(accept) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let payload = load(app, height)?;
    match format {
        ResponseFormat::Norito => Ok(NoritoBody(payload).into_response()),
        ResponseFormat::Json => {
            let body = norito::json::to_json_pretty(&payload).map_err(|error| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(
                    error.to_string(),
                ))
            })?;
            let mut response = Response::new(Body::from(body));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(response)
        }
    }
}

/// Serve the authenticated ledger state-root route.
pub(super) async fn handler_ledger_state_root(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    response(app.as_ref(), height, &headers)
}

/// Serve the authenticated ledger state-proof route.
pub(super) async fn handler_ledger_state_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    response(app.as_ref(), height, &headers)
}
