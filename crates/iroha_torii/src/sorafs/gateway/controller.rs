//! ACME automation controller wiring for the SoraFS gateway TLS surface.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::Hasher;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::prelude::*;
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time as tokio_time,
};

#[cfg(feature = "telemetry")]
use super::telemetry::record_renewal_metrics;
use super::{
    acme::{
        AcmeAutomation, AcmeAutomationError, AcmeClient, AcmeClientError, AcmeConfig,
        CertificateBundle, CertificateOrder,
    },
    telemetry::{TlsRenewalResult, TlsStateSnapshot},
};
use crate::routing::MaybeTelemetry;

/// Default automation poll interval (seconds).
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_mins(1);

/// Handle that manages TLS automation and updates gateway telemetry state.
pub struct TlsAutomationHandle {
    automation: Mutex<AcmeAutomation<SelfSignedAcmeClient>>,
    tls_state: Arc<RwLock<TlsStateSnapshot>>,
    hostnames: Vec<String>,
    poll_interval: Duration,
}

impl TlsAutomationHandle {
    /// Construct a new automation handle for the given configuration.
    #[must_use]
    pub fn new(config: AcmeConfig, tls_state: Arc<RwLock<TlsStateSnapshot>>) -> Self {
        let hostnames = config.hostnames.clone();
        let automation = AcmeAutomation::new(config, SelfSignedAcmeClient);
        Self {
            automation: Mutex::new(automation),
            tls_state,
            hostnames,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Spawn the automation loop on the Tokio runtime.
    pub fn spawn(
        self: &Arc<Self>,
        telemetry: MaybeTelemetry,
        shutdown: ShutdownSignal,
    ) -> JoinHandle<()> {
        let handle = Arc::clone(self);
        tokio::spawn(async move {
            handle.run_loop(telemetry, shutdown).await;
        })
    }

    async fn run_loop(self: Arc<Self>, telemetry: MaybeTelemetry, shutdown: ShutdownSignal) {
        self.poll_once(&telemetry).await;

        let mut ticker = tokio_time::interval(self.poll_interval);
        ticker.set_missed_tick_behavior(tokio_time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = shutdown.receive() => {
                    iroha_logger::info!("SoraFS TLS automation loop received shutdown signal");
                    break;
                }
                _ = ticker.tick() => {
                    self.poll_once(&telemetry).await;
                }
            }
        }
    }

    async fn poll_once(&self, telemetry: &MaybeTelemetry) {
        let now = SystemTime::now();
        let mut automation = self.automation.lock().await;
        match automation.process(now) {
            Ok(Some(bundle)) => {
                iroha_logger::info!(
                    ?now,
                    hostnames = ?self.hostnames,
                    "SoraFS TLS automation produced a renewed certificate"
                );
                self.record_success(now, &bundle, telemetry).await;
            }
            Ok(None) => {
                // Not yet time to renew; nothing to log.
            }
            Err(err) => {
                let reason = err.to_string();
                iroha_logger::warn!(?err, hostnames = ?self.hostnames, "SoraFS TLS automation failed");
                self.record_failure(&reason, now, telemetry).await;
            }
        }
    }

    async fn record_success(
        &self,
        now: SystemTime,
        bundle: &CertificateBundle,
        telemetry: &MaybeTelemetry,
    ) {
        #[cfg(feature = "telemetry")]
        let snapshot = {
            let mut guard = self.tls_state.write().await;
            guard.record_success(bundle, now);
            guard.clone()
        };

        #[cfg(not(feature = "telemetry"))]
        {
            let mut guard = self.tls_state.write().await;
            guard.record_success(bundle, now);
        }

        #[cfg(feature = "telemetry")]
        {
            let _ = telemetry.with_metrics(|metrics| {
                record_renewal_metrics(metrics, TlsRenewalResult::Success);
                snapshot.apply_metrics(metrics, now);
            });
        }

        #[cfg(not(feature = "telemetry"))]
        let _ = (now, telemetry);
    }

    async fn record_failure(&self, message: &str, now: SystemTime, telemetry: &MaybeTelemetry) {
        #[cfg(feature = "telemetry")]
        let snapshot = {
            let mut guard = self.tls_state.write().await;
            guard.record_failure(message);
            guard.clone()
        };

        #[cfg(not(feature = "telemetry"))]
        {
            let mut guard = self.tls_state.write().await;
            guard.record_failure(message);
        }

        #[cfg(feature = "telemetry")]
        {
            let _ = telemetry.with_metrics(|metrics| {
                record_renewal_metrics(metrics, TlsRenewalResult::Failure);
                snapshot.apply_metrics(metrics, now);
            });
        }

        #[cfg(not(feature = "telemetry"))]
        let _ = (now, telemetry);
    }

    #[cfg(test)]
    /// Test helper: execute a single automation tick with telemetry disabled.
    pub async fn poll_once_for_tests(&self) {
        self.poll_once(&MaybeTelemetry::disabled()).await;
    }
}

/// Deterministic client that issues self-signed certificates for configured hostnames.
#[derive(Debug, Default, Clone, Copy)]
pub struct SelfSignedAcmeClient;

impl AcmeClient for SelfSignedAcmeClient {
    fn order_certificate(
        &self,
        order: &CertificateOrder,
    ) -> Result<CertificateBundle, AcmeClientError> {
        if order.hostnames.is_empty() {
            return Err(AcmeClientError::Rejected {
                reason: "hostnames list may not be empty".to_string(),
            });
        }
        if order.hostnames.iter().any(|h| h.trim().is_empty()) {
            return Err(AcmeClientError::Rejected {
                reason: "hostname entries may not be blank".to_string(),
            });
        }

        let validity = Duration::from_hours(90 * 24);
        let not_after = now_plus(validity)?;

        let mut hasher = Hasher::new();
        for host in &order.hostnames {
            hasher.update(host.as_bytes());
        }
        if let Some(email) = &order.account_email {
            hasher.update(email.as_bytes());
        }
        hasher.update(order.directory_url.as_bytes());
        let fingerprint = hasher.finalize();
        let fingerprint_hex = hex::encode(fingerprint.as_bytes());

        let expiry_secs = not_after
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cert_body = format!(
            "sorafs-gateway-cert\nversion=1\nhosts={}\nnot_after={}\nfp={}",
            order.hostnames.join(","),
            expiry_secs,
            fingerprint_hex
        );
        let certificate_pem = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            BASE64_STANDARD.encode(cert_body.as_bytes())
        );
        let private_key_pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            BASE64_STANDARD.encode(fingerprint.as_bytes())
        );

        Ok(CertificateBundle {
            certificate_pem,
            private_key_pem,
            ech_config: None,
            not_after,
        })
    }
}

fn now_plus(delta: Duration) -> Result<SystemTime, AcmeClientError> {
    SystemTime::now()
        .checked_add(delta)
        .ok_or_else(|| AcmeClientError::Transport {
            reason: "renewal window overflow".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorafs::gateway::ChallengeProfile;

    fn sample_config() -> AcmeConfig {
        AcmeConfig {
            enabled: true,
            account_email: Some("ops@example.com".to_string()),
            directory_url: "https://acme.invalid/directory".to_string(),
            hostnames: vec!["gw.example.com".to_string()],
            dns_provider_id: None,
            renewal_window: Duration::from_hours(30 * 24),
            retry_backoff: Duration::from_mins(30),
            retry_jitter: Duration::from_mins(5),
            challenge: ChallengeProfile {
                dns01: true,
                tls_alpn_01: true,
            },
        }
    }

    #[tokio::test]
    async fn automation_updates_snapshot_on_success() {
        let tls_state = Arc::new(RwLock::new(TlsStateSnapshot::new(false)));
        let handle = Arc::new(TlsAutomationHandle::new(
            sample_config(),
            Arc::clone(&tls_state),
        ));
        handle.poll_once_for_tests().await;

        let snapshot = tls_state.read().await.clone();
        assert!(snapshot.expiry().is_some(), "expected expiry after renewal");
        assert_eq!(snapshot.last_result(), TlsRenewalResult::Success);
    }

    #[tokio::test]
    async fn automation_records_failure_for_blank_hostnames() {
        let tls_state = Arc::new(RwLock::new(TlsStateSnapshot::new(false)));
        let mut config = sample_config();
        config.hostnames = vec![String::new()];
        let handle = Arc::new(TlsAutomationHandle::new(config, Arc::clone(&tls_state)));
        handle.poll_once_for_tests().await;

        let snapshot = tls_state.read().await.clone();
        assert_eq!(snapshot.last_result(), TlsRenewalResult::Failure);
        assert!(snapshot.header_value().contains("last-error="));
    }
}
