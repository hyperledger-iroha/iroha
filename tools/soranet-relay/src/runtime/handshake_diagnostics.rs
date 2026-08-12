// Included from `runtime.rs` to preserve these handshake diagnostics in their original scope.

fn downgrade_detail_from_warnings(warnings: &[CapabilityWarning]) -> Option<String> {
    let slug_source = warnings
        .iter()
        .find_map(|warning| {
            let trimmed = warning.message.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or("downgrade");
    Some(normalize_downgrade_reason(slug_source))
}

fn record_handshake_suite_downgrade(metrics: &Metrics, suite: HandshakeSuite) {
    if matches!(suite, HandshakeSuite::Nk3PqForwardSecure) {
        metrics.record_downgrade("handshake_suite_nk3");
    }
}

fn pow_failure_reason(error: &pow::Error) -> SoranetPowFailureReasonV1 {
    match error {
        pow::Error::UnsupportedVersion(_) => SoranetPowFailureReasonV1::UnsupportedVersion,
        pow::Error::DifficultyMismatch { .. } => SoranetPowFailureReasonV1::DifficultyMismatch,
        pow::Error::Expired(_, _) => SoranetPowFailureReasonV1::Expired,
        pow::Error::FutureSkewExceeded(_) => SoranetPowFailureReasonV1::FutureSkewExceeded,
        pow::Error::ExpiryTimestampOverflow(_) => SoranetPowFailureReasonV1::ClockError,
        pow::Error::ExpiryWindowTooSmall(_) => SoranetPowFailureReasonV1::TtlTooShort,
        pow::Error::InvalidSolution => SoranetPowFailureReasonV1::InvalidSolution,
        pow::Error::RelayMismatch | pow::Error::TranscriptMismatch => {
            SoranetPowFailureReasonV1::RelayMismatch
        }
        pow::Error::Replay => SoranetPowFailureReasonV1::Replay,
        pow::Error::RevocationStore(_) => SoranetPowFailureReasonV1::StoreError,
        pow::Error::InvalidSignature | pow::Error::Signing(_) => {
            SoranetPowFailureReasonV1::SignatureInvalid
        }
        pow::Error::PostQuantum(_) => SoranetPowFailureReasonV1::PostQuantumError,
        pow::Error::Clock(_) => SoranetPowFailureReasonV1::ClockError,
        pow::Error::Malformed(_) | pow::Error::MalformedBinding(_) => {
            SoranetPowFailureReasonV1::UnsupportedVersion
        }
    }
}
