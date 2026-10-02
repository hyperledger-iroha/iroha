//! Signed-body ordinary current-wallet publication scoped to the actual registered S/W owner.
//!
//! This route publishes data under current permission, never a permission grant or
//! finality/ownership fact. Every private original is borrowed from one qualified
//! native pre-tail cut and output keeps the aggregate finite pool through egress.

use super::*;
use crate::native_projection_response::{capacity, encode, native_committee_original_bytes};
use iroha_core::{
    state::{AllocationBudget, StateReadOnly},
    sumeragi::certified_chain::{CertifiedChain, QcVerification},
};
use iroha_primitives::time::NativeContinuousReading;
use iroha_torii_shared::ordinary_wallet_current::{
    ORDINARY_WALLET_CURRENT_MAX_BYTES_V1, ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1,
    ORDINARY_WALLET_CURRENT_ROUTE_V1, OrdinaryWalletCurrentOriginalRefV1,
    OrdinaryWalletCurrentRequestV1, require_ordinary_wallet_relation_v1,
};

// Payload fields physically drop before their prepaid intake reservation. The
// decoded request remains owned by this object while a worker borrows its selector.
struct RequestOriginalOwner {
    request: OrdinaryWalletCurrentRequestV1,
    _body: Bytes,
    _charge: iroha_allocation::AllocationReservation,
}

/// Canonical POST authentication covers the actual body; API-token alone is insufficient.
pub(super) async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: axum::http::Request<Body>,
) -> Result<AxResponse, Error> {
    Ok(finalize(handle(app, remote, request).await))
}

fn finalize(result: Result<AxResponse, Error>) -> AxResponse {
    let mut response = finalize_bridge_finality_attestation_response(result);
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "ordinary_current_wallet_unavailable",
        message: "Current certified native ordinary current wallet are unavailable.".into(),
    }
}

fn target(method: &Method, uri: &Uri, headers: &HeaderMap) -> Result<(), Error> {
    if method != &Method::POST
        || uri.path() != ORDINARY_WALLET_CURRENT_ROUTE_V1
        || uri.query().is_some()
    {
        return Err(conversion_error(
            "ordinary current wallet require the exact signed canonical POST target".into(),
        ));
    }
    let mut content_types = headers.get_all(axum::http::header::CONTENT_TYPE).iter();
    if content_types
        .next()
        .is_none_or(|value| value != utils::NORITO_MIME_TYPE)
        || content_types.next().is_some()
    {
        return Err(conversion_error(
            "ordinary current wallet require one canonical Norito Content-Type".into(),
        ));
    }
    validate_bounded_content_length(headers, ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1)
        .map_err(|_| {
            conversion_error(
                "ordinary current wallet request body length is invalid or exceeds its bound"
                    .into(),
            )
        })?;
    Ok(())
}

fn request_original(
    body: &[u8],
    network: &iroha_data_model::NetworkId,
    headers: &HeaderMap,
) -> Result<OrdinaryWalletCurrentRequestV1, Error> {
    if body.is_empty() || body.len() > ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1 {
        return Err(conversion_error(
            "ordinary current wallet request exceeds finite bound".into(),
        ));
    }
    let request: OrdinaryWalletCurrentRequestV1 = norito::decode_canonical_with_limits(
        body,
        norito::canonical_decode_limits(ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1),
    )
    .map_err(|_| {
        conversion_error("ordinary current wallet request canonical framing rejected".into())
    })?;
    if request
        .canonical_wire()
        .map_err(|_| conversion_error("ordinary current wallet selector rejected".into()))?
        != body
        || &request.network_id != network
        || (headers.contains_key(BRIDGE_FINALITY_CHALLENGE_HEADER)
            && bridge_finality_challenge(headers)? != request.request_nonce)
    {
        return Err(conversion_error(
            "ordinary current wallet exact scope/nonce differs".into(),
        ));
    }
    Ok(request)
}
fn require_live_scope(
    app: &SharedAppState,
    request: &OrdinaryWalletCurrentRequestV1,
) -> Result<(), Error> {
    use iroha_core::state::WorldReadOnly as _;
    request
        .validate()
        .map_err(|_| conversion_error("ordinary current wallet selector rejected".into()))?;
    let view = app.state.view();
    if view.world.accounts().get(&request.signatory).is_none()
        || view.world.accounts().get(&request.wallet).is_none()
    {
        return Err(Error::AppUnauthorized {
            code: "ordinary_current_wallet_scope_revoked",
            message: "Current wallet registration is unavailable.".into(),
        });
    }
    Ok(())
}
fn authenticate_body(
    app: &SharedAppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    request: &OrdinaryWalletCurrentRequestV1,
) -> Result<iroha_crypto::PublicKey, Error> {
    let verified = crate::app_auth::verify_canonical_network_request(
        &app.state,
        app.state.network_id_ref(),
        headers,
        method,
        uri,
        body,
        Some(&request.wallet),
    )?
    .ok_or_else(|| Error::AppUnauthorized {
        code: "canonical_authentication_required",
        message: "Canonical signed-body wallet authentication is required.".into(),
    })?;
    let key = require_ordinary_wallet_relation_v1(&request.signatory, &request.wallet)
        .map_err(|_| conversion_error("ordinary current wallet relation rejected".into()))?;
    if verified.account != request.wallet
        || verified.signer != key
        || verified.verified_signers.as_slice() != [key.clone()]
    {
        return Err(Error::AppUnauthorized {
            code: "ordinary_current_wallet_signer_rejected",
            message: "Actual wallet signer differs.".into(),
        });
    }
    require_live_scope(app, request)?;
    Ok(key)
}
fn require_budget(started: NativeContinuousReading) -> Result<(), Error> {
    if started.elapsed().map_err(|_| unavailable())? > std::time::Duration::from_secs(10) {
        return Err(unavailable());
    }
    Ok(())
}

async fn handle(
    app: SharedAppState,
    remote: std::net::SocketAddr,
    request: axum::http::Request<Body>,
) -> Result<AxResponse, Error> {
    let started = NativeContinuousReading::now().map_err(|_| unavailable())?;
    let (parts, body) = request.into_parts();
    let headers = parts.headers;
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    target(&parts.method, &parts.uri, &headers)?;
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        ORDINARY_WALLET_CURRENT_ROUTE_V1,
        principal,
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    // Acquire original aggregate capacity before receiving/decoding the private
    // request. The pre-admitted bound covers bounded body collection, canonical
    // decode backing and validation's transient native re-encoding; none escapes.
    let bytes = app.query_fanout_working_set_bytes;
    let memory = app
        .query_fanout_inflight
        .try_acquire_parts([u64::try_from(bytes).map_err(|_| capacity())?])
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(bytes);
    let intake_bytes = ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1
        .checked_mul(8)
        .ok_or_else(capacity)?;
    let intake_charge = budget
        .try_reserve_bytes(intake_bytes)
        .map_err(|_| capacity())?;
    // The native authenticated account and its capture copy survive the worker
    // intake owner. Retain their finite original backing through both egress fences.
    let _principal_charge = budget
        .try_reserve_bytes(
            crate::app_auth::CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
                .checked_mul(8)
                .ok_or_else(capacity)?,
        )
        .map_err(|_| capacity())?;
    let body = axum::body::to_bytes(body, ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1)
        .await
        .map_err(|_| {
            conversion_error("ordinary current wallet request body exceeds its finite bound".into())
        })?;
    let request = request_original(body.as_ref(), app.state.network_id_ref(), &headers)?;
    let key = authenticate_body(
        &app,
        &headers,
        &parts.method,
        &parts.uri,
        body.as_ref(),
        &request,
    )?;
    let account = request.wallet.clone();
    let challenge = request.request_nonce;
    let egress_request = request.clone();
    let intake = RequestOriginalOwner {
        request,
        _body: body,
        _charge: intake_charge,
    };
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    require_budget(started)?;
    require_live_scope(&app, &egress_request)?;
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
    let capture_authority = account.clone();
    let capture_signer = key;
    let max_response = app
        .torii_proxy_max_response_bytes
        .min(ORDINARY_WALLET_CURRENT_MAX_BYTES_V1);
    let response = routing::run_admitted_blocking(
        admission,
        "native ordinary current wallet worker failed",
        move || {
            let _memory = memory;
            let intake = intake;
            let view = state.view();
            let height = u64::try_from(view.height()).map_err(|_| unavailable())?;
            require_budget(started)?;
            if height < 2 || height != intake.request.height {
                return Err(unavailable());
            }
            let chain = CertifiedChain::new(&view).map_err(|_| unavailable())?;
            let certified = chain.certified(height).map_err(|_| unavailable())?;
            if certified.verification() != QcVerification::Verified {
                return Err(unavailable());
            }
            let tip = certified.into_committed();
            let genesis = chain.committed(1).map_err(|_| unavailable())?;
            let genesis_bytes =
                norito::canonical_frame_len(genesis.block().as_ref()).map_err(|_| unavailable())?;
            let tip_bytes =
                norito::canonical_frame_len(tip.block().as_ref()).map_err(|_| unavailable())?;
            let genesis_committee = &genesis.commitment().schedule.current.committee;
            let tip_committee = &tip.commitment().schedule.current.committee;
            let genesis_storage = native_committee_original_bytes(
                genesis_committee.len(),
                genesis_committee.iter().map(|member| {
                    (
                        member.validator.public_key(),
                        member.proof_of_possession.as_slice(),
                    )
                }),
            )?;
            let tip_storage = native_committee_original_bytes(
                tip_committee.len(),
                tip_committee.iter().map(|member| {
                    (
                        member.validator.public_key(),
                        member.proof_of_possession.as_slice(),
                    )
                }),
            )?;
            let proof_bytes = genesis_bytes
                .checked_add(tip_bytes)
                .and_then(|value| value.checked_add(genesis_storage))
                .and_then(|value| value.checked_add(tip_storage))
                .and_then(|value| value.checked_add(16 * 1024))
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
            let mut encoded = state
                .with_native_ordinary_wallet_current_v1(
                    &tip,
                    &capture_authority,
                    &capture_signer,
                    &intake.request.signatory,
                    &intake.request.wallet,
                    &budget,
                    |snapshot, s, w| {
                        require_budget(started)
                            .map_err(|_| "ordinary current publication expired".to_owned())?;
                        let payload = OrdinaryWalletCurrentOriginalRefV1::new(
                            &intake.request,
                            &attestation,
                            snapshot,
                            s,
                            w,
                        );
                        encode(&payload, format, max_response, &budget, unavailable).map_err(|_| {
                            "ordinary current original serialization refused".to_owned()
                        })
                    },
                )
                .map_err(|_| unavailable())?;
            require_budget(started)?;
            encoded.memory = Some(_memory);
            let mut response = AxResponse::new(Body::from(Bytes::from_owner(encoded)));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static(match format {
                    ResponseFormat::Norito => utils::NORITO_MIME_TYPE,
                    ResponseFormat::Json => "application/json",
                }),
            );
            Ok(response)
        },
    )
    .await?;
    require_budget(started)?;
    require_live_scope(&app, &egress_request)?;
    let response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ORDINARY_WALLET_CURRENT_ROUTE_V1,
        response,
        true,
    )
    .await?;
    require_budget(started)?;
    require_live_scope(&app, &egress_request)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_current_wallet_requires_exact_signed_canonical_http_target() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(utils::NORITO_MIME_TYPE),
        );
        let uri: Uri = ORDINARY_WALLET_CURRENT_ROUTE_V1.parse().unwrap();
        target(&Method::POST, &uri, &headers).unwrap();
        assert!(target(&Method::GET, &uri, &headers).is_err());
        let query: Uri = format!("{}?wallet=other", ORDINARY_WALLET_CURRENT_ROUTE_V1)
            .parse()
            .unwrap();
        assert!(target(&Method::POST, &query, &headers).is_err());
        headers.append(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        assert!(target(&Method::POST, &uri, &headers).is_err());
    }
    #[test]
    fn ordinary_current_wallet_body_refuses_other_network_nonce_and_unbounded_frame() {
        use iroha_data_model::account::{AccountId, MultisigMember, MultisigPolicy};
        let key = iroha_crypto::KeyPair::from_seed(vec![13; 32], iroha_crypto::Algorithm::Ed25519);
        let request = OrdinaryWalletCurrentRequestV1 {
            version: 1,
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                    b"synthetic current wallet request",
                )),
            ),
            height: 2,
            request_nonce: [7; 32],
            signatory: AccountId::new(key.public_key().clone()),
            wallet: AccountId::new_multisig(
                MultisigPolicy::new(
                    1,
                    vec![MultisigMember::new(key.public_key().clone(), 1).unwrap()],
                )
                .unwrap(),
            ),
        };
        let raw = request.canonical_wire().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(
            request_original(&raw, &request.network_id, &headers).unwrap(),
            request
        );
        headers.insert(
            BRIDGE_FINALITY_CHALLENGE_HEADER,
            HeaderValue::from_str(&hex::encode([8; 32])).unwrap(),
        );
        assert!(request_original(&raw, &request.network_id, &headers).is_err());
        assert!(
            request_original(
                &vec![0; ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1 + 1],
                &request.network_id,
                &HeaderMap::new()
            )
            .is_err()
        );
    }
}
