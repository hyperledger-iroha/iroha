//! Deterministic ACME automation harness for the SoraFS gateway.

use std::time::{Duration, SystemTime};

use blake3::Hasher;
use thiserror::Error;

/// Challenge profile describing which ACME flows must be exercised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeProfile {
    /// Whether DNS-01 challenges should be solved.
    pub dns01: bool,
    /// Whether TLS-ALPN-01 challenges should be solved.
    pub tls_alpn_01: bool,
}

impl Default for ChallengeProfile {
    fn default() -> Self {
        Self {
            dns01: true,
            tls_alpn_01: true,
        }
    }
}

/// Static configuration for ACME automation.
#[derive(Clone, Debug)]
pub struct AcmeConfig {
    /// Toggle automation behaviour.
    pub enabled: bool,
    /// Account email registered with the ACME provider.
    pub account_email: Option<String>,
    /// Directory URL (Let’s Encrypt, staging, custom).
    pub directory_url: String,
    /// Hostnames covered by the certificate.
    pub hostnames: Vec<String>,
    /// Identifier of the DNS provider plugin used for DNS-01 challenges.
    pub dns_provider_id: Option<String>,
    /// Renewal window applied before certificate expiry.
    pub renewal_window: Duration,
    /// Backoff applied after failures.
    pub retry_backoff: Duration,
    /// Maximum jitter applied to retry scheduling.
    pub retry_jitter: Duration,
    /// Challenge profile to exercise.
    pub challenge: ChallengeProfile,
}

impl Default for AcmeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            account_email: None,
            directory_url: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
            hostnames: vec![],
            dns_provider_id: None,
            renewal_window: Duration::from_hours(30 * 24), // 30 days
            retry_backoff: Duration::from_mins(30),        // 30 minutes
            retry_jitter: Duration::from_mins(5),          // up to 5 minutes
            challenge: ChallengeProfile::default(),
        }
    }
}

/// Certificate and key material emitted by ACME renewals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateBundle {
    /// PEM-encoded certificate chain.
    pub certificate_pem: String,
    /// PEM-encoded private key.
    pub private_key_pem: String,
    /// Optional ECH config blob emitted with the certificate.
    pub ech_config: Option<Vec<u8>>,
    /// Expiry timestamp of the certificate.
    pub not_after: SystemTime,
}

/// Order descriptor passed to the ACME client implementation.
#[derive(Clone, Debug)]
pub struct CertificateOrder {
    /// Hostnames covered in the order.
    pub hostnames: Vec<String>,
    /// Account email derived from config.
    pub account_email: Option<String>,
    /// ACME directory URL.
    pub directory_url: String,
    /// DNS provider selected for DNS-01 automation.
    pub dns_provider_id: Option<String>,
    /// Challenge profile to satisfy.
    pub challenge: ChallengeProfile,
}

/// Errors emitted by the ACME client implementation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AcmeClientError {
    /// Endpoint rejected the request permanently.
    #[error("acme order rejected: {reason}")]
    Rejected {
        /// Human-readable reason.
        reason: String,
    },
    /// ACME server signalled a transient failure with optional retry hint.
    #[error("acme order temporarily rejected")]
    Temporary {
        /// Optional retry-after duration.
        retry_after: Option<Duration>,
    },
    /// Underlying transport or cryptographic failure.
    #[error("acme transport error: {reason}")]
    Transport {
        /// Reason string for diagnostics.
        reason: String,
    },
}

/// Trait implemented by ACME client backends.
pub trait AcmeClient {
    /// Place an order and return the resulting certificate bundle.
    ///
    /// # Errors
    ///
    /// Returns [`AcmeClientError`] when the ACME backend cannot fulfil the order.
    fn order_certificate(
        &self,
        order: &CertificateOrder,
    ) -> Result<CertificateBundle, AcmeClientError>;
}

/// Errors surfaced by the automation harness.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AcmeAutomationError {
    /// ACME client failure.
    #[error("acme client error: {0}")]
    Client(AcmeClientError),
}

#[derive(Debug, Default, Clone)]
struct AcmeState {
    bundle: Option<CertificateBundle>,
    last_success: Option<SystemTime>,
    last_error: Option<AcmeClientError>,
    next_attempt: Option<SystemTime>,
    attempts: u32,
}

impl AcmeState {
    fn record_success(&mut self, bundle: CertificateBundle, now: SystemTime) {
        self.bundle = Some(bundle);
        self.last_success = Some(now);
        self.last_error = None;
        self.attempts = 0;
        self.next_attempt = None;
    }

    fn record_failure(&mut self, error: AcmeClientError, retry_at: SystemTime) {
        self.last_error = Some(error);
        self.attempts = self.attempts.saturating_add(1);
        self.next_attempt = Some(retry_at);
    }

    fn certificate(&self) -> Option<&CertificateBundle> {
        self.bundle.as_ref()
    }
}

/// Deterministic ACME automation harness.
#[derive(Debug)]
pub struct AcmeAutomation<C> {
    config: AcmeConfig,
    client: C,
    state: AcmeState,
}

impl<C> AcmeAutomation<C>
where
    C: AcmeClient,
{
    /// Construct an automation harness.
    #[must_use]
    pub fn new(config: AcmeConfig, client: C) -> Self {
        Self {
            config,
            client,
            state: AcmeState::default(),
        }
    }

    /// Access the last known certificate bundle.
    #[must_use]
    pub fn certificate(&self) -> Option<&CertificateBundle> {
        self.state.certificate()
    }

    /// Returns the last error recorded by the automation loop.
    #[must_use]
    pub fn last_error(&self) -> Option<&AcmeClientError> {
        self.state.last_error.as_ref()
    }

    /// Execute the automation loop. Returns `Some(bundle)` when a renewal succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`AcmeAutomationError`] when the ACME backend reports a failure.
    pub fn process(
        &mut self,
        now: SystemTime,
    ) -> Result<Option<CertificateBundle>, AcmeAutomationError> {
        if !self.config.enabled {
            return Ok(None);
        }

        if let Some(next_attempt) = self.state.next_attempt {
            if now < next_attempt {
                return Ok(None);
            }
        }

        if !self.needs_renewal(now) {
            return Ok(None);
        }

        let order = CertificateOrder {
            hostnames: self.config.hostnames.clone(),
            account_email: self.config.account_email.clone(),
            directory_url: self.config.directory_url.clone(),
            dns_provider_id: self.config.dns_provider_id.clone(),
            challenge: self.config.challenge.clone(),
        };

        match self.client.order_certificate(&order) {
            Ok(bundle) => {
                let deadline = self.next_deadline(&bundle);
                self.state.record_success(bundle.clone(), now);
                self.state.next_attempt = Some(deadline);
                Ok(Some(bundle))
            }
            Err(err) => {
                let retry_after = match err {
                    AcmeClientError::Temporary { retry_after } => retry_after,
                    _ => None,
                };
                let jitter = self.compute_jitter();
                let backoff_base = retry_after.unwrap_or(self.config.retry_backoff);
                let retry_at = now + backoff_base + jitter.min(self.config.retry_jitter);
                self.state.record_failure(err.clone(), retry_at);
                Err(AcmeAutomationError::Client(err))
            }
        }
    }

    fn needs_renewal(&self, now: SystemTime) -> bool {
        self.state.bundle.as_ref().map_or(true, |bundle| {
            bundle
                .not_after
                .checked_sub(self.config.renewal_window)
                .map_or(true, |renew_at| now >= renew_at)
        })
    }

    fn next_deadline(&self, bundle: &CertificateBundle) -> SystemTime {
        bundle
            .not_after
            .checked_sub(self.config.renewal_window)
            .unwrap_or(bundle.not_after)
    }

    fn compute_jitter(&self) -> Duration {
        if self.config.retry_jitter.is_zero() {
            return Duration::ZERO;
        }
        let mut hasher = Hasher::new();
        for host in &self.config.hostnames {
            hasher.update(host.as_bytes());
        }
        hasher.update(&self.state.attempts.to_le_bytes());
        let hash = hasher.finalize();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&hash.as_bytes()[..8]);
        let spread = u64::from_le_bytes(buf);
        let max_ns = self
            .config
            .retry_jitter
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        if max_ns == 0 {
            return Duration::ZERO;
        }
        let jitter_ns = spread % (max_ns + 1);
        Duration::from_nanos(jitter_ns)
    }

    #[cfg(test)]
    /// Returns the scheduled next attempt for test verification.
    pub fn next_attempt(&self) -> Option<SystemTime> {
        self.state.next_attempt
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct MockClient {
        responses: RefCell<Vec<Result<CertificateBundle, AcmeClientError>>>,
    }

    impl MockClient {
        fn with_responses(responses: Vec<Result<CertificateBundle, AcmeClientError>>) -> Self {
            Self {
                responses: RefCell::new(responses),
            }
        }
    }

    impl AcmeClient for MockClient {
        fn order_certificate(
            &self,
            _order: &CertificateOrder,
        ) -> Result<CertificateBundle, AcmeClientError> {
            self.responses.borrow_mut().pop().unwrap_or_else(|| {
                Err(AcmeClientError::Transport {
                    reason: "exhausted responses".to_string(),
                })
            })
        }
    }

    fn sample_bundle(valid_for: Duration, now: SystemTime) -> CertificateBundle {
        CertificateBundle {
            certificate_pem: "CERT".to_string(),
            private_key_pem: "KEY".to_string(),
            ech_config: None,
            not_after: now + valid_for,
        }
    }

    #[test]
    fn automation_renews_when_due() {
        let now = SystemTime::now();
        let client =
            MockClient::with_responses(vec![Ok(sample_bundle(Duration::from_hours(90 * 24), now))]);
        let config = AcmeConfig {
            enabled: true,
            hostnames: vec!["gateway.example.com".to_string()],
            ..AcmeConfig::default()
        };
        let mut automation = AcmeAutomation::new(config, client);
        let result = automation.process(now).expect("renewal");
        assert!(result.is_some());
        assert!(automation.certificate().is_some());
    }

    #[test]
    fn automation_waits_until_window() {
        let now = SystemTime::now();
        let bundle = sample_bundle(Duration::from_hours(90 * 24), now);
        let client = MockClient::with_responses(vec![Ok(bundle.clone())]);
        let config = AcmeConfig {
            enabled: true,
            hostnames: vec!["gateway.example.com".to_string()],
            ..AcmeConfig::default()
        };
        let mut automation = AcmeAutomation::new(config, client);
        let initial = automation.process(now).expect("initial renewal");
        assert!(initial.is_some());
        let later = now + Duration::from_hours(10 * 24);
        let result = automation.process(later).expect("no-op");
        assert!(result.is_none());
    }

    #[test]
    fn automation_backoff_on_error() {
        let now = SystemTime::now();
        let client = MockClient::with_responses(vec![Err(AcmeClientError::Temporary {
            retry_after: Some(Duration::from_mins(1)),
        })]);
        let config = AcmeConfig {
            enabled: true,
            retry_backoff: Duration::from_secs(10),
            retry_jitter: Duration::from_secs(0),
            hostnames: vec!["gateway.example.com".to_string()],
            ..AcmeConfig::default()
        };
        let mut automation = AcmeAutomation::new(config, client);
        let err = automation.process(now).expect_err("expected failure");
        assert!(matches!(err, AcmeAutomationError::Client(_)));
        let retry_at = automation.next_attempt().expect("retry scheduled");
        assert!(retry_at >= now + Duration::from_mins(1));
    }
}
