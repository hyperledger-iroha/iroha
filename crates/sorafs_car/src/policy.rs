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
    use std::{fmt, sync::Arc};

    use blake3::Hasher;
    use ed25519_dalek::VerifyingKey;
    use iroha_crypto::sorafs::proof_token::{ProofToken, ProofTokenDigestKey};
    use reqwest::StatusCode;
    use thiserror::Error;

    use crate::{
        ChunkFetchSpec,
        gateway::{GatewayFailureEvidence, GatewayFetchError, GatewayFetcher},
        moderation::ValidatedModerationProof,
        multi_fetch::FetchRequest,
    };

    const POLICY_BINDING_DOMAIN: &[u8] = b"sorafs.policy.binding.v1";

    /// Verifies proof tokens emitted by gateways when policy blocks occur.
    #[derive(Clone)]
    pub struct ProofTokenVerifier {
        verifying_key: VerifyingKey,
        digest_key: Option<ProofTokenDigestKey>,
    }

    impl fmt::Debug for ProofTokenVerifier {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ProofTokenVerifier")
                .field("verifying_key", &"ed25519")
                .field("digest_key", &self.digest_key.as_ref().map(|_| "present"))
                .finish()
        }
    }

    impl ProofTokenVerifier {
        /// Construct a verifier with the supplied Ed25519 verifying key.
        #[must_use]
        pub fn new(verifying_key: VerifyingKey) -> Self {
            Self {
                verifying_key,
                digest_key: None,
            }
        }

        /// Attach the blinded-digest key used by gateways to bind tokens to cache versions.
        #[must_use]
        pub fn with_digest_key(mut self, digest_key: ProofTokenDigestKey) -> Self {
            self.digest_key = Some(digest_key);
            self
        }

        /// Decode and verify a proof token string.
        pub fn verify(
            &self,
            token_b64: &str,
            binding_digest: &[u8; 32],
        ) -> Result<ValidatedProofToken, ProofTokenError> {
            let token = ProofToken::decode_base64(token_b64).map_err(ProofTokenError::Decode)?;
            token
                .verify_signature(&self.verifying_key)
                .map_err(|_| ProofTokenError::Signature)?;
            if let Some(digest_key) = self.digest_key {
                token
                    .verify_blinded_digest(&digest_key, binding_digest)
                    .map_err(|_| ProofTokenError::Digest)?;
            }
            Ok(ValidatedProofToken {
                token,
                digest_bound: self.digest_key.is_some(),
            })
        }
    }

    /// Proof token plus verification results.
    #[derive(Debug, Clone)]
    pub struct ValidatedProofToken {
        pub token: ProofToken,
        pub digest_bound: bool,
    }

    /// Errors surfaced while decoding or verifying proof tokens.
    #[derive(Debug, Error)]
    pub enum ProofTokenError {
        #[error("failed to decode proof token: {0}")]
        Decode(iroha_crypto::sorafs::proof_token::DecodeError),
        #[error("invalid proof token signature")]
        Signature,
        #[error("proof token not bound to cache version")]
        Digest,
    }

    /// Parsed and validated policy evidence returned by a gateway.
    #[derive(Debug, Clone)]
    pub struct PolicyEvidence {
        pub cache_version: String,
        pub evidence: GatewayFailureEvidence,
        pub proof: Option<ValidatedProofToken>,
        pub moderation_proof: Option<ValidatedModerationProof>,
    }

    /// Validation errors surfaced while interpreting policy evidence.
    #[derive(Debug, Error)]
    pub enum PolicyValidationError {
        #[error("canonical status mismatch (expected {expected}, got {actual})")]
        Status {
            expected: StatusCode,
            actual: StatusCode,
        },
        #[error("missing cache version on policy block")]
        MissingCacheVersion,
        #[error("cache version mismatch (expected {expected}, got {actual})")]
        CacheVersionMismatch { expected: String, actual: String },
        #[error("policy code mismatch (expected {expected}, got {actual:?})")]
        CodeMismatch {
            expected: String,
            actual: Option<String>,
        },
        #[error("proof token missing from gateway response")]
        MissingProofToken,
        #[error("proof token verification failed: {0}")]
        ProofToken(#[from] ProofTokenError),
        #[error("verified moderation proof missing from gateway evidence")]
        MissingModerationProof,
    }

    /// Validator for policy evidence emitted by gateways.
    #[derive(Debug, Clone, Copy)]
    pub struct PolicyEvidenceValidator<'a> {
        expected_status: StatusCode,
        expected_code: Option<&'a str>,
        expected_cache_version: Option<&'a str>,
        verifier: Option<&'a ProofTokenVerifier>,
        expect_moderation_proof: bool,
    }

    impl<'a> Default for PolicyEvidenceValidator<'a> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<'a> PolicyEvidenceValidator<'a> {
        /// Construct a validator expecting HTTP 451 and the canonical `denylisted` code.
        #[must_use]
        pub fn new() -> Self {
            Self {
                expected_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                expected_code: Some("denylisted"),
                expected_cache_version: None,
                verifier: None,
                expect_moderation_proof: false,
            }
        }

        /// Require the supplied cache version in the gateway evidence.
        #[must_use]
        pub fn with_expected_cache_version(mut self, version: &'a str) -> Self {
            self.expected_cache_version = Some(version);
            self
        }

        /// Verify proof tokens with the supplied verifier.
        #[must_use]
        pub fn with_proof_verifier(mut self, verifier: &'a ProofTokenVerifier) -> Self {
            self.verifier = Some(verifier);
            self
        }

        /// Require a verified moderation proof in the gateway evidence.
        #[must_use]
        pub fn require_moderation_proof(mut self) -> Self {
            self.expect_moderation_proof = true;
            self
        }

        /// Validate policy evidence and optionally verify the proof token.
        pub fn validate(
            &self,
            evidence: GatewayFailureEvidence,
        ) -> Result<PolicyEvidence, PolicyValidationError> {
            if evidence.canonical_status != self.expected_status {
                return Err(PolicyValidationError::Status {
                    expected: self.expected_status,
                    actual: evidence.canonical_status,
                });
            }

            if let Some(expected_code) = self.expected_code
                && evidence.code.as_deref() != Some(expected_code)
            {
                return Err(PolicyValidationError::CodeMismatch {
                    expected: expected_code.to_string(),
                    actual: evidence.code.clone(),
                });
            }

            let cache_version = evidence
                .cache_version
                .clone()
                .or_else(|| evidence.denylist_version.clone())
                .ok_or(PolicyValidationError::MissingCacheVersion)?;

            if let Some(expected) = self.expected_cache_version
                && cache_version != expected
            {
                return Err(PolicyValidationError::CacheVersionMismatch {
                    expected: expected.to_string(),
                    actual: cache_version,
                });
            }

            let proof = if let Some(verifier) = self.verifier {
                let token = evidence
                    .proof_token_b64
                    .as_deref()
                    .ok_or(PolicyValidationError::MissingProofToken)?;
                let digest = compute_policy_binding_digest(&cache_version);
                Some(verifier.verify(token, &digest)?)
            } else {
                None
            };
            let moderation_proof = evidence.verified_proof.clone();
            if self.expect_moderation_proof && moderation_proof.is_none() {
                return Err(PolicyValidationError::MissingModerationProof);
            }

            Ok(PolicyEvidence {
                cache_version,
                evidence,
                proof,
                moderation_proof,
            })
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

    /// Derive the deterministic binding digest used for cache-version proof tokens.
    #[must_use]
    pub fn compute_policy_binding_digest(cache_version: &str) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(POLICY_BINDING_DOMAIN);
        hasher.update(cache_version.as_bytes());
        hasher.finalize().into()
    }
}

#[cfg(feature = "manifest")]
pub use compliance::*;

#[cfg(all(feature = "manifest", test))]
mod compliance_tests {
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{Duration, SystemTime},
    };

    use base64::Engine as _;
    use blake3;
    use ed25519_dalek::SigningKey;
    use iroha_crypto::sorafs::proof_token::{
        ModerationAction, ProofToken, ProofTokenDigestKey, ProofTokenParams,
    };
    use rand::SeedableRng;
    use reqwest::{
        StatusCode,
        header::{HeaderMap, HeaderName, HeaderValue},
    };
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{StreamTokenBodyV1, StreamTokenV1};

    use super::*;
    use crate::{
        CarBuildPlan, ChunkFetchSpec,
        gateway::{
            GatewayFailureEvidence, GatewayFetchConfig, GatewayFetchContext, GatewayProviderInput,
            HttpEngine, HttpError, HttpFuture, HttpRequest, HttpResponse,
        },
        moderation::{ModerationTokenBodyV1, ValidatedModerationProof},
        policy::{
            PolicyEvidenceValidator, ProofTokenVerifier, compute_policy_binding_digest,
            run_honey_probe,
        },
    };

    const MODERATION_HEADER: &str = "sora-moderation-token";
    const DENYLIST_HEADER: &str = "sora-denylist-version";
    const CACHE_VERSION_HEADER: &str = "sora-cache-version";

    fn sample_payload(len: usize) -> Vec<u8> {
        (0..len).map(|idx| (idx % 251) as u8).collect()
    }

    fn sample_stream_token(
        manifest_cid_hex: &str,
        provider_id_hex: &str,
        profile: &str,
        max_streams: u16,
    ) -> StreamTokenV1 {
        StreamTokenV1 {
            body: StreamTokenBodyV1 {
                token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_string(),
                manifest_cid: hex::decode(manifest_cid_hex).expect("cid hex"),
                provider_id: {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&hex::decode(provider_id_hex).expect("provider hex"));
                    bytes
                },
                profile_handle: profile.to_string(),
                max_streams,
                ttl_epoch: 9_999_999_999,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: 1_735_000_000,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            signature: vec![0; 64],
        }
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
    async fn honey_probe_verifies_cache_binding_and_proof_token() {
        let payload = sample_payload(2048);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let spec: ChunkFetchSpec = plan.chunk_fetch_specs()[0].clone();
        let manifest_id_hex = hex::encode(blake3::hash(&payload).as_bytes());
        let provider_id = provider_id_hex();
        let chunker_handle = "sorafs.sf1@1.0.0".to_string();
        let stream_token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 2);
        let stream_token_b64 = encode_token_b64(&stream_token);

        let cache_version = "cache-v2";
        let denylist_version = "dl-v2";
        let binding_digest = compute_policy_binding_digest(cache_version);

        let digest_key = ProofTokenDigestKey::new([9; 32]);
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let proof_token = ProofToken::mint(
            &mut rand::rngs::StdRng::seed_from_u64(7),
            &digest_key,
            &signing_key,
            &ProofTokenParams {
                moderation: ModerationAction::Block,
                entry_ids: &[denylist_version],
                evidence_digest: &binding_digest,
                issued_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                expires_at: None,
            },
        )
        .expect("mint token");
        let proof_token_b64 = proof_token.encode_base64();

        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(spec.digest)
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(MODERATION_HEADER),
            HeaderValue::from_str(&proof_token_b64).expect("token header"),
        );
        headers.insert(
            HeaderName::from_static(DENYLIST_HEADER),
            HeaderValue::from_str(denylist_version).expect("denylist header"),
        );
        headers.insert(
            HeaderName::from_static(CACHE_VERSION_HEADER),
            HeaderValue::from_str(cache_version).expect("cache header"),
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                headers,
                body: br#"{"error":"denylisted","message":"blocked"}"#.to_vec(),
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
            moderation_token_key_b64: None,
        };
        let provider = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64,
            privacy_events_url: None,
        };
        let context = GatewayFetchContext::build_with_engine(config, [provider], engine)
            .expect("gateway fetch context builds");

        let verifier =
            ProofTokenVerifier::new(signing_key.verifying_key()).with_digest_key(digest_key);
        let validator = PolicyEvidenceValidator::new()
            .with_expected_cache_version(cache_version)
            .with_proof_verifier(&verifier);

        let reports = run_honey_probe(&context.fetcher(), &context.providers(), &spec, &validator)
            .await
            .expect("probe succeeds");
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.provider_id, "alpha");
        assert_eq!(report.policy.cache_version, cache_version);
        assert!(report.policy.proof.as_ref().unwrap().digest_bound);
    }

    #[test]
    fn proof_token_verifier_rejects_wrong_binding_digest() {
        let cache_version = "cache-v1";
        let wrong_version = "cache-v3";
        let digest_key = ProofTokenDigestKey::new([3; 32]);
        let signing_key = SigningKey::from_bytes(&[5u8; 32]);
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let token = ProofToken::mint(
            &mut rng,
            &digest_key,
            &signing_key,
            &ProofTokenParams {
                moderation: ModerationAction::Block,
                entry_ids: &["dl-v1"],
                evidence_digest: &compute_policy_binding_digest(cache_version),
                issued_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_100),
                expires_at: None,
            },
        )
        .expect("mint token");
        let verifier =
            ProofTokenVerifier::new(signing_key.verifying_key()).with_digest_key(digest_key);

        let err = verifier
            .verify(
                &token.encode_base64(),
                &compute_policy_binding_digest(wrong_version),
            )
            .expect_err("digest mismatch should fail");
        match err {
            ProofTokenError::Digest => {}
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn validator_requires_moderation_proof_when_requested() {
        let cache_version = "cache-v1";
        let evidence = GatewayFailureEvidence {
            observed_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            canonical_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            code: Some("denylisted".to_string()),
            proof_token_b64: None,
            verified_proof: None,
            denylist_version: Some(cache_version.to_string()),
            cache_version: Some(cache_version.to_string()),
            message: None,
        };

        let validator = PolicyEvidenceValidator::new()
            .with_expected_cache_version(cache_version)
            .require_moderation_proof();
        let err = validator
            .validate(evidence)
            .expect_err("missing proof fails");
        match err {
            PolicyValidationError::MissingModerationProof => {}
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn validator_accepts_present_moderation_proof() {
        let cache_version = "cache-v1";
        let proof = ValidatedModerationProof {
            body: ModerationTokenBodyV1 {
                manifest_id: [0xAA; 32],
                chunk_digest: None,
                denylist_version: cache_version.to_string(),
                cache_version: cache_version.to_string(),
            },
        };
        let evidence = GatewayFailureEvidence {
            observed_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            canonical_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            code: Some("denylisted".to_string()),
            proof_token_b64: None,
            verified_proof: Some(proof.clone()),
            denylist_version: Some(cache_version.to_string()),
            cache_version: Some(cache_version.to_string()),
            message: None,
        };

        let validator = PolicyEvidenceValidator::new()
            .with_expected_cache_version(cache_version)
            .require_moderation_proof();
        let validated = validator.validate(evidence).expect("validation passes");
        assert!(validated.moderation_proof.is_some());
        assert_eq!(
            validated
                .moderation_proof
                .as_ref()
                .unwrap()
                .body
                .cache_version,
            cache_version
        );
    }
}
