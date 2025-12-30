//! Telemetry helpers for the SoraFS gateway TLS automation surface.

use std::time::{Duration, SystemTime};

#[cfg(feature = "telemetry")]
use iroha_core::telemetry::Telemetry;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::acme::CertificateBundle;

/// Canonical header emitted by the gateway.
pub const SORA_TLS_STATE_HEADER: &str = "x-sora-tls-state";

/// Result classification for TLS renewal attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlsRenewalResult {
    /// No renewal has been attempted yet.
    Unknown,
    /// Renewal completed successfully.
    Success,
    /// Renewal failed.
    Failure,
}

impl TlsRenewalResult {
    #[must_use]
    fn as_label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Snapshot describing the TLS automation state.
#[derive(Clone, Debug)]
pub struct TlsStateSnapshot {
    expiry: Option<SystemTime>,
    ech_enabled: bool,
    last_success: Option<SystemTime>,
    last_error: Option<String>,
    last_result: TlsRenewalResult,
}

impl TlsStateSnapshot {
    /// Construct a new snapshot with ECH state.
    #[must_use]
    pub fn new(ech_enabled: bool) -> Self {
        Self {
            expiry: None,
            ech_enabled,
            last_success: None,
            last_error: None,
            last_result: TlsRenewalResult::Unknown,
        }
    }

    /// Record a successful renewal.
    pub fn record_success(&mut self, bundle: &CertificateBundle, now: SystemTime) {
        self.expiry = Some(bundle.not_after);
        self.last_success = Some(now);
        self.last_error = None;
        self.last_result = TlsRenewalResult::Success;
    }

    /// Record a failed renewal with the provided error message.
    pub fn record_failure(&mut self, message: impl Into<String>) {
        self.last_error = Some(message.into());
        self.last_result = TlsRenewalResult::Failure;
    }

    /// Update the cached ECH state.
    pub fn set_ech_enabled(&mut self, enabled: bool) {
        self.ech_enabled = enabled;
    }

    /// Produce the canonical header value advertised to clients.
    #[must_use]
    pub fn header_value(&self) -> String {
        let mut parts = vec![if self.ech_enabled {
            "ech-enabled".to_string()
        } else {
            "ech-disabled".to_string()
        }];

        if let Some(expiry) = self.expiry {
            if let Some(value) = format_timestamp(expiry) {
                parts.push(format!("expiry={value}"));
            }
        }

        if let Some(renewed) = self.last_success {
            if let Some(value) = format_timestamp(renewed) {
                parts.push(format!("renewed-at={value}"));
            }
        }

        if let Some(err) = &self.last_error {
            parts.push(format!("last-error={}", sanitize(err)));
        }

        parts.join(";")
    }

    /// Expose the expiry timestamp.
    #[must_use]
    pub fn expiry(&self) -> Option<SystemTime> {
        self.expiry
    }

    /// Most recent renewal result.
    #[must_use]
    pub fn last_result(&self) -> TlsRenewalResult {
        self.last_result
    }

    /// Apply the snapshot to the metrics surface.
    #[cfg(feature = "telemetry")]
    pub fn apply_metrics(&self, telemetry: &Telemetry, now: SystemTime) {
        let expiry_seconds = self
            .expiry
            .and_then(|expiry| expiry.duration_since(now).ok());
        telemetry.set_sorafs_tls_state(self.ech_enabled, expiry_seconds);
    }
}

/// Record a renewal result in telemetry metrics.
#[cfg(feature = "telemetry")]
pub fn record_renewal_metrics(telemetry: &Telemetry, result: TlsRenewalResult) {
    telemetry.record_sorafs_tls_renewal(result.as_label());
}

fn format_timestamp(time: SystemTime) -> Option<String> {
    let datetime = OffsetDateTime::from(time).replace_nanosecond(0).ok()?;
    datetime.format(&Rfc3339).ok()
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            ';' | '\n' | '\r' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    #[test]
    fn header_serialisation() {
        let now = SystemTime::now();
        let mut snapshot = TlsStateSnapshot::new(true);
        let bundle = CertificateBundle {
            certificate_pem: String::new(),
            private_key_pem: String::new(),
            ech_config: None,
            not_after: now + Duration::from_hours(1),
        };
        snapshot.record_success(&bundle, now);
        snapshot.record_failure("temporary error\nwith newline");
        let header = snapshot.header_value();
        assert!(header.contains("ech-enabled"));
        assert!(header.contains("expiry="));
        assert!(header.contains("renewed-at="));
        assert!(header.contains("last-error=temporary error_with newline"));
    }
}
