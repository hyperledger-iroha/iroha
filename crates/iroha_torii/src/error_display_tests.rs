//! Exact error rendering and source contracts, independent of the derive helper.

use std::{error::Error as StdError, fmt, fmt::Write as _};

use super::*;

/// Check message bytes, fresh nested formatting, and writer error propagation.
pub(super) fn assert_display(error: &impl fmt::Display, expected: &str) {
    assert_eq!(error.to_string(), expected);
    assert_eq!(format!("{error:>120.3}"), expected);
    assert_eq!(format!("{error:🦀^120.3}"), expected);
    assert_eq!(format!("{error:+#0120.3}"), expected);

    struct FailingWriter;
    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    assert!(write!(FailingWriter, "{error}").is_err());
}

/// Check that `source` borrows the original field rather than another carrier.
pub(super) fn assert_source(error: &dyn StdError, expected: Option<&(dyn StdError + 'static)>) {
    match (error.source(), expected) {
        (Some(actual), Some(expected)) => {
            assert!(
                std::ptr::addr_eq(actual, expected),
                "source identity for {error}"
            );
            assert_eq!(actual.to_string(), expected.to_string());
        }
        (None, None) => {}
        _ => panic!("unexpected error source for {error}"),
    }
}

#[test]
fn torii_error_display_preserves_every_variant_and_source() {
    let native = norito::core::classify_decode_attempt(|| {
        Err::<(), _>(norito::Error::AllocationFailed { bytes: 17 })
    })
    .unwrap_err()
    .into_local_refusal()
    .unwrap();
    let native_message = format!("Original native evidence decode is locally refused: {native}");
    let cases = [
        (
            Error::Query(iroha_data_model::ValidationFail::NotPermitted(
                "source μ\n".into(),
            )),
            "Failed to process query",
        ),
        (Error::NativeEvidenceDecode(native), native_message.as_str()),
        (
            Error::AppQueryValidation {
                code: "code",
                message: "μ\n".into(),
            },
            "Validation error for app-facing query parameters `code`: μ\n",
        ),
        #[cfg(feature = "app_api")]
        (
            Error::CollectionQuery(Box::new(collections::CollectionError::new(
                "code", "filter", "μ\n",
            ))),
            "Invalid collection query: μ\n",
        ),
        #[cfg(feature = "app_api")]
        (
            Error::AccountOnboardingValidation {
                code: "code",
                message: "μ\n".into(),
                hint: Some("quote\"\n"),
            },
            "Account onboarding validation failed `code`: μ\n; hint: Some(\"quote\\\"\\n\")",
        ),
        (
            Error::AppUnauthorized {
                code: "code",
                message: "μ\n".into(),
            },
            "Authentication is required for app-facing operation `code`: μ\n",
        ),
        (
            Error::AppForbidden {
                code: "code",
                message: "μ\n".into(),
            },
            "Forbidden error for app-facing operation `code`: μ\n",
        ),
        (
            Error::AppNotFound {
                code: "code",
                message: "μ\n".into(),
            },
            "Not-found error for app-facing operation `code`: μ\n",
        ),
        (
            Error::AppConflict {
                code: "code",
                message: "μ\n".into(),
            },
            "Conflict error for app-facing operation `code`: μ\n",
        ),
        (
            Error::AppServiceUnavailable {
                code: "code",
                message: "μ\n".into(),
            },
            "Temporarily unavailable app-facing operation `code`: μ\n",
        ),
        (
            Error::HistoryReadExceedsBudget {
                resource: "bytes",
                required: u64::MAX,
                limit: 0,
            },
            "Committed history read needs at least 18446744073709551615 bytes, above this request's own budget 0",
        ),
        (
            Error::ProofRateLimited {
                endpoint: "/proof",
                retry_after_secs: u64::MAX,
            },
            "Proof endpoint `/proof` throttled the request; retry after `18446744073709551615s`",
        ),
        (
            Error::AcceptTransaction(
                iroha_core::tx::AcceptTransactionFail::UnexpectedGenesisAccountSignature,
            ),
            "Failed to accept transaction",
        ),
        (
            Error::TransactionAdmissionDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
            ),
            "Local readiness or resources prevented transaction admission; no acceptance or rejection was produced.",
        ),
        (
            Error::Config(eyre::eyre!("source μ\n")),
            "Failed to get or set configuration",
        ),
        #[cfg(feature = "app_api")]
        (
            Error::SorafsModerationStartup { code: "code" },
            "`SoraFS` moderation runtime startup failed `code`",
        ),
        #[cfg(feature = "app_api")]
        (
            Error::SorafsEvidenceViewerStartup { code: "code" },
            "`SoraFS` evidence-viewer runtime startup failed `code`",
        ),
        (
            Error::SerializationFailure {
                context: "body",
                source: Box::new(std::io::Error::other("source μ\n")),
            },
            "Failed to serialize response payload for `body`: source μ\n",
        ),
        (
            Error::LaneLifecycle {
                reason: "μ\n".into(),
            },
            "Failed to apply Nexus lane lifecycle plan: μ\n",
        ),
        (
            Error::PushIntoQueue {
                source: Box::new(queue::Error::Full),
                backpressure: queue::BackpressureState::Saturated {
                    queued: 7,
                    capacity: std::num::NonZeroUsize::new(7).unwrap(),
                },
            },
            "Failed to push into queue (Queue is full; backpressure=Saturated { queued: 7, capacity: 7 })",
        ),
        #[cfg(feature = "telemetry")]
        (
            Error::Prometheus(eyre::eyre!("source μ\n")),
            "Failed to get Prometheus metrics",
        ),
        #[cfg(feature = "profiling")]
        (
            Error::Pprof(eyre::eyre!("source μ\n")),
            "Failed to get pprof profile",
        ),
        #[cfg(feature = "telemetry")]
        (
            Error::StatusFailure(eyre::eyre!("source μ\n")),
            "Failed to get status",
        ),
        #[cfg(feature = "telemetry")]
        (
            Error::TelemetryProfileRestricted {
                endpoint: "/metrics",
                profile: TelemetryProfile::Disabled,
            },
            "Telemetry endpoint /metrics disabled by active profile Disabled",
        ),
        (
            Error::ConfigurationFailure(KisoError::Validation("source μ\n".into())),
            "Failure caused by configuration subsystem",
        ),
        (Error::StartServer, "Failed to start Torii"),
        (Error::FailedExit, "Torii server terminated with an error"),
    ];
    for (error, expected) in cases {
        assert_display(&error, expected);
        let source: Option<&(dyn StdError + 'static)> = match &error {
            Error::Query(source) => Some(source),
            Error::NativeEvidenceDecode(source) => Some(source),
            Error::AcceptTransaction(source) => Some(source),
            Error::TransactionAdmissionDeferred(source) => Some(source),
            Error::Config(source) => Some(source.as_ref()),
            Error::SerializationFailure { source, .. } => Some(source.as_ref()),
            Error::PushIntoQueue { source, .. } => Some(source),
            #[cfg(feature = "telemetry")]
            Error::Prometheus(source) | Error::StatusFailure(source) => Some(source.as_ref()),
            #[cfg(feature = "profiling")]
            Error::Pprof(source) => Some(source.as_ref()),
            Error::ConfigurationFailure(source) => Some(source),
            #[cfg(feature = "app_api")]
            Error::CollectionQuery(_)
            | Error::AccountOnboardingValidation { .. }
            | Error::SorafsModerationStartup { .. }
            | Error::SorafsEvidenceViewerStartup { .. } => None,
            #[cfg(feature = "telemetry")]
            Error::TelemetryProfileRestricted { .. } => None,
            Error::AppQueryValidation { .. }
            | Error::AppUnauthorized { .. }
            | Error::AppForbidden { .. }
            | Error::AppNotFound { .. }
            | Error::AppConflict { .. }
            | Error::AppServiceUnavailable { .. }
            | Error::HistoryReadExceedsBudget { .. }
            | Error::ProofRateLimited { .. }
            | Error::LaneLifecycle { .. }
            | Error::StartServer
            | Error::FailedExit => None,
        };
        assert_source(&error, source);
    }
}

#[test]
fn torii_error_from_keeps_variants_and_original_payloads() {
    let query: Error = iroha_data_model::ValidationFail::NotPermitted("query μ".into()).into();
    assert!(
        matches!(query, Error::Query(iroha_data_model::ValidationFail::NotPermitted(ref message)) if message == "query μ")
    );
    let config: Error = KisoError::Validation("config μ".into()).into();
    assert!(
        matches!(config, Error::ConfigurationFailure(KisoError::Validation(ref message)) if message == "config μ")
    );
}

#[test]
fn stream_error_display_preserves_every_variant_sources_and_from() {
    use crate::stream::Error;
    let cases = [
        (Error::ReadTimeout, "Read message timeout"),
        (Error::SendTimeout, "Send message timeout"),
        (
            Error::WebSocket(axum::Error::new(std::io::Error::other("socket μ\n"))),
            "WebSocket error: socket μ\n",
        ),
        (
            Error::Decode(norito::Error::NonCanonicalEncoding),
            "Error during Norito message decoding",
        ),
        (
            Error::Encode(norito::Error::NonCanonicalEncoding),
            "Error during Norito message encoding",
        ),
        (
            Error::UnexpectedFrame {
                expected: "binary",
                actual: "text μ\n",
            },
            "Unexpected WebSocket frame `text μ\n`; expected binary",
        ),
        (Error::Closed, "Connection is closed"),
    ];
    for (error, expected) in cases {
        assert_display(&error, expected);
        let source: Option<&(dyn StdError + 'static)> = match &error {
            Error::WebSocket(source) => Some(source),
            Error::Decode(source) => Some(source),
            Error::ReadTimeout
            | Error::SendTimeout
            | Error::Encode(_)
            | Error::UnexpectedFrame { .. }
            | Error::Closed => None,
        };
        assert_source(&error, source);
    }
    let error: Error = norito::Error::NonCanonicalEncoding.into();
    assert!(matches!(
        error,
        Error::Decode(norito::Error::NonCanonicalEncoding)
    ));
}

#[test]
#[cfg(feature = "app_api")]
fn filter_error_display_preserves_both_variants_without_sources() {
    use crate::filter::ValidateError;
    for (error, expected) in [
        (
            ValidateError::UnsupportedField("metadata.μ\n".into()),
            "unsupported field path: metadata.μ\n",
        ),
        (
            ValidateError::TypeMismatch("metadata.μ\n".into()),
            "type mismatch at field: metadata.μ\n",
        ),
    ] {
        assert_display(&error, expected);
        assert_source(&error, None);
    }
}
