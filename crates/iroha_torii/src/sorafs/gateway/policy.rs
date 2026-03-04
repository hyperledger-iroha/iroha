//! Gateway policy orchestration for GAR enforcement, denylists, and rate limiting.

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use hex::ToHex;
use iroha_data_model::{
    events::data::sorafs::{SorafsGarPolicy, SorafsGarPolicyDetail, SorafsGarViolation},
    sorafs::{capacity::ProviderId, gar::GarCdnPolicyV1, pin_registry::ManifestDigest},
};
use iroha_logger::debug;

use super::{
    denylist::{
        DenylistHit, DenylistKind, GatewayDenylist, PerceptualMatch, PerceptualMatchBasis,
        PerceptualObservation,
    },
    rate_limit::{ClientFingerprint, GatewayRateLimitConfig, GatewayRateLimiter, RateLimitError},
};
use crate::sorafs::{
    AdmissionRegistry,
    gateway::{DenylistEntryBuilder, PerceptualFamilyEntry},
};

/// Policy configuration controlling the enforcement surface.
#[derive(Clone, Debug)]
pub struct GatewayPolicyConfig {
    /// Require manifests to ship the governance envelope before serving data.
    pub require_manifest_envelope: bool,
    /// Enforce that providers appear in the admission registry.
    pub enforce_admission: bool,
    /// Rate limiting configuration applied to gateway clients.
    pub rate_limit: GatewayRateLimitConfig,
    /// Optional CDN-facing GAR policy to enforce.
    pub cdn_policy: Option<GarCdnPolicyV1>,
}

impl Default for GatewayPolicyConfig {
    fn default() -> Self {
        Self {
            require_manifest_envelope: true,
            enforce_admission: true,
            rate_limit: GatewayRateLimitConfig::default(),
            cdn_policy: None,
        }
    }
}

/// Result returned after evaluating policy rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Request is permitted to proceed.
    Allow,
    /// Request is denied because a rule triggered.
    Deny(PolicyViolation),
}

/// Detailed classification for policy denials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    /// Manifest envelope was required but missing.
    ManifestEnvelopeMissing,
    /// Provider identifier was expected but absent.
    MissingProviderId,
    /// Admission registry is unavailable while enforcement is enabled.
    AdmissionUnavailable,
    /// Provider is not admitted according to the governance registry.
    ProviderNotAdmitted {
        /// Identifier of the provider that failed the check.
        provider_id: [u8; 32],
    },
    /// Request or resource matches a denylist entry.
    Denylisted(Box<DenylistHit>),
    /// Rate limiting rejected the client.
    RateLimited(RateLimitError),
    /// Request TTL exceeds the GAR-configured override.
    CdnTtlExceeded {
        /// Allowed TTL in seconds.
        allowed_secs: u64,
        /// Observed TTL in seconds, when available.
        observed_secs: Option<u64>,
    },
    /// Request must be served only after purge tags are present.
    CdnPurgeRequired {
        /// Tags that must accompany the request.
        required_tags: Vec<String>,
    },
    /// Request is missing prerequisite moderation directives.
    CdnModerationRequired {
        /// Moderation slugs required by policy.
        required_slugs: Vec<String>,
    },
    /// Request exceeded the CDN rate ceiling.
    CdnRateCeilingExceeded {
        /// Maximum requests per second allowed by policy.
        ceiling_rps: u64,
        /// Optional retry-after hint propagated from the limiter.
        retry_after: Option<Duration>,
    },
    /// Request originated from a denied or unsupported region.
    CdnGeofenceDenied {
        /// Region label (header or derived), when available.
        region: Option<String>,
    },
    /// Request blocked due to an active legal hold.
    CdnLegalHoldActive,
}

/// Context supplied to the policy evaluator for each request.
#[derive(Debug)]
pub struct RequestContext<'a> {
    provider_id: Option<&'a [u8; 32]>,
    manifest_digest: Option<&'a [u8; 32]>,
    content_cid: Option<&'a [u8]>,
    manifest_envelope_present: bool,
    perceptual: Option<PerceptualObservation<'a>>,
    client: &'a ClientFingerprint,
    wall_time: SystemTime,
    monotonic_now: Instant,
    remote_addr: Option<SocketAddr>,
    canonical_host: Option<String>,
    region: Option<String>,
    cache_ttl_secs: Option<u64>,
    policy_tags: Vec<String>,
    moderation_slugs: Vec<String>,
}

impl<'a> RequestContext<'a> {
    /// Construct a new context with mandatory metadata.
    #[must_use]
    pub fn new(
        client: &'a ClientFingerprint,
        wall_time: SystemTime,
        monotonic_now: Instant,
    ) -> Self {
        Self {
            provider_id: None,
            manifest_digest: None,
            content_cid: None,
            manifest_envelope_present: false,
            perceptual: None,
            client,
            wall_time,
            monotonic_now,
            remote_addr: None,
            canonical_host: None,
            region: None,
            cache_ttl_secs: None,
            policy_tags: Vec::new(),
            moderation_slugs: Vec::new(),
        }
    }

    /// Attach the provider identifier used for GAR enforcement.
    #[must_use]
    pub fn with_provider_id(mut self, provider_id: &'a [u8; 32]) -> Self {
        self.provider_id = Some(provider_id);
        self
    }

    /// Attach the manifest digest to enable denylist checks.
    #[must_use]
    pub fn with_manifest_digest(mut self, digest: &'a [u8; 32]) -> Self {
        self.manifest_digest = Some(digest);
        self
    }

    /// Attach the content identifier (CID) for denylist enforcement.
    #[must_use]
    pub fn with_content_cid(mut self, cid: &'a [u8]) -> Self {
        self.content_cid = Some(cid);
        self
    }

    /// Attach perceptual fingerprint metadata for denylist enforcement.
    #[must_use]
    pub fn with_perceptual_observation(mut self, observation: PerceptualObservation<'a>) -> Self {
        self.perceptual = Some(observation);
        self
    }

    /// Marks that the manifest envelope was supplied for this request.
    #[must_use]
    pub fn with_manifest_envelope(mut self, present: bool) -> Self {
        self.manifest_envelope_present = present;
        self
    }

    /// Provider identifier associated with the request, if known.
    #[must_use]
    pub fn provider_id(&self) -> Option<&'a [u8; 32]> {
        self.provider_id
    }

    /// Manifest digest associated with the request (BLAKE3-256), if supplied.
    #[must_use]
    pub fn manifest_digest(&self) -> Option<&'a [u8; 32]> {
        self.manifest_digest
    }

    /// Content identifier (CID) referenced by the request, if any.
    #[must_use]
    pub fn content_cid(&self) -> Option<&'a [u8]> {
        self.content_cid
    }

    /// Perceptual fingerprint metadata provided by the request.
    #[must_use]
    pub fn perceptual_observation(&self) -> Option<PerceptualObservation<'a>> {
        self.perceptual
    }

    /// Whether a manifest envelope was supplied alongside the request.
    #[must_use]
    pub fn manifest_envelope_present(&self) -> bool {
        self.manifest_envelope_present
    }

    /// Fingerprint representing the client issuing the request.
    #[must_use]
    pub fn client(&self) -> &'a ClientFingerprint {
        self.client
    }

    /// Attach the remote socket address associated with the request.
    #[must_use]
    pub fn with_remote_addr(mut self, remote: SocketAddr) -> Self {
        self.remote_addr = Some(remote);
        self
    }

    /// Attach the canonical host associated with the request.
    #[must_use]
    pub fn with_canonical_host(mut self, host: impl Into<String>) -> Self {
        let host = host.into();
        if !host.is_empty() {
            self.canonical_host = Some(host);
        }
        self
    }

    /// Attach the region associated with the request (e.g., ISO country or POP code).
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        let region = region.into();
        if !region.is_empty() {
            self.region = Some(region);
        }
        self
    }

    /// Attach the cache TTL observed on the request (seconds).
    #[must_use]
    pub fn with_cache_ttl_secs(mut self, ttl: u64) -> Self {
        self.cache_ttl_secs = Some(ttl);
        self
    }

    /// Attach policy tags supplied by the request (comma-separated header).
    #[must_use]
    pub fn with_policy_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for tag in tags {
            let value = tag.into();
            if !value.is_empty() {
                self.policy_tags.push(value);
            }
        }
        self
    }

    /// Attach moderation directive slugs that were already applied upstream.
    #[must_use]
    pub fn with_moderation_slugs<I, S>(mut self, slugs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for slug in slugs {
            let value = slug.into();
            if !value.is_empty() {
                self.moderation_slugs.push(value);
            }
        }
        self
    }

    /// Remote socket address for the request, when available.
    #[must_use]
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Current wall-clock time associated with the request.
    #[must_use]
    pub fn wall_time(&self) -> SystemTime {
        self.wall_time
    }

    /// Monotonic instant used for rate-limiter accounting.
    #[must_use]
    pub fn monotonic_now(&self) -> Instant {
        self.monotonic_now
    }

    /// Canonical host for the request, if supplied.
    #[must_use]
    pub fn canonical_host(&self) -> Option<&str> {
        self.canonical_host.as_deref()
    }

    /// Optional region label associated with the request.
    #[must_use]
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    /// Optional cache TTL observed for the request (seconds).
    #[must_use]
    pub fn cache_ttl_secs(&self) -> Option<u64> {
        self.cache_ttl_secs
    }

    /// Policy tags supplied with the request.
    #[must_use]
    pub fn policy_tags(&self) -> &[String] {
        &self.policy_tags
    }

    /// Moderation directive slugs supplied with the request.
    #[must_use]
    pub fn moderation_slugs(&self) -> &[String] {
        &self.moderation_slugs
    }
}

impl PolicyViolation {
    /// Convert the violation into telemetry-friendly reason/detail labels.
    #[must_use]
    pub fn telemetry_labels(&self) -> (&'static str, &'static str) {
        match self {
            Self::ManifestEnvelopeMissing => ("manifest_envelope", "missing"),
            Self::MissingProviderId => ("provider", "missing_id"),
            Self::AdmissionUnavailable => ("admission", "unavailable"),
            Self::ProviderNotAdmitted { .. } => ("admission", "not_admitted"),
            Self::Denylisted(hit) => {
                let hit = hit.as_ref();
                let detail = match hit.kind() {
                    DenylistKind::Provider(_) => "provider",
                    DenylistKind::ManifestDigest(_) => "manifest_digest",
                    DenylistKind::Cid(_) => "cid",
                    DenylistKind::Url(_) => "url",
                    DenylistKind::AccountId(_) => "account_id",
                    DenylistKind::AccountAlias(_) => "account_alias",
                    DenylistKind::PerceptualFamily { .. } => "perceptual_family",
                };
                ("denylist", detail)
            }
            Self::RateLimited(error) => match error {
                RateLimitError::Limited { .. } => ("rate_limit", "limited"),
                RateLimitError::Banned { .. } => ("rate_limit", "banned"),
            },
            Self::CdnTtlExceeded { .. } => ("cdn", "ttl"),
            Self::CdnPurgeRequired { .. } => ("cdn", "purge"),
            Self::CdnModerationRequired { .. } => ("cdn", "moderation"),
            Self::CdnRateCeilingExceeded { .. } => ("cdn", "rate_ceiling"),
            Self::CdnGeofenceDenied { .. } => ("cdn", "geofence"),
            Self::CdnLegalHoldActive => ("cdn", "legal_hold"),
        }
    }
}

fn normalized_label(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_uppercase())
    }
}

fn tags_intersect(required: &[String], provided: &[String]) -> bool {
    required.iter().any(|required_tag| {
        provided
            .iter()
            .any(|candidate| required_tag.eq_ignore_ascii_case(candidate))
    })
}

fn build_cdn_rate_limiter(
    cdn_policy: Option<&GarCdnPolicyV1>,
    fallback: &GatewayRateLimitConfig,
) -> Option<GatewayRateLimiter> {
    let ceiling = cdn_policy.and_then(|policy| policy.rate_ceiling_rps)?;
    if ceiling == 0 {
        return None;
    }
    let limit = ceiling.min(u32::MAX as u64) as u32;
    let config = GatewayRateLimitConfig {
        max_requests: Some(limit),
        window: Duration::from_secs(1),
        ban_duration: fallback.ban_duration,
    };
    Some(GatewayRateLimiter::new(config))
}

fn enforce_cdn_policy(
    policy: &GarCdnPolicyV1,
    limiter: Option<&GatewayRateLimiter>,
    ctx: &RequestContext<'_>,
) -> Option<PolicyViolation> {
    if policy.legal_hold {
        return Some(PolicyViolation::CdnLegalHoldActive);
    }

    let region = ctx.region().and_then(normalized_label);
    if let Some(region_label) = &region {
        if policy
            .deny_regions
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(region_label))
        {
            return Some(PolicyViolation::CdnGeofenceDenied {
                region: Some(region_label.clone()),
            });
        }
    }

    if !policy.allow_regions.is_empty() {
        match region {
            Some(region_label) => {
                if !policy
                    .allow_regions
                    .iter()
                    .any(|entry| entry.eq_ignore_ascii_case(&region_label))
                {
                    return Some(PolicyViolation::CdnGeofenceDenied {
                        region: Some(region_label),
                    });
                }
            }
            None => {
                return Some(PolicyViolation::CdnGeofenceDenied { region: None });
            }
        }
    }

    if let Some(ttl_override) = policy.ttl_override_secs {
        if let Some(observed) = ctx.cache_ttl_secs() {
            if observed > ttl_override {
                return Some(PolicyViolation::CdnTtlExceeded {
                    allowed_secs: ttl_override,
                    observed_secs: Some(observed),
                });
            }
        }
    }

    if !policy.purge_tags.is_empty() && !tags_intersect(&policy.purge_tags, ctx.policy_tags()) {
        return Some(PolicyViolation::CdnPurgeRequired {
            required_tags: policy.purge_tags.clone(),
        });
    }

    if !policy.moderation_slugs.is_empty()
        && !tags_intersect(&policy.moderation_slugs, ctx.moderation_slugs())
    {
        return Some(PolicyViolation::CdnModerationRequired {
            required_slugs: policy.moderation_slugs.clone(),
        });
    }

    if let (Some(limiter), Some(ceiling)) = (limiter, policy.rate_ceiling_rps) {
        let key = ctx
            .canonical_host()
            .map(str::to_ascii_lowercase)
            .filter(|host| !host.is_empty())
            .unwrap_or_else(|| "global".to_string());
        let fingerprint = ClientFingerprint::from_identifier(&format!("cdn|{key}"));
        if let Err(error) = limiter.check(&fingerprint, ctx.monotonic_now()) {
            let retry_after = match error {
                RateLimitError::Limited { retry_after } => Some(retry_after),
                RateLimitError::Banned { retry_after } => retry_after,
            };
            return Some(PolicyViolation::CdnRateCeilingExceeded {
                ceiling_rps: ceiling,
                retry_after,
            });
        }
    }

    None
}

/// Policy orchestrator performing GAR, denylist, and rate limiting checks.
#[derive(Debug)]
pub struct GatewayPolicy {
    config: GatewayPolicyConfig,
    admission: Option<Arc<AdmissionRegistry>>,
    denylist: Arc<GatewayDenylist>,
    rate_limiter: GatewayRateLimiter,
    cdn_rate_limiter: Option<GatewayRateLimiter>,
}

impl GatewayPolicy {
    /// Construct a policy instance.
    #[must_use]
    pub fn new(
        config: GatewayPolicyConfig,
        admission: Option<Arc<AdmissionRegistry>>,
        denylist: Arc<GatewayDenylist>,
        rate_limiter: GatewayRateLimiter,
    ) -> Self {
        let cdn_rate_limiter =
            build_cdn_rate_limiter(config.cdn_policy.as_ref(), &config.rate_limit);
        Self {
            config,
            admission,
            denylist,
            rate_limiter,
            cdn_rate_limiter,
        }
    }

    /// Construct a policy using default configuration.
    #[must_use]
    pub fn new_default(
        admission: Option<Arc<AdmissionRegistry>>,
        denylist: Arc<GatewayDenylist>,
    ) -> Self {
        let config = GatewayPolicyConfig::default();
        let rate_limiter = GatewayRateLimiter::new(config.rate_limit.clone());
        Self::new(config, admission, denylist, rate_limiter)
    }

    /// Evaluate the supplied request context and return the policy decision.
    #[must_use]
    pub fn evaluate(&self, ctx: &RequestContext<'_>) -> PolicyDecision {
        if self.config.require_manifest_envelope && !ctx.manifest_envelope_present() {
            return PolicyDecision::Deny(PolicyViolation::ManifestEnvelopeMissing);
        }

        if let Some(provider_id) = ctx.provider_id() {
            if let Some(hit) = self.denylist.check_provider(provider_id, ctx.wall_time()) {
                return PolicyDecision::Deny(PolicyViolation::Denylisted(Box::new(hit)));
            }
        }

        if let Some(digest) = ctx.manifest_digest() {
            if let Some(hit) = self.denylist.check_manifest_digest(digest, ctx.wall_time()) {
                return PolicyDecision::Deny(PolicyViolation::Denylisted(Box::new(hit)));
            }
        }

        if let Some(cid) = ctx.content_cid() {
            if let Some(hit) = self.denylist.check_cid(cid, ctx.wall_time()) {
                return PolicyDecision::Deny(PolicyViolation::Denylisted(Box::new(hit)));
            }
        }

        if let Some(observation) = ctx.perceptual_observation() {
            if let Some(hit) = self
                .denylist
                .check_perceptual(&observation, ctx.wall_time())
            {
                return PolicyDecision::Deny(PolicyViolation::Denylisted(Box::new(hit)));
            }
        }

        if self.config.enforce_admission {
            let Some(registry) = &self.admission else {
                return PolicyDecision::Allow;
            };

            let Some(provider_id) = ctx.provider_id() else {
                return PolicyDecision::Deny(PolicyViolation::MissingProviderId);
            };

            if registry.entry(provider_id).is_none() {
                debug!(
                    "GAR enforcement: provider {provider_id:02x?} missing from registry",
                    provider_id = provider_id
                );
                return PolicyDecision::Deny(PolicyViolation::ProviderNotAdmitted {
                    provider_id: *provider_id,
                });
            }
        }

        if let Some(policy) = &self.config.cdn_policy {
            if let Some(violation) = enforce_cdn_policy(policy, self.cdn_rate_limiter.as_ref(), ctx)
            {
                return PolicyDecision::Deny(violation);
            }
        }

        if let Err(err) = self.rate_limiter.check(ctx.client(), ctx.monotonic_now()) {
            return PolicyDecision::Deny(PolicyViolation::RateLimited(err));
        }

        PolicyDecision::Allow
    }

    /// Returns a reference to the rate limiter (primarily for tests).
    #[must_use]
    pub fn rate_limiter(&self) -> &GatewayRateLimiter {
        &self.rate_limiter
    }
}

/// Build a [`SorafsGarViolation`] payload describing the provided policy failure.
pub fn build_gar_violation_event(
    ctx: &RequestContext<'_>,
    violation: &PolicyViolation,
) -> SorafsGarViolation {
    let mut provider_id = ctx.provider_id().map(|id| ProviderId::new(*id));
    let mut manifest_digest = ctx
        .manifest_digest()
        .map(|digest| ManifestDigest::new(*digest));
    let manifest_cid_b64 = ctx.content_cid().map(|cid| BASE64_STD.encode(cid));
    let mut denylisted_cid_b64: Option<String> = None;
    let mut denylisted_url: Option<String> = None;
    let mut denylisted_account_id: Option<String> = None;
    let mut denylisted_account_alias: Option<String> = None;
    let mut denylisted_perceptual_family_hex: Option<String> = None;
    let mut denylisted_perceptual_variant_hex: Option<String> = None;
    let mut denylisted_perceptual_hash_hex: Option<String> = None;
    let mut denylisted_perceptual_embedding_hex: Option<String> = None;
    let mut perceptual_hamming_distance: Option<u8> = None;
    let mut perceptual_hamming_radius: Option<u8> = None;
    let mut jurisdiction: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut entry_alias: Option<String> = None;
    let mut issued_at_unix: Option<u64> = None;
    let mut expires_at_unix: Option<u64> = None;
    let mut retry_after_seconds: Option<u64> = None;
    let mut region = ctx.region().map(ToOwned::to_owned);
    let host = ctx.canonical_host().map(ToOwned::to_owned);
    let mut policy_labels: Vec<String> = Vec::new();
    let mut observed_ttl_seconds = ctx.cache_ttl_secs();
    let mut rate_ceiling_rps: Option<u64> = None;

    let (policy, detail) = match violation {
        PolicyViolation::ManifestEnvelopeMissing => (
            SorafsGarPolicy::ManifestEnvelope,
            SorafsGarPolicyDetail::ManifestEnvelopeMissing,
        ),
        PolicyViolation::MissingProviderId => (
            SorafsGarPolicy::Provider,
            SorafsGarPolicyDetail::ProviderIdMissing,
        ),
        PolicyViolation::AdmissionUnavailable => (
            SorafsGarPolicy::Admission,
            SorafsGarPolicyDetail::AdmissionUnavailable,
        ),
        PolicyViolation::ProviderNotAdmitted { provider_id: pid } => {
            provider_id = Some(ProviderId::new(*pid));
            (
                SorafsGarPolicy::Admission,
                SorafsGarPolicyDetail::ProviderNotAdmitted,
            )
        }
        PolicyViolation::Denylisted(hit) => {
            let hit = hit.as_ref();
            let detail = match hit.kind() {
                DenylistKind::Provider(id) => {
                    provider_id = Some(ProviderId::new(*id));
                    SorafsGarPolicyDetail::DenylistedProvider
                }
                DenylistKind::ManifestDigest(digest) => {
                    manifest_digest = Some(ManifestDigest::new(*digest));
                    SorafsGarPolicyDetail::DenylistedManifestDigest
                }
                DenylistKind::Cid(cid) => {
                    let encoded = BASE64_STD.encode(cid);
                    denylisted_cid_b64 = Some(encoded);
                    SorafsGarPolicyDetail::DenylistedCid
                }
                DenylistKind::Url(url) => {
                    denylisted_url = Some(url.clone());
                    SorafsGarPolicyDetail::DenylistedUrl
                }
                DenylistKind::AccountId(account) => {
                    denylisted_account_id = Some(account.clone());
                    SorafsGarPolicyDetail::DenylistedAccountId
                }
                DenylistKind::AccountAlias(alias) => {
                    denylisted_account_alias = Some(alias.clone());
                    SorafsGarPolicyDetail::DenylistedAccountAlias
                }
                DenylistKind::PerceptualFamily {
                    family_id,
                    variant_id,
                } => {
                    denylisted_perceptual_family_hex = Some(family_id.encode_hex::<String>());
                    denylisted_perceptual_variant_hex =
                        variant_id.map(|id| id.encode_hex::<String>());
                    if let Some(perceptual) = hit.perceptual_match() {
                        match perceptual.basis() {
                            PerceptualMatchBasis::Hash {
                                expected,
                                hamming_distance,
                                radius,
                                ..
                            } => {
                                denylisted_perceptual_hash_hex =
                                    Some(expected.encode_hex::<String>());
                                perceptual_hamming_distance = Some(*hamming_distance);
                                perceptual_hamming_radius = Some(*radius);
                            }
                            PerceptualMatchBasis::Embedding { expected, .. } => {
                                denylisted_perceptual_embedding_hex =
                                    Some(expected.encode_hex::<String>());
                            }
                        }
                    }
                    SorafsGarPolicyDetail::DenylistedPerceptualFamily
                }
            };

            let entry = hit.entry();
            jurisdiction = entry.jurisdiction().map(ToOwned::to_owned);
            reason = entry.reason().map(ToOwned::to_owned);
            entry_alias = entry.alias().map(ToOwned::to_owned);
            issued_at_unix = entry
                .issued_at()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            expires_at_unix = entry
                .expires_at()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());

            (SorafsGarPolicy::Denylist, detail)
        }
        PolicyViolation::RateLimited(error) => {
            let detail = match error {
                RateLimitError::Limited { retry_after } => {
                    retry_after_seconds = Some(retry_after.as_secs());
                    SorafsGarPolicyDetail::RateLimitExceeded
                }
                RateLimitError::Banned { retry_after } => {
                    retry_after_seconds = retry_after.map(|duration| duration.as_secs());
                    SorafsGarPolicyDetail::RateLimitBanned
                }
            };
            (SorafsGarPolicy::RateLimit, detail)
        }
        PolicyViolation::CdnTtlExceeded {
            allowed_secs,
            observed_secs,
        } => {
            if observed_ttl_seconds.is_none() {
                observed_ttl_seconds = *observed_secs;
            }
            policy_labels.push(format!("ttl<= {allowed_secs}s"));
            if let Some(observed) = observed_secs {
                policy_labels.push(format!("ttl_observed={observed}s"));
            }
            (SorafsGarPolicy::Cdn, SorafsGarPolicyDetail::CdnTtlExceeded)
        }
        PolicyViolation::CdnPurgeRequired { required_tags } => {
            policy_labels.extend(required_tags.clone());
            (
                SorafsGarPolicy::Cdn,
                SorafsGarPolicyDetail::CdnPurgeRequired,
            )
        }
        PolicyViolation::CdnModerationRequired { required_slugs } => {
            policy_labels.extend(required_slugs.clone());
            (
                SorafsGarPolicy::Cdn,
                SorafsGarPolicyDetail::CdnModerationBlocked,
            )
        }
        PolicyViolation::CdnRateCeilingExceeded {
            ceiling_rps,
            retry_after,
        } => {
            rate_ceiling_rps = Some(*ceiling_rps);
            retry_after_seconds = retry_after.map(|duration| duration.as_secs());
            (
                SorafsGarPolicy::Cdn,
                SorafsGarPolicyDetail::CdnRateCeilingExceeded,
            )
        }
        PolicyViolation::CdnGeofenceDenied { region: geo } => {
            if region.is_none() {
                region.clone_from(geo);
            }
            (
                SorafsGarPolicy::Cdn,
                SorafsGarPolicyDetail::CdnGeofenceDenied,
            )
        }
        PolicyViolation::CdnLegalHoldActive => (
            SorafsGarPolicy::Cdn,
            SorafsGarPolicyDetail::CdnLegalHoldActive,
        ),
    };

    let (reason_label, detail_label) = violation.telemetry_labels();
    policy_labels.insert(0, detail_label.to_string());
    policy_labels.insert(0, reason_label.to_string());

    let client_fingerprint_hex = ctx.client().as_bytes().encode_hex::<String>();
    let remote_addr = ctx.remote_addr().map(|addr| addr.to_string());
    let occurred_at_unix = ctx
        .wall_time()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();

    SorafsGarViolation {
        policy,
        detail,
        provider_id,
        manifest_digest,
        manifest_cid_b64,
        client_fingerprint_hex,
        remote_addr,
        region,
        host,
        policy_labels,
        observed_ttl_seconds,
        rate_ceiling_rps,
        jurisdiction,
        reason,
        entry_alias,
        issued_at_unix,
        expires_at_unix,
        denylisted_cid_b64,
        denylisted_url,
        denylisted_account_id,
        denylisted_account_alias,
        denylisted_perceptual_family_hex,
        denylisted_perceptual_variant_hex,
        denylisted_perceptual_hash_hex,
        denylisted_perceptual_embedding_hex,
        perceptual_hamming_distance,
        perceptual_hamming_radius,
        retry_after_seconds,
        occurred_at_unix,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    fn sample_provider_id() -> [u8; 32] {
        [0x42; 32]
    }

    #[test]
    fn policy_denies_missing_manifest_envelope() {
        let denylist = Arc::new(GatewayDenylist::new());
        let admission = Some(Arc::new(AdmissionRegistry::empty()));
        let policy = GatewayPolicy::new_default(admission, denylist);
        let client = ClientFingerprint::from_identifier("client");
        let provider = sample_provider_id();
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_provider_id(&provider)
            .with_manifest_envelope(false);
        let decision = policy.evaluate(&ctx);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::ManifestEnvelopeMissing)
        ));
    }

    #[test]
    fn policy_denies_unknown_provider() {
        let denylist = Arc::new(GatewayDenylist::new());
        let admission = Some(Arc::new(AdmissionRegistry::empty()));
        let policy = GatewayPolicy::new_default(admission, denylist);
        let client = ClientFingerprint::from_identifier("client");
        let other_provider = [0x55; 32];
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_provider_id(&other_provider)
            .with_manifest_envelope(true);
        let decision = policy.evaluate(&ctx);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::ProviderNotAdmitted { .. })
        ));
    }

    #[test]
    fn policy_denies_rate_limited_client() {
        let denylist = Arc::new(GatewayDenylist::new());
        let admission = None;
        let rate_limit = GatewayRateLimitConfig {
            max_requests: Some(1),
            window: Duration::from_mins(1),
            ..GatewayRateLimitConfig::default()
        };
        let config = GatewayPolicyConfig {
            enforce_admission: false,
            rate_limit,
            ..GatewayPolicyConfig::default()
        };
        let policy = GatewayPolicy::new(
            config.clone(),
            admission,
            denylist,
            GatewayRateLimiter::new(config.rate_limit),
        );
        let client = ClientFingerprint::from_identifier("client");
        let base = Instant::now();
        let provider = sample_provider_id();
        let ctx = RequestContext::new(&client, SystemTime::now(), base)
            .with_provider_id(&provider)
            .with_manifest_envelope(true);
        assert!(matches!(policy.evaluate(&ctx), PolicyDecision::Allow));
        let denied = policy.evaluate(
            &RequestContext::new(&client, SystemTime::now(), base + Duration::from_millis(5))
                .with_provider_id(&provider)
                .with_manifest_envelope(true),
        );
        assert!(matches!(
            denied,
            PolicyDecision::Deny(PolicyViolation::RateLimited(_))
        ));
    }

    #[test]
    fn gar_violation_event_for_provider_not_admitted() {
        let client = ClientFingerprint::from_identifier("client");
        let provider = sample_provider_id();
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_provider_id(&provider)
            .with_manifest_envelope(true)
            .with_remote_addr(SocketAddr::from(([127, 0, 0, 1], 8080)));
        let violation = PolicyViolation::ProviderNotAdmitted {
            provider_id: provider,
        };
        let event = build_gar_violation_event(&ctx, &violation);
        assert_eq!(event.policy, SorafsGarPolicy::Admission);
        assert_eq!(event.detail, SorafsGarPolicyDetail::ProviderNotAdmitted);
        assert_eq!(event.provider_id.unwrap().as_bytes(), &provider);
        assert_eq!(event.remote_addr.as_deref(), Some("127.0.0.1:8080"));
    }

    #[test]
    fn gar_violation_event_for_denylisted_url() {
        let client = ClientFingerprint::from_identifier("client");
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_manifest_envelope(true);
        let entry = DenylistEntryBuilder::default()
            .jurisdiction("US")
            .reason("restricted jurisdiction")
            .build();
        let denylist = GatewayDenylist::from_entries([(
            DenylistKind::Url("https://example.com".to_owned()),
            entry.clone(),
        )]);
        let hit = denylist
            .check_url("https://example.com", SystemTime::now())
            .expect("denylist hit must exist");
        let violation = PolicyViolation::Denylisted(Box::new(hit));
        let event = build_gar_violation_event(&ctx, &violation);
        assert_eq!(event.policy, SorafsGarPolicy::Denylist);
        assert_eq!(event.detail, SorafsGarPolicyDetail::DenylistedUrl);
        assert_eq!(event.denylisted_url.as_deref(), Some("https://example.com"));
        assert_eq!(event.jurisdiction.as_deref(), Some("US"));
        assert_eq!(event.reason.as_deref(), Some("restricted jurisdiction"));
    }

    #[test]
    fn gar_violation_event_for_rate_limit_violation() {
        let client = ClientFingerprint::from_identifier("client");
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_manifest_envelope(true);
        let violation = PolicyViolation::RateLimited(RateLimitError::Limited {
            retry_after: Duration::from_secs(42),
        });
        let event = build_gar_violation_event(&ctx, &violation);
        assert_eq!(event.policy, SorafsGarPolicy::RateLimit);
        assert_eq!(event.detail, SorafsGarPolicyDetail::RateLimitExceeded);
        assert_eq!(event.retry_after_seconds, Some(42));
        assert_eq!(
            event.policy_labels,
            vec!["rate_limit".to_string(), "limited".to_string()]
        );
    }

    #[test]
    fn gar_violation_event_for_missing_envelope_maps_labels() {
        let client = ClientFingerprint::from_identifier("client");
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now());
        let violation = PolicyViolation::ManifestEnvelopeMissing;
        let event = build_gar_violation_event(&ctx, &violation);
        assert_eq!(event.policy, SorafsGarPolicy::ManifestEnvelope);
        assert_eq!(event.detail, SorafsGarPolicyDetail::ManifestEnvelopeMissing);
        assert_eq!(
            event.policy_labels,
            vec!["manifest_envelope".to_string(), "missing".to_string()]
        );
    }

    #[test]
    fn gar_violation_event_for_perceptual_family() {
        let denylist = Arc::new(GatewayDenylist::new());
        let metadata = DenylistEntryBuilder::default()
            .reason("Perceptual block")
            .build();
        let family_id = [0x11; 16];
        let variant_id = [0x22; 16];
        let canonical_hash = [0xAA; 32];
        let entry = PerceptualFamilyEntry::new(family_id, metadata)
            .with_variant_id(Some(variant_id))
            .with_perceptual_hash(Some(canonical_hash), 4);
        denylist.upsert_perceptual(entry);
        let policy = GatewayPolicy::new_default(None, denylist.clone());
        let client = ClientFingerprint::from_identifier("client");
        let mut observed_hash = canonical_hash;
        observed_hash[0] ^= 0x01;
        let observation = PerceptualObservation::new(Some(&observed_hash), None);
        let ctx = RequestContext::new(&client, SystemTime::now(), Instant::now())
            .with_manifest_envelope(true)
            .with_perceptual_observation(observation);
        let decision = policy.evaluate(&ctx);
        let violation = match decision {
            PolicyDecision::Deny(PolicyViolation::Denylisted(hit)) => {
                PolicyViolation::Denylisted(hit)
            }
            other => panic!("unexpected decision: {other:?}"),
        };
        let event = build_gar_violation_event(&ctx, &violation);
        assert_eq!(
            event.detail,
            SorafsGarPolicyDetail::DenylistedPerceptualFamily
        );
        assert_eq!(
            event.denylisted_perceptual_family_hex,
            Some(hex::encode(family_id))
        );
        assert_eq!(
            event.denylisted_perceptual_variant_hex,
            Some(hex::encode(variant_id))
        );
        assert_eq!(
            event.denylisted_perceptual_hash_hex,
            Some(hex::encode(canonical_hash))
        );
        assert_eq!(event.perceptual_hamming_distance, Some(1));
        assert_eq!(event.perceptual_hamming_radius, Some(4));
        assert_eq!(
            event.policy_labels.first().map(String::as_str),
            Some("denylist")
        );
        assert_eq!(
            event.policy_labels.get(1).map(String::as_str),
            Some("perceptual_family")
        );
    }

    fn policy_with_cdn(cdn: GarCdnPolicyV1) -> GatewayPolicy {
        let denylist = Arc::new(GatewayDenylist::new());
        let config = GatewayPolicyConfig {
            enforce_admission: false,
            require_manifest_envelope: false,
            cdn_policy: Some(cdn.clone()),
            ..GatewayPolicyConfig::default()
        };
        GatewayPolicy::new(
            config.clone(),
            None,
            denylist,
            GatewayRateLimiter::new(config.rate_limit),
        )
    }

    fn base_context(client: &ClientFingerprint) -> RequestContext<'_> {
        RequestContext::new(client, SystemTime::now(), Instant::now()).with_manifest_envelope(true)
    }

    #[test]
    fn cdn_policy_blocks_legal_hold() {
        let client = ClientFingerprint::from_identifier("client");
        let policy = policy_with_cdn(GarCdnPolicyV1 {
            legal_hold: true,
            ..GarCdnPolicyV1::default()
        });
        let decision = policy.evaluate(&base_context(&client));
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::CdnLegalHoldActive)
        ));
    }

    #[test]
    fn cdn_policy_rejects_geofence_miss() {
        let client = ClientFingerprint::from_identifier("client");
        let policy = policy_with_cdn(GarCdnPolicyV1 {
            allow_regions: vec!["EU".to_string()],
            deny_regions: vec!["US".to_string()],
            ..GarCdnPolicyV1::default()
        });
        let ctx = base_context(&client).with_region("us");
        let decision = policy.evaluate(&ctx);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::CdnGeofenceDenied { .. })
        ));
    }

    #[test]
    fn cdn_policy_enforces_ttl_override() {
        let client = ClientFingerprint::from_identifier("client");
        let policy = policy_with_cdn(GarCdnPolicyV1 {
            ttl_override_secs: Some(60),
            ..GarCdnPolicyV1::default()
        });
        let ctx = base_context(&client).with_cache_ttl_secs(120);
        let decision = policy.evaluate(&ctx);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::CdnTtlExceeded { .. })
        ));
    }

    #[test]
    fn cdn_policy_requires_purge_tags() {
        let client = ClientFingerprint::from_identifier("client");
        let policy = policy_with_cdn(GarCdnPolicyV1 {
            purge_tags: vec!["hotfix".to_string()],
            ..GarCdnPolicyV1::default()
        });
        let ctx = base_context(&client);
        let decision = policy.evaluate(&ctx);
        assert!(matches!(
            decision,
            PolicyDecision::Deny(PolicyViolation::CdnPurgeRequired { .. })
        ));
    }

    #[test]
    fn cdn_policy_rate_ceiling_trips() {
        let client = ClientFingerprint::from_identifier("client");
        let policy = policy_with_cdn(GarCdnPolicyV1 {
            rate_ceiling_rps: Some(1),
            ..GarCdnPolicyV1::default()
        });
        let ctx = base_context(&client).with_canonical_host("cdn.example");
        assert!(matches!(policy.evaluate(&ctx), PolicyDecision::Allow));
        let denied = policy.evaluate(&ctx);
        assert!(matches!(
            denied,
            PolicyDecision::Deny(PolicyViolation::CdnRateCeilingExceeded { .. })
        ));
    }
}
