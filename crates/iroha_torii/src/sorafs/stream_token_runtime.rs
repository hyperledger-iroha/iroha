//! Strict Torii ownership checks for the production stream-token runtime.

use std::sync::Arc;

use iroha_config::parameters::actual::{SorafsTokenConfig, Torii as ToriiConfig};
use iroha_data_model::{NetworkId, sorafs::reputation::derive_stream_token_gateway_id_v1};

use super::{
    StreamTokenAdmissionCaptureV1, StreamTokenGatewayAdmissionQualificationV1, StreamTokenIssuer,
    StreamTokenRuntimeSigner,
};

/// Reject missing, unexpected, or drifting production admission ownership.
pub(crate) fn preflight_admission_capture(
    network_id: &NetworkId,
    config: &ToriiConfig,
    runtime_deps: &crate::ToriiRuntimeDeps,
) -> Result<(), String> {
    let tokens = &config.sorafs_storage.stream_tokens;
    let capture = runtime_deps.sorafs_stream_token_admission_capture.as_ref();
    if !tokens.enabled {
        return if capture.is_none() {
            Ok(())
        } else {
            Err(
                "stream-token admission capture is unexpected while stream tokens are disabled"
                    .to_owned(),
            )
        };
    }

    let capture = capture.ok_or_else(|| {
        "enabled stream tokens require a deployment-owned admission capture".to_owned()
    })?;
    let handle = tokens
        .admission_provider_handle
        .as_deref()
        .ok_or_else(|| "enabled stream tokens require an admission provider handle".to_owned())?;
    let revision = tokens
        .admission_provider_revision
        .ok_or_else(|| "enabled stream tokens require an admission provider revision".to_owned())?;
    let policy_digest = tokens.admission_provider_policy_digest.ok_or_else(|| {
        "enabled stream tokens require an admission provider policy digest".to_owned()
    })?;
    let compliance = config.sorafs_gateway.compliance.as_ref().ok_or_else(|| {
        "enabled stream tokens require a governed gateway compliance identity".to_owned()
    })?;
    let gateway_id = derive_stream_token_gateway_id_v1(network_id, &compliance.gateway_id)
        .map_err(|_| "configured stream-token gateway identity is invalid".to_owned())?;
    let qualification = StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id,
        revision,
        policy_digest,
        max_pending: tokens.admission_max_pending,
        max_tracked_tokens: tokens.admission_max_tracked_tokens,
        lease_ttl_ms: tokens.admission_lease_ttl_ms,
    };
    capture
        .validate_expected_binding(
            handle,
            qualification,
            tokens.admission_reconcile_max_items,
        )
        .map_err(|_| {
            "stream-token admission capture is substituted, stale, test-marked, unavailable, or unstable"
                .to_owned()
        })
}

/// Construct the optional issuer after signer qualification.
pub(crate) fn build_issuer(
    config: &SorafsTokenConfig,
    api_tokens: &[String],
    signer: Option<Arc<dyn StreamTokenRuntimeSigner>>,
) -> Option<Arc<StreamTokenIssuer>> {
    match StreamTokenIssuer::from_config(config, api_tokens, signer) {
        Ok(issuer) => issuer.map(Arc::new),
        Err(error) => panic!("invalid SoraFS stream token configuration: {error}"),
    }
}

impl crate::ToriiRuntimeDeps {
    /// Attach the runtime-only HSM/KMS signer used for stream-token issuance.
    #[must_use]
    pub fn with_sorafs_stream_token_signer(
        mut self,
        signer: Arc<dyn StreamTokenRuntimeSigner>,
    ) -> Self {
        self.sorafs_stream_token_signer = Some(signer);
        self
    }

    /// Attach the qualified deployment-owned stream-token admission capture.
    #[must_use]
    pub fn with_sorafs_stream_token_admission_capture(
        mut self,
        capture: Arc<StreamTokenAdmissionCaptureV1>,
    ) -> Self {
        self.sorafs_stream_token_admission_capture = Some(capture);
        self
    }
}

impl crate::AppState {
    /// Clone the production stream-token admission capture.
    pub(crate) fn stream_token_admission_capture(
        &self,
    ) -> Option<Arc<StreamTokenAdmissionCaptureV1>> {
        self.stream_token_admission_capture.clone()
    }

    /// Clone the configured stream-token issuer.
    pub(crate) fn stream_token_issuer(&self) -> Option<Arc<StreamTokenIssuer>> {
        self.stream_token_issuer.clone()
    }

    #[cfg(test)]
    pub(crate) fn stream_token_concurrency(&self) -> &super::StreamTokenConcurrencyTracker {
        &self.stream_token_concurrency
    }

    #[cfg(test)]
    pub(crate) fn stream_token_quota(&self) -> &super::StreamTokenQuotaTracker {
        &self.stream_token_quota
    }
}
