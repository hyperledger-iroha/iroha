//! Shared policy helpers for SoraFS orchestrator tooling.
use std::fmt;

/// Transport policy applied when selecting providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportPolicy {
    /// Prefer SoraNet relays while keeping direct transports as a fallback. This is now the default
    /// multi-source posture across the workspace.
    #[default]
    SoranetPreferred,
    /// Require SoraNet transport and refuse direct providers.
    SoranetStrict,
    /// Force direct transport (Torii/QUIC) only. Use this as an explicit downgrade when relays are
    /// unhealthy or a compliance policy mandates direct mode.
    DirectOnly,
}

impl TransportPolicy {
    /// Returns the canonical label for this policy.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SoranetPreferred => "soranet-first",
            Self::SoranetStrict => "soranet-strict",
            Self::DirectOnly => "direct-only",
        }
    }

    /// Parses a [`TransportPolicy`] from textual input.
    pub fn parse(label: &str) -> Option<Self> {
        let token = label.trim().to_ascii_lowercase();
        match token.as_str() {
            "soranet_first" | "soranet-first" => Some(Self::SoranetPreferred),
            "soranet_strict" | "soranet-strict" | "soranet_only" | "soranet-only" => {
                Some(Self::SoranetStrict)
            }
            "direct_only" | "direct-only" => Some(Self::DirectOnly),
            _ => None,
        }
    }
}

impl fmt::Display for TransportPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Staged anonymity policy enforced for SoraNet paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::enum_variant_names)]
pub enum AnonymityPolicy {
    /// Require at least one PQ-capable guard (Stage A).
    #[default]
    GuardPq,
    /// Prefer PQ-capable relays for ≥ two thirds of hops (Stage B).
    MajorityPq,
    /// Enforce PQ-only paths; fall back to direct transports otherwise (Stage C).
    StrictPq,
}

impl AnonymityPolicy {
    /// Returns the canonical label for this policy.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::GuardPq => "anon-guard-pq",
            Self::MajorityPq => "anon-majority-pq",
            Self::StrictPq => "anon-strict-pq",
        }
    }

    /// Parses an [`AnonymityPolicy`] from textual input (accepts stage aliases).
    pub fn parse(label: &str) -> Option<Self> {
        let token = label.trim().to_ascii_lowercase();
        match token.as_str() {
            "anon_guard_pq" | "anon-guard-pq" | "stage_a" | "stage-a" | "stagea" => {
                Some(Self::GuardPq)
            }
            "anon_majority_pq" | "anon-majority-pq" | "stage_b" | "stage-b" | "stageb" => {
                Some(Self::MajorityPq)
            }
            "anon_strict_pq" | "anon-strict-pq" | "stage_c" | "stage-c" | "stagec" => {
                Some(Self::StrictPq)
            }
            _ => None,
        }
    }
}

impl fmt::Display for AnonymityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Summary describing effective and override policy labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyLabelSummary {
    /// Label applied after considering overrides.
    pub effective_label: &'static str,
    /// Whether an override flag was provided.
    pub override_flag: bool,
    /// Label supplied by the override, if any.
    pub override_label: Option<&'static str>,
}

fn build_policy_labels<T: Copy + Default>(
    requested: Option<T>,
    override_policy: Option<T>,
    label_fn: fn(T) -> &'static str,
) -> PolicyLabelSummary {
    let override_flag = override_policy.is_some();
    let override_label = override_policy.map(label_fn);
    let effective = override_policy.unwrap_or_else(|| requested.unwrap_or_default());
    PolicyLabelSummary {
        effective_label: label_fn(effective),
        override_flag,
        override_label,
    }
}

/// Returns the label summary for a transport policy pair.
#[must_use]
pub fn transport_policy_labels(
    requested: Option<TransportPolicy>,
    override_policy: Option<TransportPolicy>,
) -> PolicyLabelSummary {
    build_policy_labels(requested, override_policy, TransportPolicy::label)
}

/// Returns the label summary for an anonymity policy pair.
#[must_use]
pub fn anonymity_policy_labels(
    requested: Option<AnonymityPolicy>,
    override_policy: Option<AnonymityPolicy>,
) -> PolicyLabelSummary {
    build_policy_labels(requested, override_policy, AnonymityPolicy::label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_policy_parse_accepts_aliases() {
        assert_eq!(
            TransportPolicy::parse("soranet-first"),
            Some(TransportPolicy::SoranetPreferred)
        );
        assert_eq!(
            TransportPolicy::parse("Soranet_Strict"),
            Some(TransportPolicy::SoranetStrict)
        );
        assert_eq!(
            TransportPolicy::parse("DIRECT_ONLY"),
            Some(TransportPolicy::DirectOnly)
        );
        assert_eq!(TransportPolicy::parse("unknown"), None);
    }

    #[test]
    fn anonymity_policy_parse_accepts_aliases() {
        assert_eq!(
            AnonymityPolicy::parse("stage-a"),
            Some(AnonymityPolicy::GuardPq)
        );
        assert_eq!(
            AnonymityPolicy::parse("stage_b"),
            Some(AnonymityPolicy::MajorityPq)
        );
        assert_eq!(
            AnonymityPolicy::parse("ANON-STRICT-PQ"),
            Some(AnonymityPolicy::StrictPq)
        );
        assert_eq!(AnonymityPolicy::parse("nope"), None);
    }

    #[test]
    fn policy_label_summary_prefers_override() {
        let summary = transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
        assert_eq!(summary.effective_label, "soranet-first");
        assert!(!summary.override_flag);
        assert!(summary.override_label.is_none());

        let summary = transport_policy_labels(
            Some(TransportPolicy::SoranetPreferred),
            Some(TransportPolicy::DirectOnly),
        );
        assert_eq!(summary.effective_label, "direct-only");
        assert!(summary.override_flag);
        assert_eq!(summary.override_label, Some("direct-only"));
    }
}

#[cfg(feature = "manifest")]
mod compliance {
    use std::sync::Arc;

    use reqwest::StatusCode;
    use thiserror::Error;

    use crate::{
        ChunkFetchSpec,
        gateway::{
            GATEWAY_COMPLIANCE_DENIED_CODE, GatewayFailureEvidence, GatewayFetchError,
            GatewayFetcher, is_canonical_catalog_digest_hex,
            is_canonical_gateway_compliance_source,
        },
        multi_fetch::FetchRequest,
    };

    /// Parsed and validated policy evidence returned by a gateway.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PolicyEvidence {
        /// Exact catalog-backed denial returned by the gateway.
        pub evidence: GatewayFailureEvidence,
    }

    /// Validation errors surfaced while interpreting policy evidence.
    #[derive(Debug, Error)]
    pub enum PolicyValidationError {
        /// The wire status was not exactly HTTP 451.
        #[error("policy status mismatch (expected 451, got {actual})")]
        Status { actual: StatusCode },
        /// The body did not carry the exact V1 denial code.
        #[error("policy code is not gateway_compliance_denied")]
        Code,
        /// The body named a source that cannot deny under the V1 precedence rules.
        #[error("policy source is not a canonical denying source")]
        Source,
        /// The body catalog digest was not canonical lowercase 32-byte hex.
        #[error("policy catalog digest is not canonical lowercase 32-byte hex")]
        CatalogDigest,
        /// The caller supplied a noncanonical catalog digest expectation.
        #[error("expected policy catalog digest is not canonical lowercase 32-byte hex")]
        ExpectedCatalogDigest,
        /// The denial was evaluated under a different governed catalog.
        #[error("policy catalog digest mismatch (expected {expected}, got {actual})")]
        CatalogDigestMismatch { expected: String, actual: String },
    }

    /// Validator for policy evidence emitted by gateways.
    #[derive(Debug, Clone, Copy)]
    pub struct PolicyEvidenceValidator<'a> {
        expected_catalog_digest_hex: Option<&'a str>,
    }

    impl<'a> Default for PolicyEvidenceValidator<'a> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<'a> PolicyEvidenceValidator<'a> {
        /// Construct a validator requiring the exact canonical V1 denial envelope.
        #[must_use]
        pub fn new() -> Self {
            Self {
                expected_catalog_digest_hex: None,
            }
        }

        /// Require an exact governed catalog digest in the gateway evidence.
        #[must_use]
        pub fn with_expected_catalog_digest(mut self, digest_hex: &'a str) -> Self {
            self.expected_catalog_digest_hex = Some(digest_hex);
            self
        }

        /// Validate exact catalog-backed policy evidence.
        pub fn validate(
            &self,
            evidence: GatewayFailureEvidence,
        ) -> Result<PolicyEvidence, PolicyValidationError> {
            if evidence.observed_status != StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS {
                return Err(PolicyValidationError::Status {
                    actual: evidence.observed_status,
                });
            }
            if evidence.code != GATEWAY_COMPLIANCE_DENIED_CODE {
                return Err(PolicyValidationError::Code);
            }
            if !is_canonical_gateway_compliance_source(&evidence.source) {
                return Err(PolicyValidationError::Source);
            }
            if !is_canonical_catalog_digest_hex(&evidence.catalog_digest_hex) {
                return Err(PolicyValidationError::CatalogDigest);
            }
            if let Some(expected) = self.expected_catalog_digest_hex {
                if !is_canonical_catalog_digest_hex(expected) {
                    return Err(PolicyValidationError::ExpectedCatalogDigest);
                }
                if evidence.catalog_digest_hex != expected {
                    return Err(PolicyValidationError::CatalogDigestMismatch {
                        expected: expected.to_owned(),
                        actual: evidence.catalog_digest_hex.clone(),
                    });
                }
            }
            Ok(PolicyEvidence { evidence })
        }
    }

    /// Errors surfaced while probing gateways with honey tokens.
    #[derive(Debug, Error)]
    pub enum HoneyProbeError {
        #[error("provider `{provider}` returned success for honey probe")]
        UnexpectedSuccess { provider: String },
        #[error("provider `{provider}` returned unexpected failure: {error}")]
        UnexpectedFetch {
            provider: String,
            #[source]
            error: Box<GatewayFetchError>,
        },
        #[error("provider `{provider}` policy evidence failed validation: {error}")]
        Validation {
            provider: String,
            #[source]
            error: PolicyValidationError,
        },
    }

    /// Policy evidence captured for a gateway during a honey probe.
    #[derive(Debug, Clone)]
    pub struct HoneyProbeReport {
        pub provider_id: String,
        pub policy: PolicyEvidence,
    }

    /// Execute a honey probe against all configured providers, expecting a policy block.
    pub async fn run_honey_probe(
        fetcher: &GatewayFetcher,
        providers: &[crate::multi_fetch::FetchProvider],
        spec: &ChunkFetchSpec,
        validator: &PolicyEvidenceValidator<'_>,
    ) -> Result<Vec<HoneyProbeReport>, HoneyProbeError> {
        let mut reports = Vec::new();
        for provider in providers {
            let request = FetchRequest {
                provider: Arc::new(provider.clone()),
                spec: spec.clone(),
                attempt: 1,
            };
            let evidence = match fetcher.fetch(request).await {
                Err(GatewayFetchError::PolicyBlocked { evidence, .. }) => evidence,
                Err(error) => {
                    return Err(HoneyProbeError::UnexpectedFetch {
                        provider: provider.id().as_str().to_string(),
                        error: Box::new(error),
                    });
                }
                Ok(_) => {
                    return Err(HoneyProbeError::UnexpectedSuccess {
                        provider: provider.id().as_str().to_string(),
                    });
                }
            };
            let policy =
                validator
                    .validate(evidence)
                    .map_err(|error| HoneyProbeError::Validation {
                        provider: provider.id().as_str().to_string(),
                        error,
                    })?;
            reports.push(HoneyProbeReport {
                provider_id: provider.id().as_str().to_string(),
                policy,
            });
        }
        Ok(reports)
    }
}

#[cfg(feature = "manifest")]
pub use compliance::*;

#[cfg(all(feature = "manifest", test))]
mod compliance_tests {
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use base64::Engine as _;
    use blake3;
    use ed25519_dalek::SigningKey;
    use reqwest::{StatusCode, header::HeaderMap};
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{STREAM_TOKEN_MAX_TTL_SECS_V1, StreamTokenBodyV1, StreamTokenV1};

    use super::*;
    use crate::{
        CarBuildPlan, ChunkFetchSpec,
        gateway::{
            GatewayFailureEvidence, GatewayFetchConfig, GatewayFetchContext, GatewayProviderInput,
            HttpEngine, HttpError, HttpFuture, HttpRequest, HttpResponse,
        },
        policy::{PolicyEvidenceValidator, run_honey_probe},
    };

    const CATALOG_DIGEST_HEX: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn sample_payload(len: usize) -> Vec<u8> {
        (0..len).map(|idx| (idx % 251) as u8).collect()
    }

    fn sample_stream_token(
        manifest_cid_hex: &str,
        provider_id_hex: &str,
        profile: &str,
        max_streams: u16,
    ) -> StreamTokenV1 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_secs();
        StreamTokenV1::sign(
            StreamTokenBodyV1 {
                token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_string(),
                manifest_cid: hex::decode(manifest_cid_hex).expect("cid hex"),
                provider_id: {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&hex::decode(provider_id_hex).expect("provider hex"));
                    bytes
                },
                profile_handle: profile.to_string(),
                max_streams,
                ttl_epoch: now + STREAM_TOKEN_MAX_TTL_SECS_V1,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: now,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            &SigningKey::from_bytes(&[0x42; 32]),
        )
        .expect("sign stream token")
    }

    fn encode_token_b64(token: &StreamTokenV1) -> String {
        let bytes = norito::to_bytes(token).expect("encode token");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn provider_id_hex() -> String {
        "ab".repeat(32)
    }

    #[derive(Clone)]
    struct StubEngine {
        responses: HashMap<String, HttpResponse>,
    }

    impl StubEngine {
        fn new(responses: HashMap<String, HttpResponse>) -> Self {
            Self { responses }
        }
    }

    impl HttpEngine for StubEngine {
        fn get(&self, request: HttpRequest) -> HttpFuture {
            let path = request.url.path().to_string();
            let maybe = self.responses.get(&path).cloned();
            Box::pin(async move {
                maybe.ok_or_else(|| {
                    HttpError::Stub(format!("no stubbed response registered for {path}"))
                })
            })
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn honey_probe_requires_exact_catalog_backed_denial() {
        let payload = sample_payload(2048);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let spec: ChunkFetchSpec = plan.try_chunk_fetch_specs().expect("valid CAR plan")[0].clone();
        let manifest_id_hex = hex::encode(blake3::hash(&payload).as_bytes());
        let provider_id = provider_id_hex();
        let chunker_handle = "sorafs.sf1@1.0.0".to_string();
        let stream_token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 2);
        let stream_token_b64 = encode_token_b64(&stream_token);

        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(spec.digest)
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                headers: HeaderMap::new(),
                body: format!(
                    r#"{{"error":"gateway_compliance_denied","source":"baseline","catalog_digest_hex":"{CATALOG_DIGEST_HEX}"}}"#
                )
                .into_bytes(),
            },
        );
        let engine = Arc::new(StubEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
        };
        let provider = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            gateway_public_key_hex: hex::encode(
                SigningKey::from_bytes(&[0x42; 32])
                    .verifying_key()
                    .to_bytes(),
            ),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64,
            privacy_events_url: None,
        };
        let context = GatewayFetchContext::build_with_engine(config, [provider], engine)
            .expect("gateway fetch context builds");

        let validator =
            PolicyEvidenceValidator::new().with_expected_catalog_digest(CATALOG_DIGEST_HEX);

        let reports = run_honey_probe(&context.fetcher(), &context.providers(), &spec, &validator)
            .await
            .expect("probe succeeds");
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.provider_id, "alpha");
        assert_eq!(
            report.policy.evidence.catalog_digest_hex,
            CATALOG_DIGEST_HEX
        );
        assert_eq!(report.policy.evidence.source, "baseline");
    }

    fn policy_evidence(source: &str, catalog_digest_hex: &str) -> GatewayFailureEvidence {
        GatewayFailureEvidence {
            observed_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            code: "gateway_compliance_denied".to_owned(),
            source: source.to_owned(),
            catalog_digest_hex: catalog_digest_hex.to_owned(),
        }
    }

    #[test]
    fn validator_accepts_only_denying_sources_and_exact_catalog() {
        let validator =
            PolicyEvidenceValidator::new().with_expected_catalog_digest(CATALOG_DIGEST_HEX);
        for source in ["baseline", "legal_safety_hold"] {
            let validated = validator
                .validate(policy_evidence(source, CATALOG_DIGEST_HEX))
                .expect("canonical denial validates");
            assert_eq!(validated.evidence.source, source);
        }

        for source in ["no_match", "accepted_appeal", "unknown"] {
            let error = validator
                .validate(policy_evidence(source, CATALOG_DIGEST_HEX))
                .expect_err("non-denying source must fail");
            assert!(matches!(error, PolicyValidationError::Source));
        }
    }

    #[test]
    fn validator_rejects_noncanonical_or_unexpected_catalog() {
        let validator =
            PolicyEvidenceValidator::new().with_expected_catalog_digest(CATALOG_DIGEST_HEX);

        let mut wrong_status = policy_evidence("baseline", CATALOG_DIGEST_HEX);
        wrong_status.observed_status = StatusCode::FORBIDDEN;
        assert!(matches!(
            validator.validate(wrong_status),
            Err(PolicyValidationError::Status { .. })
        ));

        let mut wrong_code = policy_evidence("baseline", CATALOG_DIGEST_HEX);
        wrong_code.code = "denylisted".to_owned();
        assert!(matches!(
            validator.validate(wrong_code),
            Err(PolicyValidationError::Code)
        ));

        let uppercase = CATALOG_DIGEST_HEX.to_ascii_uppercase();
        assert!(matches!(
            validator.validate(policy_evidence("baseline", &uppercase)),
            Err(PolicyValidationError::CatalogDigest)
        ));

        assert!(matches!(
            validator.validate(policy_evidence("baseline", &"ab".repeat(32))),
            Err(PolicyValidationError::CatalogDigestMismatch { .. })
        ));

        let malformed_expectation =
            PolicyEvidenceValidator::new().with_expected_catalog_digest("not-a-digest");
        assert!(matches!(
            malformed_expectation.validate(policy_evidence("baseline", CATALOG_DIGEST_HEX)),
            Err(PolicyValidationError::ExpectedCatalogDigest)
        ));
    }
}
