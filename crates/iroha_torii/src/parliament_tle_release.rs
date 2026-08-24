//! Authenticated local partial-release service for Parliament timed opening.
//!
//! Routing and canonical account authentication are mounted in `lib.rs`. This
//! module owns only identifier validation, blocking runtime isolation, Core
//! authorization, and payload-free failure mapping.

use std::sync::Arc;

use axum::body::Body;
use iroha_core::tle_release::{
    TlePartialReleaseShareV1, TleReleaseAuthorizationErrorV1, TleReleaseCoordinatorErrorV1,
    TleReleaseCoordinatorV1,
};
use iroha_data_model::governance::types::BallotAttemptId;

/// Consume at most one byte and enforce the partial-release zero-body contract.
///
/// This must be called only after canonical authentication. Using the raw body
/// here avoids a buffering extractor running before the access gate, while the
/// one-byte limit rejects headerless HTTP/2 bodies without admitting an
/// unbounded payload.
pub(crate) async fn require_empty_body_v1(body: Body) -> Result<(), crate::Error> {
    match axum::body::to_bytes(body, 1).await {
        Ok(bytes) if bytes.is_empty() => Ok(()),
        Ok(_) | Err(_) => Err(crate::Error::AppQueryValidation {
            code: "parliament_tle_partial_release_body_not_empty",
            message: "Parliament TLE partial-release requests must have an empty body".to_owned(),
        }),
    }
}

/// Parse one exact, nonzero Parliament ballot-attempt identifier.
fn parse_ballot_attempt_id(value: &str) -> Result<BallotAttemptId, crate::Error> {
    let ballot_attempt_id = value.parse::<BallotAttemptId>().map_err(|_| {
        crate::routing::conversion_error(
            "ballot_attempt_id must be exactly 64 lowercase hexadecimal characters".to_owned(),
        )
    })?;
    if ballot_attempt_id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(crate::routing::conversion_error(
            "ballot_attempt_id must be non-zero".to_owned(),
        ));
    }
    Ok(ballot_attempt_id)
}

fn map_coordinator_error(error: TleReleaseCoordinatorErrorV1) -> crate::Error {
    match error {
        TleReleaseCoordinatorErrorV1::Authorization(
            TleReleaseAuthorizationErrorV1::MissingTimedOvnEvidence
            | TleReleaseAuthorizationErrorV1::MissingGovernanceAttempt
            | TleReleaseAuthorizationErrorV1::MissingBallot,
        ) => crate::Error::AppNotFound {
            code: "parliament_tle_release_not_found",
            message: "the requested Parliament TLE release state was not found".to_owned(),
        },
        TleReleaseCoordinatorErrorV1::Authorization(_) => crate::Error::AppConflict {
            code: "parliament_tle_release_not_authorized",
            message: "Core did not authorize a partial release from committed state".to_owned(),
        },
        TleReleaseCoordinatorErrorV1::SignerUnavailable => crate::Error::AppServiceUnavailable {
            code: "parliament_tle_release_signer_unavailable",
            message: "this node has no available Parliament TLE release signer".to_owned(),
        },
        TleReleaseCoordinatorErrorV1::SignerFailed => crate::Error::AppServiceUnavailable {
            code: "parliament_tle_release_signer_failed",
            message: "the Parliament TLE release signer could not produce a share".to_owned(),
        },
        TleReleaseCoordinatorErrorV1::InvalidSignerOutput => crate::Error::AppServiceUnavailable {
            code: "parliament_tle_release_signer_invalid_output",
            message: "the Parliament TLE release signer returned an invalid share".to_owned(),
        },
        TleReleaseCoordinatorErrorV1::InvalidPartialSet
        | TleReleaseCoordinatorErrorV1::InvalidFinalRelease => crate::Error::AppConflict {
            code: "parliament_tle_release_invalid_partial_set",
            message: "the Parliament TLE release partial set is invalid".to_owned(),
        },
    }
}

/// Request this node's proof-carrying partial for one committed ballot.
///
/// The route wrapper must enforce canonical account request authentication and
/// per-account admission before calling this function. It accepts no request
/// body and no caller-selected release identity, height, key session, participant
/// seat, or transcript. Core reconstructs those values from a point-in-time
/// committed query view before the runtime signer is invoked.
///
/// Signer work runs on the blocking pool because production implementations may
/// synchronously enter PKCS#11, an HSM, or a local KMS bridge. The coordinator
/// independently verifies the returned public proof before this function returns.
///
/// # Errors
///
/// Returns a stable payload-free Torii error for an invalid identifier, rejected
/// Core authorization, unavailable signer, blocking-worker failure, or invalid
/// signer output.
pub(crate) async fn request_local_partial_release_v1(
    state: Arc<iroha_core::state::State>,
    coordinator: Arc<TleReleaseCoordinatorV1>,
    ballot_attempt_id: String,
    signer_admission: crate::QueryAdmissionPermit,
) -> Result<TlePartialReleaseShareV1, crate::Error> {
    let ballot_attempt_id = parse_ballot_attempt_id(&ballot_attempt_id)?;
    tokio::task::spawn_blocking(move || {
        // Keep physical signer capacity reserved even if the HTTP future is
        // cancelled and dropping its JoinHandle detaches this blocking task.
        let _signer_admission = signer_admission;
        let view = state.query_view();
        coordinator.request_partial_release(&view, ballot_attempt_id)
    })
    .await
    .map_err(|_| crate::Error::AppServiceUnavailable {
        code: "parliament_tle_release_worker_unavailable",
        message: "the Parliament TLE release worker is unavailable".to_owned(),
    })?
    .map_err(map_coordinator_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method, Request, header::CONTENT_LENGTH};

    #[test]
    fn identifier_parser_rejects_noncanonical_and_zero_ids() {
        assert!(parse_ballot_attempt_id("not-a-ballot").is_err());
        assert!(parse_ballot_attempt_id(&"00".repeat(32)).is_err());
        assert!(parse_ballot_attempt_id(&"01".repeat(32)).is_ok());
    }

    #[test]
    fn coordinator_failure_mapping_exposes_only_closed_error_classes() {
        let provider_error = map_coordinator_error(TleReleaseCoordinatorErrorV1::SignerFailed);
        let rendered = provider_error.to_string();
        assert!(rendered.contains("parliament_tle_release_signer_failed"));
        assert!(!rendered.contains("handle"));
        assert!(!rendered.contains("share-metadata"));

        let early = map_coordinator_error(TleReleaseCoordinatorErrorV1::Authorization(
            TleReleaseAuthorizationErrorV1::ReleaseHeightNotReached,
        ));
        assert!(matches!(early, crate::Error::AppConflict { .. }));
        let missing = map_coordinator_error(TleReleaseCoordinatorErrorV1::Authorization(
            TleReleaseAuthorizationErrorV1::MissingTimedOvnEvidence,
        ));
        assert!(matches!(missing, crate::Error::AppNotFound { .. }));
    }

    #[test]
    fn route_passes_bounded_admission_into_the_physical_signer_task() {
        let route_source = include_str!("lib.rs");
        let handler = route_source
            .split("async fn handler_gov_parliament_tle_partial_release")
            .nth(1)
            .and_then(|tail| tail.split("async fn handler_gov_citizen_status").next())
            .expect("partial-release route source");
        let access = handler
            .find("check_access(")
            .expect("canonical access gate");
        let body = handler
            .find("require_empty_body_v1(request.into_body()).await?;")
            .expect("bounded zero-body gate");
        let admission = handler
            .find("let signer_admission = acquire_query_admission(app.as_ref(), true).await?;")
            .expect("heavy admission gate");
        assert!(access < body && body < admission);
        assert!(handler.contains("ballot_attempt_id,\n        signer_admission,\n    )"));

        let service_source = include_str!("parliament_tle_release.rs");
        assert!(service_source.contains("let _signer_admission = signer_admission;"));
    }

    #[tokio::test]
    async fn zero_body_gate_accepts_the_normal_empty_request() {
        require_empty_body_v1(Body::empty())
            .await
            .expect("empty request body");
    }

    #[tokio::test]
    async fn zero_body_gate_rejects_headerless_http2_style_content() {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/gov/parliament/ballots/test/partial-release")
            .body(Body::from("x"))
            .expect("headerless streaming-style request");
        assert!(request.headers().get(CONTENT_LENGTH).is_none());
        let error = require_empty_body_v1(request.into_body())
            .await
            .expect_err("nonempty body must fail without Content-Length");
        assert!(matches!(
            error,
            crate::Error::AppQueryValidation {
                code: "parliament_tle_partial_release_body_not_empty",
                ..
            }
        ));
    }
}
