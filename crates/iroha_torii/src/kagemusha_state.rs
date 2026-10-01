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
pub(super) fn capacity() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
    ))
}

use iroha_torii_shared::resource_names_state::{
    NATIVE_RESOURCE_NAMES_STATE_MAX_BYTES_V1, NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1,
    NativeAssetAliasBindingOriginalRefV1, NativeDataspaceSnsOriginalRefV1,
    NativeResourceNamesStateRefV1,
};
const RESOURCE_NAMES_ROUTE: &str = "/v1/ledger/resource-names/{challenge}";

/// Full-original carrier requires real native signed read authority; API-token access alone never suffices.
pub(super) async fn handle_resource_names(
    State(app): State<SharedAppState>,
    axum::extract::Path(challenge): axum::extract::Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let result = handle_resource_names_inner(app, challenge, headers, method, uri, remote).await;
    Ok(finalize_resource_names_response(result))
}

async fn handle_resource_names_inner(
    app: SharedAppState,
    challenge_literal: String,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    remote: std::net::SocketAddr,
) -> Result<AxResponse, Error> {
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let challenge =
        validate_resource_names_request_target(&headers, &method, &uri, &challenge_literal)?;
    let account = authenticate_full_ledger_carrier_reader(&app, &headers, &method, &uri)?;
    let capture_authority = account.clone();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(&headers, Some(remote.ip()), RESOURCE_NAMES_ROUTE, principal);
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
        .min(NATIVE_RESOURCE_NAMES_STATE_MAX_BYTES_V1);
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
            let proof_bytes = proof_bytes
                .0
                .checked_add(proof_bytes.1)
                .and_then(|len| len.checked_add(genesis_storage))
                .and_then(|len| len.checked_add(tip_storage))
                // Fixed status/signature/hash metadata is separately bounded;
                // variable committee/key/PoP backing is exact and never charged here.
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
                .with_native_resource_names_snapshot_v1(
                    &tip,
                    &capture_authority,
                    &budget,
                    |snapshot, originals, keys, names| {
                        // Every retained canonical record buffer and borrowed row is
                        // prepaid from the same aggregate operation pool as the snapshot/output.
                        let mut wires =
                            iroha_allocation::ChargedBuffer::new(originals.len(), &budget)
                                .map_err(|error| error.to_string())?;
                        for (_, value) in originals {
                            let length =
                                norito::canonical_frame_len(*value).map_err(|e| e.to_string())?;
                            if length > 1024 * 1024 {
                                return Err("native alias original exceeds bound".into());
                            }
                            let wire = encode(
                                *value,
                                ResponseFormat::Norito,
                                1024 * 1024,
                                &budget,
                                unavailable,
                            )
                            .map_err(|error| error.to_string())?;
                            if wire.as_ref().len() != length {
                                return Err("native alias original length changed".into());
                            }
                            wires.push_reserved(wire);
                        }
                        let mut aliases =
                            iroha_allocation::ChargedBuffer::new(originals.len(), &budget)
                                .map_err(|error| error.to_string())?;
                        for ((key, _), wire) in originals.iter().zip(wires.as_slice()) {
                            aliases.push_reserved(NativeAssetAliasBindingOriginalRefV1::new(
                                key,
                                wire.as_ref(),
                            ));
                        }
                        let mut sns = iroha_allocation::ChargedBuffer::new(names.len(), &budget)
                            .map_err(|error| error.to_string())?;
                        for (key, value) in names {
                            if value.len() > 1024 * 1024 {
                                return Err("native SNS original exceeds bound".into());
                            }
                            sns.push_reserved(NativeDataspaceSnsOriginalRefV1::new(
                                key,
                                value.as_slice(),
                            ));
                        }
                        let payload = NativeResourceNamesStateRefV1::new(
                            &attestation,
                            snapshot,
                            aliases.as_slice(),
                            keys,
                            sns.as_slice(),
                        );
                        encode(&payload, format, max_response, &budget, unavailable)
                            .map_err(|_| "native names state serialization refused".to_owned())
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
    // Authorization is current, not an archived grant. A revocation while
    // work was queued or encoded refuses before any private body is delivered.
    require_full_ledger_carrier_permission(&app, &account)?;
    let mut response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        RESOURCE_NAMES_ROUTE,
        response,
        true,
    )
    .await?;
    require_full_ledger_carrier_permission(&app, &account)?;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

fn finalize_resource_names_response(result: Result<AxResponse, Error>) -> AxResponse {
    let mut response = finalize_bridge_finality_attestation_response(result);
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn validate_resource_names_request_target(
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    challenge_literal: &str,
) -> Result<[u8; 32], Error> {
    let challenge = bridge_finality_challenge(headers)?;
    if method != &Method::GET
        || uri.query().is_some()
        || challenge_literal != hex::encode(challenge)
        || uri.path() != format!("{NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1}{challenge_literal}")
    {
        return Err(conversion_error(
            "resource names request must bind its exact canonical challenge in the signed URI"
                .into(),
        ));
    }
    Ok(challenge)
}

pub(super) fn native_committee_original_bytes<'a>(
    expected: usize,
    members: impl IntoIterator<Item = (&'a iroha_crypto::PublicKey, &'a [u8])>,
) -> Result<usize, Error> {
    // proof_committee retains a tuple Vec; build_proof collects FinalityValidator.
    // Do not assume allocator reuse of two different element layouts. Both Vec
    // geometries and each independently cloned compact key/PoP are prepaid.
    let tuples = std::alloc::Layout::array::<(iroha_crypto::PublicKey, Vec<u8>)>(expected)
        .map_err(|_| capacity())?
        .size();
    let final_values = std::alloc::Layout::array::<
        iroha_data_model::sumeragi_finality::FinalityValidator,
    >(expected)
    .map_err(|_| capacity())?
    .size();
    let mut bytes = tuples.checked_add(final_values).ok_or_else(capacity)?;
    let mut seen = 0usize;
    for (key, pop) in members {
        seen = seen.checked_add(1).ok_or_else(capacity)?;
        if seen > expected {
            return Err(capacity());
        }
        bytes = bytes
            .checked_add(key.retained_allocation_layout().size())
            .and_then(|value| value.checked_add(pop.len()))
            .ok_or_else(capacity)?;
    }
    if seen != expected {
        return Err(capacity());
    }
    Ok(bytes)
}

#[cfg(test)]
mod resource_names_route_tests {
    use super::*;
    #[test]
    fn outer_names_success_and_error_responses_keep_private_no_store() {
        for response in [
            finalize_resource_names_response(Ok(AxResponse::new(Body::empty()))),
            finalize_resource_names_response(Err(unavailable())),
        ] {
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::CACHE_CONTROL)
                    .unwrap(),
                "private, no-store"
            );
        }
    }
    #[test]
    fn names_challenge_is_an_exact_signed_uri_component_not_an_unsigned_header() {
        let value = "ab".repeat(32);
        let uri: Uri = format!("{NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1}{value}")
            .parse()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            BRIDGE_FINALITY_CHALLENGE_HEADER,
            HeaderValue::from_str(&value).unwrap(),
        );
        assert_eq!(
            validate_resource_names_request_target(&headers, &Method::GET, &uri, &value).unwrap(),
            [0xab; 32]
        );
        for altered in [
            format!("{uri}?challenge={value}"),
            format!("{uri}?a=1&a=2"),
            format!(
                "{}{}",
                NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1,
                "02".repeat(32)
            ),
            format!(
                "{}%30{}",
                NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1,
                &value[1..]
            ),
        ] {
            assert!(
                validate_resource_names_request_target(
                    &headers,
                    &Method::GET,
                    &altered.parse().unwrap(),
                    &value
                )
                .is_err()
            );
        }
        assert!(
            validate_resource_names_request_target(&headers, &Method::POST, &uri, &value).is_err()
        );
        assert!(
            validate_resource_names_request_target(
                &headers,
                &Method::GET,
                &uri,
                &value.to_uppercase()
            )
            .is_err()
        );
        headers.append(
            BRIDGE_FINALITY_CHALLENGE_HEADER,
            HeaderValue::from_str(&value).unwrap(),
        );
        assert!(
            validate_resource_names_request_target(&headers, &Method::GET, &uri, &value).is_err()
        );
    }
    #[test]
    fn variable_committees_are_fully_prepaid_in_the_original_pool() {
        let key =
            iroha_crypto::KeyPair::from_seed(vec![71; 32], iroha_crypto::Algorithm::BlsNormal);
        let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap();
        let bytes = native_committee_original_bytes(
            1024,
            std::iter::repeat_n((key.public_key(), pop.as_slice()), 1024),
        )
        .unwrap();
        assert!(
            bytes > 16 * 1024,
            "fixed metadata overhead is insufficient for supported large committees"
        );
        let budget = AllocationBudget::new(16 * 1024);
        assert!(budget.try_reserve_bytes(bytes).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        let budget = AllocationBudget::new(bytes);
        let charge = budget.try_reserve_bytes(bytes).unwrap();
        assert_eq!(budget.reserved_bytes(), bytes);
        drop(charge);
        assert_eq!(budget.reserved_bytes(), 0);
        assert!(native_committee_original_bytes(2, [(key.public_key(), pop.as_slice())]).is_err());
        assert!(native_committee_original_bytes(0, [(key.public_key(), pop.as_slice())]).is_err());
        assert!(native_committee_original_bytes(usize::MAX, std::iter::empty()).is_err());
    }
}
