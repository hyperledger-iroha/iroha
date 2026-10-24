//! Challenge-bound complete native resource-name originals for an authorized certified-original reader.
//!
//! This route publishes data under current permission, never a permission grant or
//! finality/ownership fact. Every original is borrowed from one certified applied cut
//! and output keeps the aggregate finite pool through egress.

use super::*;
use crate::certified_originals_auth::{authenticate, require_permission};
use crate::native_projection_response::{
    NativeProjectionError, capacity, current_root_tip, encode, native_committee_original_bytes,
};
use iroha_core::{
    state::{AllocationBudget, StateReadOnly, WorldStateSnapshotError},
    sumeragi::{
        certified_chain::{CertifiedChain, CommittedBlock},
        finality::AttestationBuildError,
    },
};
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_JOURNAL_BYTES, NativeFinalityLimits,
    },
};
use iroha_torii_shared::resource_names_state::{
    NATIVE_RESOURCE_NAMES_STATE_MAX_BYTES_V1, NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1,
    NativeAssetAliasBindingOriginalRefV1, NativeDataspaceSnsOriginalRefV1,
    NativeResourceNamesStateRefV1,
};
const ROUTE: &str = "/v1/ledger/resource-names/{challenge}";

// This helper is also exercised with exact three-frame limits (genesis, target
// and parent), so a full historical certificate walk cannot regress unnoticed.
fn current_names_tip(
    view: &impl StateReadOnly,
    height: u64,
    root_scope: SumeragiRootScope,
    limits: NativeFinalityLimits,
) -> Result<CommittedBlock, Error> {
    current_root_tip(view, height, root_scope, limits, unavailable).map_err(|error| {
        let reason = match &error {
            NativeProjectionError::ResponseLimit { .. } => "response_bound",
            NativeProjectionError::Buffer(_) => "buffer",
            NativeProjectionError::Encoding(_) => "encoding",
            NativeProjectionError::RootScope(_) => "root_scope_read",
            NativeProjectionError::History(_) => "history_or_certificate",
            NativeProjectionError::Invalid(_) => "invalid_geometry",
            NativeProjectionError::Unavailable(_) => "cut_or_root_mismatch",
        };
        refused("recent_tip", reason)
    })
}

fn require_status_height(
    status: &iroha_data_model::sumeragi::SumeragiStatus,
    height: u64,
) -> Result<(), Error> {
    if status.applied_height != height || status.committed_height != height {
        return Err(refused("driver", "status_height_mismatch"));
    }
    Ok(())
}

fn attestation_refusal_reason(error: &AttestationBuildError) -> &'static str {
    match error {
        AttestationBuildError::EmptyState => "empty_state",
        AttestationBuildError::HeightOverflow => "height_overflow",
        AttestationBuildError::HeightIsNotDurableTip { .. } => "tip_changed",
        AttestationBuildError::StatusHeightMismatch => "status_height_mismatch",
        AttestationBuildError::RestartRequired => "restart_required",
        AttestationBuildError::InvalidStatus => "invalid_status",
        AttestationBuildError::InvalidSigner => "invalid_signer",
        AttestationBuildError::FinalityProof(_) => "tip_proof",
        AttestationBuildError::GenesisFinalityProof(_) => "genesis_proof",
        AttestationBuildError::InvalidBody(_) => "invalid_body",
        AttestationBuildError::ClockUnavailable => "clock_unavailable",
        AttestationBuildError::Signing(_) => "signing",
    }
}

fn snapshot_refusal_reason(error: &WorldStateSnapshotError) -> &'static str {
    match error {
        WorldStateSnapshotError::Encoding(_) => "encoding",
        WorldStateSnapshotError::View(_) => "view",
        WorldStateSnapshotError::Acquisition(_) => "overlay_acquisition",
        WorldStateSnapshotError::Allocation(_) => "allocation",
        WorldStateSnapshotError::Buffer(_) => "buffer",
        WorldStateSnapshotError::Allocator(_) => "allocator",
        WorldStateSnapshotError::DecodeControl(_) => "decode_control",
        WorldStateSnapshotError::RootScopeRead(_) => "root_scope_read",
        WorldStateSnapshotError::Amx(_) => "amx_original",
        WorldStateSnapshotError::CutChanged { .. } => "cut_changed",
        WorldStateSnapshotError::Invalid(_) => "invalid_cut_or_original",
    }
}

fn refused(stage: &'static str, reason: &'static str) -> Error {
    // Fixed classifications only: no private originals, account, request,
    // challenge, URL, credentials or arbitrary error text enter diagnostics.
    iroha_logger::warn!(stage, reason, "native resource names current cut refused");
    unavailable()
}

// Only the original physical World writer can wake this snapshot attempt.
// No StateView or failed World overlay is retained while parked. The capture
// closure must keep the same certified tip, challenge, attestation and budget;
// every retry reruns the original cut, permission and publication fences.
fn snapshot_after_busy<T>(
    budget: &AllocationBudget,
    deadline: std::time::Instant,
    mut capture: impl FnMut() -> Result<T, WorldStateSnapshotError>,
) -> Result<T, WorldStateSnapshotError> {
    use iroha_allocation::release::ReleaseRegistration;
    use std::task::{Context, Waker};

    // Admit the physical waiter before the first snapshot attempt. If it cannot
    // fit, preserve the original one-attempt behavior and its exact refusal.
    let mut registration = budget
        .try_reserve(ReleaseRegistration::allocation_layout())
        .ok()
        .and_then(|mut prepaid| ReleaseRegistration::from_reservation(&mut prepaid).ok());
    if let Some(registration) = &mut registration {
        registration.bind_thread(std::thread::current());
    }
    loop {
        let error = match capture() {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        let WorldStateSnapshotError::Acquisition(mv::storage::AdmittedStorageError::Busy {
            release,
            ..
        }) = &error
        else {
            return Err(error);
        };
        let Some(registration) = &mut registration else {
            return Err(error);
        };
        let mut context = Context::from_waker(Waker::noop());
        loop {
            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                registration.cancel();
                return Err(error);
            };
            if remaining.is_zero() {
                registration.cancel();
                return Err(error);
            }
            if registration.poll_wait(release, &mut context).is_ready() {
                registration.cancel();
                break;
            }
            // The registration wakes this exact blocking worker. Spurious
            // wakes recheck the same release; neither wait nor retry resets time.
            std::thread::park_timeout(remaining);
        }
        if std::time::Instant::now() >= deadline {
            return Err(error);
        }
    }
}

/// Full-original carrier requires real native signed read authority; API-token access alone never suffices.
pub async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::Path(challenge): axum::extract::Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let result = handle(app, challenge, headers, method, uri, remote).await;
    Ok(finalize(result))
}

async fn handle(
    app: SharedAppState,
    challenge_literal: String,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    remote: std::net::SocketAddr,
) -> Result<AxResponse, Error> {
    // Reuse the existing query admission allowance, measured once at route
    // entry. The caller's own request/freshness deadline is never extended.
    let snapshot_deadline = std::time::Instant::now()
        .checked_add(query_admission_remaining(&app))
        .ok_or_else(capacity)?;
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let challenge = validate_request_target(&headers, &method, &uri, &challenge_literal)?;
    let account = authenticate(&app, &headers, &method, &uri, &[])?;
    let capture_authority = account.clone();
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
    let memory = crate::acquire_query_pool_credit(&app, bytes)
        .await
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(bytes);
    let max_response = app
        .torii_proxy_max_response_bytes
        .min(NATIVE_RESOURCE_NAMES_STATE_MAX_BYTES_V1);
    let driver = app
        .sumeragi
        .as_ref()
        .ok_or_else(|| refused("driver", "absent"))?
        .clone();
    let identity = driver.identity().clone();
    let fingerprint = iroha_crypto::Hash::new_from_chunks(&[
        app.build_status.version.as_bytes(),
        app.build_status.git_commit_sha.as_bytes(),
    ]);
    let state = app.state.clone();
    let signer = app.torii_proxy_bridge_signer.clone();
    let response = routing::run_admitted_blocking(
        admission,
        "native resource names state worker failed",
        move || {
            let _memory = memory;
            let view = state.view();
            let height =
                u64::try_from(view.height()).map_err(|_| refused("view", "height_overflow"))?;
            if height < 2 {
                return Err(refused("view", "no_certified_tip"));
            }
            // Sample the actual driver after this worker opens its immutable
            // view, rather than before a potentially long blocking-worker queue.
            // A publication race still refuses; the attestation builder repeats
            // the complete identity, instance and height checks after proof work.
            if driver.restart_required() {
                return Err(refused("driver", "restart_required"));
            }
            let status = driver
                .status_dto()
                .ok_or_else(|| refused("driver", "status_absent"))?;
            if status.unanchored || status.abstaining || status.halted.is_some() {
                return Err(refused("driver", "not_active"));
            }
            if status.signer.as_ref() != Some(identity.node_id.public_key()) {
                return Err(refused("driver", "signer_mismatch"));
            }
            require_status_height(&status, height)?;
            let chain =
                CertifiedChain::new(&view).map_err(|_| refused("chain", "source_unavailable"))?;
            // The original State execution ancestry supplies the target's
            // parent schedule. Reverify the exact target quorum and signed
            // availability, without replaying every historical quorum for each
            // current read. Keep one finite source/decode owner through output.
            let unit = bytes / 8;
            let limits = NativeFinalityLimits {
                block_bytes: unit.min(NATIVE_FINALITY_MAX_BLOCK_BYTES),
                journal_bytes: (unit * 2).min(NATIVE_FINALITY_MAX_JOURNAL_BYTES),
                block_count: 8,
                allocated_bytes: unit * 4,
            };
            let decode_limits = limits.decode_limits().map_err(|_| capacity())?;
            let _source_charge = budget
                .try_reserve_bytes(limits.journal_bytes + limits.allocated_bytes)
                .map_err(|_| capacity())?;
            let tip = norito::core::with_decode_limits_scope(decode_limits, || {
                current_names_tip(&view, height, chain.root_scope(), limits)
            })?;
            // Proof frame copies and their finite committee/signer vectors are
            // prepaid from the same operation budget before the existing builder.
            let genesis = chain
                .committed(1)
                .map_err(|_| refused("genesis", "source_unavailable"))?;
            let proof_bytes = norito::canonical_frame_len(genesis.block().as_ref())
                .and_then(|len| {
                    norito::canonical_frame_len(tip.block().as_ref()).map(|tip_len| (len, tip_len))
                })
                .map_err(|_| refused("proof_backing", "invalid_frame"))?;
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
            .map_err(|error| refused("attestation", attestation_refusal_reason(&error)))?;
            drop(chain);
            drop(view);
            let mut body = snapshot_after_busy(&budget, snapshot_deadline, || {
                state.with_native_resource_names_snapshot_v1(
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
                            let wire = super::native_projection_response::encode_canonical(
                                *value,
                                1024 * 1024,
                                &budget,
                            )
                            .map_err(|error| error.to_string())?;
                            wires.push_reserved(wire);
                        }
                        let mut aliases =
                            iroha_allocation::ChargedBuffer::new(originals.len(), &budget)
                                .map_err(|error| error.to_string())?;
                        for ((key, _), wire) in originals.iter().zip(wires.as_slice()) {
                            aliases.push_reserved(NativeAssetAliasBindingOriginalRefV1::new(
                                key,
                                wire.as_slice(),
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
                        encode(&payload, format, max_response, &budget)
                            .map_err(|_| "native names state serialization refused".to_owned())
                    },
                )
            })
            .map_err(|error| refused("snapshot", snapshot_refusal_reason(&error)))?;
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
    require_permission(&app, &account)?;
    let mut response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await?;
    require_permission(&app, &account)?;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

fn finalize(result: Result<AxResponse, Error>) -> AxResponse {
    let mut response = finalize_bridge_finality_attestation_response(result);
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn validate_request_target(
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    challenge_literal: &str,
) -> Result<[u8; 32], Error> {
    let challenge = bridge_finality_challenge(headers)?;
    if method != Method::GET
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

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "native_resource_names_state_unavailable",
        message: "Current certified native resource names state is unavailable.".into(),
    }
}

#[cfg(test)]
#[path = "resource_names_state/recent_tip_tests.rs"]
mod recent_tip_tests;

#[cfg(test)]
#[path = "resource_names_state/snapshot_wait_tests.rs"]
mod snapshot_wait_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outer_names_success_and_error_responses_keep_private_no_store() {
        for response in [
            finalize(Ok(AxResponse::new(Body::empty()))),
            finalize(Err(unavailable())),
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
    fn unavailable_names_state_reports_its_own_service_code() {
        assert!(matches!(
            unavailable(),
            Error::AppServiceUnavailable {
                code: "native_resource_names_state_unavailable",
                ..
            }
        ));
        assert_eq!(
            finalize(Err(unavailable())).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    #[test]
    fn names_refusal_diagnostics_preserve_fixed_service_rejection() {
        assert_eq!(
            attestation_refusal_reason(&AttestationBuildError::StatusHeightMismatch),
            "status_height_mismatch"
        );
        assert_eq!(
            attestation_refusal_reason(&AttestationBuildError::Signing("private input".into())),
            "signing"
        );
        assert_eq!(
            snapshot_refusal_reason(&WorldStateSnapshotError::CutChanged {
                requested_height: 2,
                observed_height: 3,
            }),
            "cut_changed"
        );
        assert_eq!(
            snapshot_refusal_reason(&WorldStateSnapshotError::Invalid("private original".into())),
            "invalid_cut_or_original"
        );
        assert_eq!(
            finalize(Err(refused("snapshot", "cut_changed"))).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    #[test]
    fn rate_limit_and_egress_route_is_the_catalogued_names_template() {
        assert_eq!(
            ROUTE,
            format!("{NATIVE_RESOURCE_NAMES_STATE_ROUTE_PREFIX_V1}{{challenge}}")
        );
        assert_eq!(
            ROUTE,
            iroha_torii_shared::route_catalog::core::RESOURCE_NAMES_STATE.path()
        );
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
            validate_request_target(&headers, &Method::GET, &uri, &value).unwrap(),
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
                validate_request_target(&headers, &Method::GET, &altered.parse().unwrap(), &value)
                    .is_err()
            );
        }
        assert!(validate_request_target(&headers, &Method::POST, &uri, &value).is_err());
        assert!(
            validate_request_target(&headers, &Method::GET, &uri, &value.to_uppercase()).is_err()
        );
        headers.append(
            BRIDGE_FINALITY_CHALLENGE_HEADER,
            HeaderValue::from_str(&value).unwrap(),
        );
        assert!(validate_request_target(&headers, &Method::GET, &uri, &value).is_err());
    }
}
