//! Stream token issuance helpers for Torii chunk-range gateways.
use base64::Engine as _;
use ed25519_dalek::VerifyingKey;
use iroha_config::parameters::{actual, validate_production_runtime_handle};
use iroha_crypto::PublicKey;
use rand::{
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::OsRng,
};
use sorafs_manifest::{
    STREAM_TOKEN_MAX_BASE64_BYTES_V1, STREAM_TOKEN_MAX_TTL_SECS_V1, STREAM_TOKEN_MAX_WIRE_BYTES_V1,
    StreamTokenBodyV1, StreamTokenError, StreamTokenV1,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
/// Fixed rolling window applied to authenticated-subject issuance quotas.
const ISSUANCE_QUOTA_WINDOW: Duration = Duration::from_mins(1);
/// Maximum number of active authenticated issuance subjects retained by one gateway.
const MAX_ISSUANCE_SUBJECTS: usize = 4_096;
/// Maximum accepted encoded token header length.
pub(crate) const MAX_STREAM_TOKEN_BASE64_BYTES: usize = STREAM_TOKEN_MAX_BASE64_BYTES_V1;
/// Maximum accepted decoded token frame length.
const MAX_STREAM_TOKEN_WIRE_BYTES: usize = STREAM_TOKEN_MAX_WIRE_BYTES_V1;
/// Canonical token IDs are 16 random bytes rendered as lowercase hexadecimal.
const TOKEN_ID_HEX_LEN: usize = 32;
/// Maximum manifest CID bytes carried by a token.
const MAX_MANIFEST_CID_BYTES: usize = 128;
/// Maximum canonical chunk-profile handle bytes carried by a token.
const MAX_PROFILE_HANDLE_BYTES: usize = 128;
/// Maximum issuance client identifier bytes.
pub(crate) const MAX_CLIENT_ID_BYTES: usize = 128;
/// Maximum echoed issuance nonce bytes.
pub(crate) const MAX_NONCE_BYTES: usize = 128;
/// Maximum concurrency encoded in one token.
const MAX_TOKEN_STREAMS: u16 = 1_024;
/// Maximum per-request byte budget encoded in one token (1 GiB).
const MAX_TOKEN_RATE_LIMIT_BYTES: u64 = 1_073_741_824;
/// Maximum per-token request budget and per-subject issuance quota.
const MAX_TOKEN_REQUESTS_PER_MINUTE: u32 = 10_000;
/// Maximum tolerated positive clock skew for an otherwise valid token.
pub(crate) const MAX_TOKEN_FUTURE_SKEW_SECS: u64 = 60;
/// Payload-free failure categories exposed by a runtime-only stream-token signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StreamTokenSigningError {
    /// The signing provider could not complete the bounded operation.
    #[error("stream-token runtime signer unavailable")]
    Unavailable,
    /// The signing provider refused the canonical request.
    #[error("stream-token runtime signer refused request")]
    Refused,
}
/// Public revision and policy identity reported by the stream-token signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamTokenRuntimeSignerQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}
impl StreamTokenRuntimeSignerQualificationV1 {
    /// Construct one public signer qualification.
    ///
    /// Call [`Self::validate`] before trusting a value returned by an external provider.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }
    /// Return the exact non-zero adapter revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Return the exact non-zero public-policy digest.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }
    /// Reject a zero revision or zero policy digest.
    ///
    /// # Errors
    ///
    /// Returns the precise invalid public field.
    pub fn validate(self) -> Result<(), StreamTokenRuntimeSignerQualificationValueErrorV1> {
        if self.revision == 0 {
            return Err(StreamTokenRuntimeSignerQualificationValueErrorV1::ZeroRevision);
        }
        if self.policy_digest == [0; 32] {
            return Err(StreamTokenRuntimeSignerQualificationValueErrorV1::ZeroPolicyDigest);
        }
        Ok(())
    }
}
/// Invalid public stream-token signer qualification value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamTokenRuntimeSignerQualificationValueErrorV1 {
    /// Adapter revision is zero.
    ZeroRevision,
    /// Public-policy digest is all zeroes.
    ZeroPolicyDigest,
}
/// Payload-free failure while probing a runtime stream-token signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StreamTokenRuntimeSignerProbeErrorV1 {
    /// The configured signing provider is temporarily unavailable.
    #[error("stream-token runtime signer probe unavailable")]
    Unavailable,
    /// The signer identity is stale or revoked.
    #[error("stream-token runtime signer probe stale or revoked")]
    StaleOrRevoked,
}
/// Runtime-only pure-Ed25519 signing boundary for stream-token issuance.
///
/// Implementations own their credentials, sessions, and bounded provider
/// timeout. They must sign the supplied bytes exactly and return the canonical
/// 64-byte `R || S` signature without exposing private key material.
pub trait StreamTokenRuntimeSigner: Send + Sync {
    /// Return the exact opaque runtime handle bound by configuration.
    fn handle(&self) -> &str;
    /// Return the exact Ed25519 public key bound by configuration.
    fn public_key(&self) -> [u8; 32];
    /// Probe the exact active adapter revision and public-policy digest.
    fn qualification(
        &self,
    ) -> Result<StreamTokenRuntimeSignerQualificationV1, StreamTokenRuntimeSignerProbeErrorV1>;
    /// Sign one canonical, domain-separated stream-token payload.
    fn sign(
        &self,
        signing_payload: &[u8],
    ) -> Result<[u8; ed25519_dalek::SIGNATURE_LENGTH], StreamTokenSigningError>;
}
/// Issuer used to sign stream tokens with configured defaults.
pub struct StreamTokenIssuer {
    signer: Arc<dyn StreamTokenRuntimeSigner>,
    expected_signer_handle: String,
    expected_signer_qualification: StreamTokenRuntimeSignerQualificationV1,
    verifying_key: VerifyingKey,
    defaults: TokenDefaults,
    issuance_budgets: Mutex<BTreeMap<StreamTokenQuotaSubject, IssuanceBudget>>,
    max_issuance_budgets: usize,
    max_seen_epoch: AtomicU64,
}
/// Opaque, non-secret identity used for stream-token issuance accounting.
///
/// The subject is derived only after the exact-network operator signature has been authenticated.
/// Display labels such as `X-SoraFS-Client` must never be used to construct quota identities.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StreamTokenQuotaSubject([u8; 32]);
impl StreamTokenQuotaSubject {
    const DERIVATION_CONTEXT: &'static str =
        "iroha.torii.sorafs.stream-token.issuance-quota-subject.v1";
    /// Derive a non-reversible quota subject from an authenticated operator public key.
    pub(crate) fn from_authenticated_operator(public_key: &PublicKey) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(Self::DERIVATION_CONTEXT);
        hasher.update(public_key.to_string().as_bytes());
        Self(*hasher.finalize().as_bytes())
    }
}
/// Default limits applied when overrides are not supplied.
#[derive(Debug, Clone, Copy)]
struct TokenDefaults {
    /// Key version embedded in issued tokens.
    key_version: u32,
    /// Default time-to-live in seconds.
    ttl_secs: u64,
    /// Default concurrent stream limit.
    max_streams: u16,
    /// Default per-token byte budget.
    rate_limit_bytes: u64,
    /// Default per-authenticated-subject issuance quota (requests per minute).
    requests_per_minute: u32,
}
/// Quota accounting snapshot for one authenticated issuance subject.
#[derive(Debug, Clone, Copy)]
struct IssuanceBudget {
    /// Start timestamp of the active quota window.
    window_start: Instant,
    /// Issuances already consumed within the window.
    used: u32,
}
/// Overrides supplied when minting a token.
#[derive(Copy, Clone, Debug, Default)]
pub struct TokenOverrides {
    /// Optional override for the token time-to-live in seconds.
    pub ttl_secs: Option<u64>,
    /// Optional override for the number of concurrent streams allowed.
    pub max_streams: Option<u16>,
    /// Optional override for the per-token byte rate limit.
    pub rate_limit_bytes: Option<u64>,
    /// Optional override for the per-token request quota (requests per minute).
    pub requests_per_minute: Option<u32>,
}
/// Result of a successful token issuance.
#[derive(Debug)]
pub struct TokenIssue {
    /// Signed stream token.
    pub token: StreamTokenV1,
    /// Remaining issuance quota within the current window.
    pub remaining_quota: u32,
}
impl StreamTokenIssuer {
    /// Construct an issuer from the Torii configuration.
    ///
    /// # Errors
    ///
    /// Returns [`StreamTokenIssuerError`] if configuration and the injected
    /// runtime signer do not form one exact, safe binding.
    pub fn from_config(
        config: &actual::SorafsTokenConfig,
        signer: Option<Arc<dyn StreamTokenRuntimeSigner>>,
    ) -> Result<Option<Self>, StreamTokenIssuerError> {
        if !config.enabled {
            if signer.is_some() {
                return Err(StreamTokenIssuerError::UnexpectedRuntimeSigner);
            }
            return Ok(None);
        }
        let signer = signer.ok_or(StreamTokenIssuerError::MissingRuntimeSigner)?;
        let configured_handle = config
            .signer_handle
            .as_ref()
            .ok_or(StreamTokenIssuerError::MissingRuntimeSignerHandle)?;
        validate_production_runtime_handle(configured_handle)
            .map_err(|_| StreamTokenIssuerError::InvalidRuntimeSignerHandle)?;
        let configured_public_key = config
            .signer_public_key
            .ok_or(StreamTokenIssuerError::MissingRuntimeSignerPublicKey)?;
        let verifying_key = VerifyingKey::from_bytes(&configured_public_key)
            .map_err(|_| StreamTokenIssuerError::InvalidRuntimeSignerPublicKey)?;
        if verifying_key.is_weak() {
            return Err(StreamTokenIssuerError::WeakRuntimeSignerPublicKey);
        }
        validate_production_runtime_handle(signer.handle())
            .map_err(|_| StreamTokenIssuerError::InvalidRuntimeSignerHandle)?;
        if signer.handle() != configured_handle {
            return Err(StreamTokenIssuerError::RuntimeSignerHandleMismatch);
        }
        if signer.public_key() != configured_public_key {
            return Err(StreamTokenIssuerError::RuntimeSignerPublicKeyMismatch);
        }
        let configured_revision = config
            .signer_revision
            .ok_or(StreamTokenIssuerError::MissingRuntimeSignerRevision)?;
        let configured_policy_digest = config
            .signer_policy_digest
            .ok_or(StreamTokenIssuerError::MissingRuntimeSignerPolicyDigest)?;
        let expected_signer_qualification = StreamTokenRuntimeSignerQualificationV1::new(
            configured_revision,
            configured_policy_digest,
        );
        expected_signer_qualification
            .validate()
            .map_err(|_| StreamTokenIssuerError::InvalidRuntimeSignerQualification)?;
        let first = signer
            .qualification()
            .map_err(map_runtime_signer_probe_error)?;
        first
            .validate()
            .map_err(|_| StreamTokenIssuerError::InvalidRuntimeSignerQualification)?;
        if first != expected_signer_qualification
            || signer.handle() != configured_handle
            || signer.public_key() != configured_public_key
        {
            return Err(StreamTokenIssuerError::RuntimeSignerQualificationMismatch);
        }
        let second = signer
            .qualification()
            .map_err(map_runtime_signer_probe_error)?;
        second
            .validate()
            .map_err(|_| StreamTokenIssuerError::RuntimeSignerQualificationChanged)?;
        if second != first
            || signer.handle() != configured_handle
            || signer.public_key() != configured_public_key
        {
            return Err(StreamTokenIssuerError::RuntimeSignerQualificationChanged);
        }
        let defaults = TokenDefaults {
            key_version: config.key_version,
            ttl_secs: config.default_ttl_secs,
            max_streams: config.default_max_streams,
            rate_limit_bytes: config.default_rate_limit_bytes,
            requests_per_minute: config.default_requests_per_minute,
        };
        defaults.validate()?;
        Ok(Some(Self {
            signer,
            expected_signer_handle: configured_handle.clone(),
            expected_signer_qualification,
            verifying_key,
            defaults,
            issuance_budgets: Mutex::new(BTreeMap::new()),
            max_issuance_budgets: MAX_ISSUANCE_SUBJECTS,
            max_seen_epoch: AtomicU64::new(0),
        }))
    }
    /// Issue a signed stream token for the provided manifest details.
    ///
    /// # Errors
    ///
    /// Returns [`StreamTokenIssuerError`] when system time overflows, the runtime signer fails, or
    /// the request violates the configured issuance quotas.
    pub(crate) fn issue_token(
        &self,
        quota_subject: StreamTokenQuotaSubject,
        manifest_cid: Vec<u8>,
        provider_id: [u8; 32],
        profile_handle: String,
        overrides: TokenOverrides,
    ) -> Result<TokenIssue, StreamTokenIssuerError> {
        let ttl_secs = checked_override("ttl_secs", overrides.ttl_secs, self.defaults.ttl_secs)?;
        let max_streams = checked_override(
            "max_streams",
            overrides.max_streams,
            self.defaults.max_streams,
        )?;
        let rate_limit_bytes = checked_override(
            "rate_limit_bytes",
            overrides.rate_limit_bytes,
            self.defaults.rate_limit_bytes,
        )?;
        let requests_per_minute = checked_override(
            "requests_per_minute",
            overrides.requests_per_minute,
            self.defaults.requests_per_minute,
        )?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StreamTokenIssuerError::TimeOverflow)?
            .as_secs();
        self.observe_epoch(now)?;
        let ttl_epoch = now
            .checked_add(ttl_secs)
            .ok_or(StreamTokenIssuerError::TimeOverflow)?;
        let body = StreamTokenBodyV1 {
            token_id: new_token_id()?,
            manifest_cid,
            provider_id,
            profile_handle,
            max_streams,
            ttl_epoch,
            rate_limit_bytes,
            issued_at: now,
            requests_per_minute,
            token_pk_version: self.defaults.key_version,
        };
        validate_token_body(&body)?;
        let remaining_quota = self.reserve_issuance_budget(quota_subject, Instant::now())?;
        let signing_payload = body
            .signing_payload_bytes()
            .map_err(StreamTokenError::from)
            .map_err(StreamTokenIssuerError::StreamToken)?;
        self.revalidate_runtime_signer()?;
        let signature = self
            .signer
            .sign(&signing_payload)
            .map_err(|error| match error {
                StreamTokenSigningError::Unavailable => {
                    StreamTokenIssuerError::RuntimeSignerUnavailable
                }
                StreamTokenSigningError::Refused => StreamTokenIssuerError::RuntimeSignerRefused,
            })?;
        self.revalidate_runtime_signer()?;
        let token = StreamTokenV1::from_external_signature(body, signature, &self.verifying_key)
            .map_err(|_| StreamTokenIssuerError::RuntimeSignerOutputInvalid)?;
        Ok(TokenIssue {
            token,
            remaining_quota,
        })
    }
    fn revalidate_runtime_signer(&self) -> Result<(), StreamTokenIssuerError> {
        if self.signer.handle() != self.expected_signer_handle
            || validate_production_runtime_handle(self.signer.handle()).is_err()
            || self.signer.public_key() != self.verifying_key.to_bytes()
        {
            return Err(StreamTokenIssuerError::RuntimeSignerQualificationChanged);
        }
        let qualification = self
            .signer
            .qualification()
            .map_err(map_runtime_signer_probe_error)?;
        qualification
            .validate()
            .map_err(|_| StreamTokenIssuerError::RuntimeSignerQualificationChanged)?;
        if qualification != self.expected_signer_qualification
            || self.signer.handle() != self.expected_signer_handle
            || self.signer.public_key() != self.verifying_key.to_bytes()
        {
            return Err(StreamTokenIssuerError::RuntimeSignerQualificationChanged);
        }
        Ok(())
    }
    /// Return the Ed25519 verifying key bytes.
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
    /// Return a reference to the verifying key used for stream tokens.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
    /// Return the default key version embedded in issued tokens.
    #[must_use]
    pub fn key_version(&self) -> u32 {
        self.defaults.key_version
    }
    fn reserve_issuance_budget(
        &self,
        quota_subject: StreamTokenQuotaSubject,
        now: Instant,
    ) -> Result<u32, StreamTokenIssuerError> {
        let limit = self.defaults.requests_per_minute;
        let mut budgets = self
            .issuance_budgets
            .lock()
            .map_err(|_| StreamTokenIssuerError::IssuanceQuotaStateUnavailable)?;
        budgets.retain(|_, budget| {
            now.saturating_duration_since(budget.window_start) < ISSUANCE_QUOTA_WINDOW
        });
        if let Some(budget) = budgets.get_mut(&quota_subject) {
            let elapsed = now.saturating_duration_since(budget.window_start);
            if budget.used >= limit {
                let remaining =
                    ISSUANCE_QUOTA_WINDOW.saturating_sub(elapsed.min(ISSUANCE_QUOTA_WINDOW));
                let retry_after_secs = remaining
                    .as_secs()
                    .saturating_add(u64::from(remaining.subsec_nanos() != 0))
                    .max(1);
                return Err(StreamTokenIssuerError::IssuanceQuotaExceeded {
                    limit,
                    retry_after_secs,
                });
            }
            budget.used += 1;
            return Ok(limit - budget.used);
        }
        if budgets.len() >= self.max_issuance_budgets {
            return Err(StreamTokenIssuerError::IssuanceQuotaCapacityExceeded {
                capacity: self.max_issuance_budgets,
            });
        }
        budgets.insert(
            quota_subject,
            IssuanceBudget {
                window_start: now,
                used: 1,
            },
        );
        Ok(limit - 1)
    }
    fn observe_epoch(&self, now: u64) -> Result<(), StreamTokenIssuerError> {
        let previous = self.max_seen_epoch.fetch_max(now, Ordering::SeqCst);
        if now < previous {
            return Err(StreamTokenIssuerError::ClockRollback {
                observed_epoch: previous,
                current_epoch: now,
            });
        }
        Ok(())
    }
}
impl TokenDefaults {
    fn validate(self) -> Result<(), StreamTokenIssuerError> {
        validate_bounded_nonzero("key_version", self.key_version, u32::MAX)?;
        validate_bounded_nonzero(
            "default_ttl_secs",
            self.ttl_secs,
            STREAM_TOKEN_MAX_TTL_SECS_V1,
        )?;
        validate_bounded_nonzero("default_max_streams", self.max_streams, MAX_TOKEN_STREAMS)?;
        validate_bounded_nonzero(
            "default_rate_limit_bytes",
            self.rate_limit_bytes,
            MAX_TOKEN_RATE_LIMIT_BYTES,
        )?;
        validate_bounded_nonzero(
            "default_requests_per_minute",
            self.requests_per_minute,
            MAX_TOKEN_REQUESTS_PER_MINUTE,
        )
    }
}
fn validate_bounded_nonzero<T>(
    field: &'static str,
    value: T,
    maximum: T,
) -> Result<(), StreamTokenIssuerError>
where
    T: Copy + Default + Ord + std::fmt::Display,
{
    if value == T::default() || value > maximum {
        return Err(StreamTokenIssuerError::InvalidPolicy {
            field,
            reason: format!("must be between 1 and {maximum} (found {value})"),
        });
    }
    Ok(())
}
fn checked_override<T>(
    field: &'static str,
    requested: Option<T>,
    ceiling: T,
) -> Result<T, StreamTokenIssuerError>
where
    T: Copy + Default + Ord + std::fmt::Display,
{
    let value = requested.unwrap_or(ceiling);
    if value == T::default() || value > ceiling {
        return Err(StreamTokenIssuerError::InvalidPolicy {
            field,
            reason: format!("must be between 1 and the configured ceiling {ceiling}"),
        });
    }
    Ok(value)
}
/// Validate the context-free, canonical v1 stream-token body policy.
pub fn validate_token_body(body: &StreamTokenBodyV1) -> Result<(), StreamTokenBodyError> {
    if body.token_id.len() != TOKEN_ID_HEX_LEN
        || !body
            .token_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StreamTokenBodyError::TokenId);
    }
    if body.manifest_cid.is_empty() || body.manifest_cid.len() > MAX_MANIFEST_CID_BYTES {
        return Err(StreamTokenBodyError::ManifestCid);
    }
    if body.provider_id.iter().all(|byte| *byte == 0) {
        return Err(StreamTokenBodyError::ProviderId);
    }
    if body.profile_handle.is_empty()
        || body.profile_handle.len() > MAX_PROFILE_HANDLE_BYTES
        || !body.profile_handle.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@' | b':')
        })
    {
        return Err(StreamTokenBodyError::ProfileHandle);
    }
    if body.max_streams == 0 || body.max_streams > MAX_TOKEN_STREAMS {
        return Err(StreamTokenBodyError::MaxStreams);
    }
    if body.issued_at == 0 || body.ttl_epoch <= body.issued_at {
        return Err(StreamTokenBodyError::Lifetime);
    }
    if body.ttl_epoch - body.issued_at > STREAM_TOKEN_MAX_TTL_SECS_V1 {
        return Err(StreamTokenBodyError::Lifetime);
    }
    if body.rate_limit_bytes == 0 || body.rate_limit_bytes > MAX_TOKEN_RATE_LIMIT_BYTES {
        return Err(StreamTokenBodyError::RateLimit);
    }
    if body.requests_per_minute == 0 || body.requests_per_minute > MAX_TOKEN_REQUESTS_PER_MINUTE {
        return Err(StreamTokenBodyError::RequestsPerMinute);
    }
    if body.token_pk_version == 0 {
        return Err(StreamTokenBodyError::KeyVersion);
    }
    Ok(())
}
fn new_token_id() -> Result<String, StreamTokenIssuerError> {
    let mut rng = OsRng;
    new_token_id_with_rng(&mut rng)
}
fn new_token_id_with_rng<R: TryCryptoRng>(rng: &mut R) -> Result<String, StreamTokenIssuerError> {
    let mut bytes = [0u8; 16];
    rng.try_fill_bytes(&mut bytes)
        .map_err(|err| StreamTokenIssuerError::RandomBytes {
            operation: "issuing stream token id",
            message: err.to_string(),
        })?;
    Ok(hex::encode(bytes))
}
const fn map_runtime_signer_probe_error(
    error: StreamTokenRuntimeSignerProbeErrorV1,
) -> StreamTokenIssuerError {
    match error {
        StreamTokenRuntimeSignerProbeErrorV1::Unavailable => {
            StreamTokenIssuerError::RuntimeSignerUnavailable
        }
        StreamTokenRuntimeSignerProbeErrorV1::StaleOrRevoked => {
            StreamTokenIssuerError::RuntimeSignerQualificationChanged
        }
    }
}
/// Errors encountered while configuring or issuing stream tokens.
#[derive(Debug, Error)]
pub enum StreamTokenIssuerError {
    /// A signer was injected while stream-token issuance is disabled.
    #[error("stream-token runtime signer injected while issuance is disabled")]
    UnexpectedRuntimeSigner,
    /// Stream-token issuance is enabled without a runtime signer.
    #[error("stream-token issuance requires an injected runtime signer")]
    MissingRuntimeSigner,
    /// The enabled configuration omitted the non-secret runtime handle.
    #[error("stream-token issuance requires a configured runtime signer handle")]
    MissingRuntimeSignerHandle,
    /// The enabled configuration omitted the public verification key.
    #[error("stream-token issuance requires a configured runtime signer public key")]
    MissingRuntimeSignerPublicKey,
    /// The enabled configuration omitted the adapter revision.
    #[error("stream-token issuance requires a configured runtime signer revision")]
    MissingRuntimeSignerRevision,
    /// The enabled configuration omitted the public-policy digest.
    #[error("stream-token issuance requires a configured runtime signer policy digest")]
    MissingRuntimeSignerPolicyDigest,
    /// The configured handle was non-canonical or marked for development use.
    #[error("invalid production stream-token runtime signer handle")]
    InvalidRuntimeSignerHandle,
    /// The configured bytes were not a valid Ed25519 public key.
    #[error("invalid stream-token runtime signer public key")]
    InvalidRuntimeSignerPublicKey,
    /// The configured Ed25519 public key was weak.
    #[error("weak stream-token runtime signer public key")]
    WeakRuntimeSignerPublicKey,
    /// The injected provider did not expose the exact configured handle.
    #[error("stream-token runtime signer handle does not match configuration")]
    RuntimeSignerHandleMismatch,
    /// The injected provider did not expose the exact configured public key.
    #[error("stream-token runtime signer public key does not match configuration")]
    RuntimeSignerPublicKeyMismatch,
    /// The configured or first observed qualification contains a zero field.
    #[error("invalid stream-token runtime signer qualification")]
    InvalidRuntimeSignerQualification,
    /// The injected provider did not expose the exact configured qualification.
    #[error("stream-token runtime signer qualification does not match configuration")]
    RuntimeSignerQualificationMismatch,
    /// The provider identity changed across probes or one signing boundary.
    #[error("stream-token runtime signer qualification changed")]
    RuntimeSignerQualificationChanged,
    /// The bounded provider signing operation was unavailable.
    #[error("stream-token runtime signer unavailable")]
    RuntimeSignerUnavailable,
    /// The signing provider refused the canonical signing request.
    #[error("stream-token runtime signer refused request")]
    RuntimeSignerRefused,
    /// The signing-provider output was malformed or did not verify under the configured public key.
    #[error("stream-token runtime signer produced invalid output")]
    RuntimeSignerOutputInvalid,
    /// A configured or requested token policy was zero, unsafe, or above its ceiling.
    #[error("invalid stream-token policy {field}: {reason}")]
    InvalidPolicy {
        /// Policy field that failed validation.
        field: &'static str,
        /// Human-readable constraint violation.
        reason: String,
    },
    /// The generated token body failed canonical structural validation.
    #[error("invalid stream-token body: {0}")]
    InvalidBody(#[from] StreamTokenBodyError),
    /// System clock produced a timestamp prior to the Unix epoch.
    #[error("system time before UNIX epoch")]
    TimeOverflow,
    /// The system wall clock moved backwards after a later issuance was observed.
    #[error("stream-token issuance clock moved backwards from {observed_epoch} to {current_epoch}")]
    ClockRollback {
        /// Greatest epoch previously observed by this issuer.
        observed_epoch: u64,
        /// Epoch observed for the current issuance attempt.
        current_epoch: u64,
    },
    /// Serialising the canonical stream-token body failed.
    #[error("failed to create stream token: {0}")]
    StreamToken(#[from] StreamTokenError),
    /// Random byte generation failed during stream token issuance.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
    /// The authenticated issuance subject exceeded its per-minute token quota.
    #[error("authenticated subject exceeded token issuance quota ({limit} requests/minute)")]
    IssuanceQuotaExceeded {
        /// Configured quota limit in requests per minute.
        limit: u32,
        /// Recommended retry delay in seconds before issuing another token.
        retry_after_secs: u64,
    },
    /// The bounded set of active issuance subjects is full.
    #[error("stream-token issuance state capacity exhausted ({capacity} active subjects)")]
    IssuanceQuotaCapacityExceeded {
        /// Maximum active issuance budgets retained by this process.
        capacity: usize,
    },
    /// The issuance accounting lock was poisoned; issuance fails closed.
    #[error("stream-token issuance quota state is unavailable")]
    IssuanceQuotaStateUnavailable,
}
/// Canonical structural errors in a v1 stream-token body.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum StreamTokenBodyError {
    /// Token IDs must be exactly 16 random bytes encoded as lowercase hex.
    #[error("token_id must be exactly 32 lowercase hexadecimal characters")]
    TokenId,
    /// Manifest CID bytes were empty or exceeded the protocol ceiling.
    #[error("manifest_cid must contain 1-{MAX_MANIFEST_CID_BYTES} bytes")]
    ManifestCid,
    /// The provider identifier used the reserved all-zero value.
    #[error("provider_id must not be all zero")]
    ProviderId,
    /// The chunk-profile handle was empty, oversized, or non-canonical.
    #[error("profile_handle is not canonical")]
    ProfileHandle,
    /// The concurrency budget was zero or exceeded the v1 ceiling.
    #[error("max_streams must be between 1 and {MAX_TOKEN_STREAMS}")]
    MaxStreams,
    /// The token lifetime was zero, inverted, or exceeded the v1 ceiling.
    #[error(
        "token lifetime must be positive and no more than {STREAM_TOKEN_MAX_TTL_SECS_V1} seconds"
    )]
    Lifetime,
    /// The byte budget was zero or exceeded the v1 ceiling.
    #[error("rate_limit_bytes must be between 1 and {MAX_TOKEN_RATE_LIMIT_BYTES}")]
    RateLimit,
    /// The request quota was zero or exceeded the v1 ceiling.
    #[error("requests_per_minute must be between 1 and {MAX_TOKEN_REQUESTS_PER_MINUTE}")]
    RequestsPerMinute,
    /// Key version zero is reserved and unsupported.
    #[error("token_pk_version must be greater than zero")]
    KeyVersion,
}
/// Errors produced while decoding stream tokens from client headers.
#[derive(Debug, Error)]
pub enum StreamTokenHeaderError {
    /// The encoded header exceeded the strict transport ceiling.
    #[error("stream token header exceeds {maximum} bytes")]
    HeaderTooLong {
        /// Maximum accepted encoded header length.
        maximum: usize,
    },
    /// Header value was not valid base64.
    #[error("stream token header must be base64-encoded")]
    InvalidEncoding,
    /// Base64 text was valid but not in the canonical padded representation.
    #[error("stream token header must use canonical padded base64")]
    NonCanonicalEncoding,
    /// The decoded token frame exceeded the strict wire ceiling.
    #[error("decoded stream token exceeds {maximum} bytes")]
    PayloadTooLong {
        /// Maximum accepted decoded token length.
        maximum: usize,
    },
    /// The decoded token payload failed Norito deserialisation.
    #[error("invalid stream token payload: {0}")]
    InvalidPayload(norito::Error),
    /// The token body or signature shape violated canonical v1 constraints.
    #[error("invalid stream token body: {0}")]
    InvalidBody(#[from] StreamTokenBodyError),
    /// The Ed25519 signature did not have its fixed canonical length.
    #[error("stream token signature must be exactly 64 bytes")]
    InvalidSignatureLength,
}
/// Encode a stream token into base64 suitable for transport headers.
///
/// # Errors
///
/// Returns [`StreamTokenError`] when Norito encoding fails.
pub fn encode_token_base64(token: &StreamTokenV1) -> Result<String, StreamTokenError> {
    let bytes = norito::to_bytes(token)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}
/// Decode a stream token provided in a transport header.
///
/// # Errors
///
/// Returns [`StreamTokenHeaderError`] when the payload is not valid base64 or fails Norito decoding.
pub fn decode_token_base64(value: &str) -> Result<StreamTokenV1, StreamTokenHeaderError> {
    if value.is_empty() {
        return Err(StreamTokenHeaderError::InvalidEncoding);
    }
    if value.len() > MAX_STREAM_TOKEN_BASE64_BYTES {
        return Err(StreamTokenHeaderError::HeaderTooLong {
            maximum: MAX_STREAM_TOKEN_BASE64_BYTES,
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| StreamTokenHeaderError::InvalidEncoding)?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != value {
        return Err(StreamTokenHeaderError::NonCanonicalEncoding);
    }
    if bytes.len() > MAX_STREAM_TOKEN_WIRE_BYTES {
        return Err(StreamTokenHeaderError::PayloadTooLong {
            maximum: MAX_STREAM_TOKEN_WIRE_BYTES,
        });
    }
    let token = norito::decode_from_bytes::<StreamTokenV1>(&bytes)
        .map_err(StreamTokenHeaderError::InvalidPayload)?;
    validate_token_body(&token.body)?;
    if token.signature.len() != ed25519_dalek::SIGNATURE_LENGTH {
        return Err(StreamTokenHeaderError::InvalidSignatureLength);
    }
    Ok(token)
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    struct FailingTryRng;
    #[derive(Debug)]
    struct FailingTryRngError;
    impl std::fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing stream token RNG")
        }
    }
    impl TryRngCore for FailingTryRng {
        type Error = FailingTryRngError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(FailingTryRngError)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(FailingTryRngError)
        }
        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Err(FailingTryRngError)
        }
    }
    impl TryCryptoRng for FailingTryRng {}
    #[derive(Debug, Clone, Copy)]
    enum TestSignerMode {
        Sign,
        Unavailable,
        Refused,
        WrongKey,
        Malformed,
    }
    struct TestStreamTokenRuntimeSigner {
        handle: String,
        signing_key: SigningKey,
        advertised_public_key: [u8; 32],
        first_probe_public_key: Option<[u8; 32]>,
        mode: TestSignerMode,
        qualification: StreamTokenRuntimeSignerQualificationV1,
        qualification_results: Mutex<
            Vec<
                Result<
                    StreamTokenRuntimeSignerQualificationV1,
                    StreamTokenRuntimeSignerProbeErrorV1,
                >,
            >,
        >,
        qualification_calls: std::sync::atomic::AtomicUsize,
        signing_payloads: Mutex<Vec<Vec<u8>>>,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl TestStreamTokenRuntimeSigner {
        fn new(handle: &str, seed: [u8; 32], mode: TestSignerMode) -> Self {
            let signing_key = SigningKey::from_bytes(&seed);
            Self {
                handle: handle.to_owned(),
                advertised_public_key: signing_key.verifying_key().to_bytes(),
                first_probe_public_key: None,
                signing_key,
                mode,
                qualification: StreamTokenRuntimeSignerQualificationV1::new(4, [0xb4; 32]),
                qualification_results: Mutex::new(Vec::new()),
                qualification_calls: std::sync::atomic::AtomicUsize::new(0),
                signing_payloads: Mutex::new(Vec::new()),
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
        fn with_qualification_results(
            mut self,
            results: Vec<
                Result<
                    StreamTokenRuntimeSignerQualificationV1,
                    StreamTokenRuntimeSignerProbeErrorV1,
                >,
            >,
        ) -> Self {
            *self
                .qualification_results
                .get_mut()
                .expect("test qualification results") = results;
            self
        }
        fn with_first_probe_public_key(mut self, public_key: [u8; 32]) -> Self {
            self.first_probe_public_key = Some(public_key);
            self
        }
    }
    impl StreamTokenRuntimeSigner for TestStreamTokenRuntimeSigner {
        fn handle(&self) -> &str {
            &self.handle
        }
        fn public_key(&self) -> [u8; 32] {
            if self.qualification_calls.load(Ordering::Relaxed) == 1 {
                return self
                    .first_probe_public_key
                    .unwrap_or(self.advertised_public_key);
            }
            self.advertised_public_key
        }
        fn qualification(
            &self,
        ) -> Result<StreamTokenRuntimeSignerQualificationV1, StreamTokenRuntimeSignerProbeErrorV1>
        {
            let index = self.qualification_calls.fetch_add(1, Ordering::Relaxed);
            self.qualification_results
                .lock()
                .expect("test qualification results")
                .get(index)
                .copied()
                .unwrap_or(Ok(self.qualification))
        }
        fn sign(
            &self,
            signing_payload: &[u8],
        ) -> Result<[u8; ed25519_dalek::SIGNATURE_LENGTH], StreamTokenSigningError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.signing_payloads
                .lock()
                .expect("test signing payloads")
                .push(signing_payload.to_vec());
            match self.mode {
                TestSignerMode::Sign => Ok(self.signing_key.sign(signing_payload).to_bytes()),
                TestSignerMode::Unavailable => Err(StreamTokenSigningError::Unavailable),
                TestSignerMode::Refused => Err(StreamTokenSigningError::Refused),
                TestSignerMode::WrongKey => Ok(SigningKey::from_bytes(&[0x7a; 32])
                    .sign(signing_payload)
                    .to_bytes()),
                TestSignerMode::Malformed => Ok([0; ed25519_dalek::SIGNATURE_LENGTH]),
            }
        }
    }
    fn token_config(public_key: [u8; 32], requests_per_minute: u32) -> actual::SorafsTokenConfig {
        actual::SorafsTokenConfig {
            enabled: true,
            signer_handle: Some("provider:prod/stream-token/v1".to_owned()),
            signer_public_key: Some(public_key),
            signer_revision: Some(4),
            signer_policy_digest: Some([0xb4; 32]),
            key_version: 1,
            default_ttl_secs: 900,
            default_max_streams: 2,
            default_rate_limit_bytes: 512 * 1024,
            default_requests_per_minute: requests_per_minute,
            ..actual::SorafsTokenConfig::default()
        }
    }
    fn issuer_and_signer(
        limit: u32,
        mode: TestSignerMode,
    ) -> (StreamTokenIssuer, Arc<TestStreamTokenRuntimeSigner>) {
        let signer = Arc::new(TestStreamTokenRuntimeSigner::new(
            "provider:prod/stream-token/v1",
            [0x33; 32],
            mode,
        ));
        let config = token_config(signer.public_key(), limit);
        let runtime_signer: Arc<dyn StreamTokenRuntimeSigner> = signer.clone();
        let issuer = StreamTokenIssuer::from_config(&config, Some(runtime_signer))
            .expect("valid runtime signer binding")
            .expect("enabled issuer");
        (issuer, signer)
    }
    fn sample_body() -> StreamTokenBodyV1 {
        StreamTokenBodyV1 {
            token_id: "0123456789abcdef0123456789abcdef".to_string(),
            manifest_cid: vec![0x01, 0x55, 0x01],
            provider_id: [0xAA; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            max_streams: 4,
            ttl_epoch: 1_731_234_567,
            rate_limit_bytes: 10 * 1024 * 1024,
            issued_at: 1_731_234_000,
            requests_per_minute: 120,
            token_pk_version: 3,
        }
    }
    #[test]
    fn sign_and_verify_roundtrip() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let verifying = signing.verifying_key();
        let body = sample_body();
        let token = StreamTokenV1::sign(body.clone(), &signing).expect("sign");
        token.verify(&verifying).expect("verify");
        assert_eq!(token.body, body);
        let hash = token.body_hash().expect("hash");
        let bytes = body.to_canonical_bytes().expect("bytes");
        assert_eq!(hash.as_bytes(), blake3::hash(&bytes).as_bytes());
    }
    #[test]
    fn new_token_id_reports_rng_failure() {
        let mut rng = FailingTryRng;
        match new_token_id_with_rng(&mut rng) {
            Err(StreamTokenIssuerError::RandomBytes { operation, message }) => {
                assert_eq!(operation, "issuing stream token id");
                assert!(message.contains("failing stream token RNG"));
            }
            Ok(_) => panic!("RNG failure must be reported"),
            Err(other) => panic!("expected RNG failure, got {other:?}"),
        }
    }
    #[test]
    fn signer_qualification_rejects_zero_public_fields() {
        assert_eq!(
            StreamTokenRuntimeSignerQualificationV1::new(0, [0xb4; 32]).validate(),
            Err(StreamTokenRuntimeSignerQualificationValueErrorV1::ZeroRevision)
        );
        assert_eq!(
            StreamTokenRuntimeSignerQualificationV1::new(4, [0; 32]).validate(),
            Err(StreamTokenRuntimeSignerQualificationValueErrorV1::ZeroPolicyDigest)
        );
    }
    #[test]
    fn disabled_issuance_rejects_an_unexpected_runtime_signer() {
        let signer = Arc::new(TestStreamTokenRuntimeSigner::new(
            "provider:prod/stream-token/v1",
            [0x33; 32],
            TestSignerMode::Sign,
        ));
        let runtime_signer: Arc<dyn StreamTokenRuntimeSigner> = signer.clone();
        let mut config = token_config(signer.public_key(), 2);
        config.enabled = false;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer)),
            Err(StreamTokenIssuerError::UnexpectedRuntimeSigner)
        ));
        assert!(
            StreamTokenIssuer::from_config(&config, None)
                .expect("disabled issuance needs no signer")
                .is_none()
        );
    }
    #[test]
    fn runtime_signer_binding_fails_closed() {
        let signer = Arc::new(TestStreamTokenRuntimeSigner::new(
            "provider:prod/stream-token/v1",
            [0x33; 32],
            TestSignerMode::Sign,
        ));
        let runtime_signer: Arc<dyn StreamTokenRuntimeSigner> = signer.clone();
        let mut config = token_config(signer.public_key(), 2);
        assert!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone()))
                .expect("canonical production handle must be accepted")
                .is_some()
        );
        config.enabled = false;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
            Err(StreamTokenIssuerError::UnexpectedRuntimeSigner)
        ));
        config.enabled = true;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, None),
            Err(StreamTokenIssuerError::MissingRuntimeSigner)
        ));
        config.signer_handle = None;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
            Err(StreamTokenIssuerError::MissingRuntimeSignerHandle)
        ));
        for invalid_handle in [
            "mock-stream-token",
            "https://operator:secret@signer",
            "https://signer/path?credential=secret",
            "https://signer/path#fragment",
            "provider:prod/%73tream-token/v1",
            "provider:prod\\stream-token\\v1",
        ] {
            config.signer_handle = Some(invalid_handle.to_owned());
            assert!(matches!(
                StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
                Err(StreamTokenIssuerError::InvalidRuntimeSignerHandle)
            ));
        }
        config.signer_handle = Some("provider:prod/other-token/v1".to_owned());
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
            Err(StreamTokenIssuerError::RuntimeSignerHandleMismatch)
        ));
        config.signer_handle = Some("provider:prod/stream-token/v1".to_owned());
        config.signer_public_key = None;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
            Err(StreamTokenIssuerError::MissingRuntimeSignerPublicKey)
        ));
        let mut weak_public_key = [0; 32];
        weak_public_key[0] = 1;
        config.signer_public_key = Some(weak_public_key);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer.clone())),
            Err(StreamTokenIssuerError::WeakRuntimeSignerPublicKey)
        ));
        config.signer_public_key = Some(
            SigningKey::from_bytes(&[0x34; 32])
                .verifying_key()
                .to_bytes(),
        );
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(runtime_signer)),
            Err(StreamTokenIssuerError::RuntimeSignerPublicKeyMismatch)
        ));
    }
    #[test]
    fn runtime_signer_qualification_fails_closed_at_startup() {
        let signer = Arc::new(TestStreamTokenRuntimeSigner::new(
            "provider:prod/stream-token/v1",
            [0x33; 32],
            TestSignerMode::Sign,
        ));
        let mut config = token_config(signer.public_key(), 2);
        config.signer_revision = None;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(signer.clone())),
            Err(StreamTokenIssuerError::MissingRuntimeSignerRevision)
        ));
        config.signer_revision = Some(4);
        config.signer_policy_digest = None;
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(signer.clone())),
            Err(StreamTokenIssuerError::MissingRuntimeSignerPolicyDigest)
        ));
        config.signer_policy_digest = Some([0; 32]);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(signer.clone())),
            Err(StreamTokenIssuerError::InvalidRuntimeSignerQualification)
        ));
        config.signer_policy_digest = Some([0xb4; 32]);
        config.signer_revision = Some(5);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(signer.clone())),
            Err(StreamTokenIssuerError::RuntimeSignerQualificationMismatch)
        ));
        let unavailable = Arc::new(
            TestStreamTokenRuntimeSigner::new(
                "provider:prod/stream-token/v1",
                [0x33; 32],
                TestSignerMode::Sign,
            )
            .with_qualification_results(vec![Err(
                StreamTokenRuntimeSignerProbeErrorV1::Unavailable,
            )]),
        );
        let config = token_config(unavailable.public_key(), 2);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(unavailable)),
            Err(StreamTokenIssuerError::RuntimeSignerUnavailable)
        ));
        let expected = StreamTokenRuntimeSignerQualificationV1::new(4, [0xb4; 32]);
        let drifted = StreamTokenRuntimeSignerQualificationV1::new(5, [0xb4; 32]);
        let drifting = Arc::new(
            TestStreamTokenRuntimeSigner::new(
                "provider:prod/stream-token/v1",
                [0x33; 32],
                TestSignerMode::Sign,
            )
            .with_qualification_results(vec![Ok(expected), Ok(drifted)]),
        );
        let config = token_config(drifting.public_key(), 2);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(drifting)),
            Err(StreamTokenIssuerError::RuntimeSignerQualificationChanged)
        ));
        let transient_identity_drift = Arc::new(
            TestStreamTokenRuntimeSigner::new(
                "provider:prod/stream-token/v1",
                [0x33; 32],
                TestSignerMode::Sign,
            )
            .with_first_probe_public_key([0x7a; 32]),
        );
        let config = token_config(transient_identity_drift.public_key(), 2);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(transient_identity_drift)),
            Err(StreamTokenIssuerError::RuntimeSignerQualificationMismatch)
        ));
        let test_marked = Arc::new(TestStreamTokenRuntimeSigner::new(
            "provider:test/stream-token/v1",
            [0x33; 32],
            TestSignerMode::Sign,
        ));
        let config = token_config(test_marked.public_key(), 2);
        assert!(matches!(
            StreamTokenIssuer::from_config(&config, Some(test_marked)),
            Err(StreamTokenIssuerError::InvalidRuntimeSignerHandle)
        ));
    }
    #[test]
    fn verify_rejects_modified_body() {
        let signing = SigningKey::from_bytes(&[0x24; 32]);
        let verifying = signing.verifying_key();
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
        let mut tampered = token.clone();
        tampered.body.max_streams = 8;
        let err = tampered.verify(&verifying).expect_err("should fail");
        assert!(matches!(err, StreamTokenError::SignatureInvalid(_)));
    }
    fn issuer_with_limit(limit: u32) -> StreamTokenIssuer {
        issuer_with_capacity(limit, MAX_ISSUANCE_SUBJECTS)
    }
    fn issuer_with_capacity(limit: u32, max_issuance_budgets: usize) -> StreamTokenIssuer {
        let (mut issuer, _) = issuer_and_signer(limit, TestSignerMode::Sign);
        issuer.max_issuance_budgets = max_issuance_budgets;
        issuer
    }
    fn quota_subject(label: &str) -> StreamTokenQuotaSubject {
        let seed = *blake3::hash(label.as_bytes()).as_bytes();
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = PublicKey::from_bytes(
            iroha_crypto::Algorithm::Ed25519,
            &signing_key.verifying_key().to_bytes(),
        )
        .expect("derived operator fixture key");
        StreamTokenQuotaSubject::from_authenticated_operator(&public_key)
    }
    #[test]
    fn issuer_signs_exact_payload_and_verifies_before_release() {
        let (issuer, signer) = issuer_and_signer(2, TestSignerMode::Sign);
        let issue = issuer
            .issue_token(
                quota_subject("credential-exact"),
                vec![0xAA],
                [0x11; 32],
                "sorafs.sf1@1.0.0".to_owned(),
                TokenOverrides::default(),
            )
            .expect("issue verified token");
        let payloads = signer
            .signing_payloads
            .lock()
            .expect("captured signing payloads");
        assert_eq!(
            payloads.as_slice(),
            [issue
                .token
                .body
                .signing_payload_bytes()
                .expect("canonical signing payload")]
        );
        issue
            .token
            .verify(issuer.verifying_key())
            .expect("issuer must release only a strictly verified token");
        assert_eq!(
            signer.qualification_calls.load(Ordering::Relaxed),
            4,
            "startup and the signing boundary each require two public probes"
        );
    }
    #[test]
    fn runtime_signer_qualification_is_fenced_before_and_after_signing() {
        let expected = StreamTokenRuntimeSignerQualificationV1::new(4, [0xb4; 32]);
        let drifted = StreamTokenRuntimeSignerQualificationV1::new(5, [0xb4; 32]);
        for (label, reports, expected_sign_calls) in [
            (
                "before signing",
                vec![Ok(expected), Ok(expected), Ok(drifted)],
                0,
            ),
            (
                "after signing",
                vec![Ok(expected), Ok(expected), Ok(expected), Ok(drifted)],
                1,
            ),
        ] {
            let signer = Arc::new(
                TestStreamTokenRuntimeSigner::new(
                    "provider:prod/stream-token/v1",
                    [0x33; 32],
                    TestSignerMode::Sign,
                )
                .with_qualification_results(reports),
            );
            let config = token_config(signer.public_key(), 1);
            let issuer = StreamTokenIssuer::from_config(&config, Some(signer.clone()))
                .expect("startup qualification")
                .expect("enabled issuer");
            assert!(
                matches!(
                    issuer.issue_token(
                        quota_subject("credential-drift"),
                        vec![0xAA],
                        [0x11; 32],
                        "sorafs.sf1@1.0.0".to_owned(),
                        TokenOverrides::default(),
                    ),
                    Err(StreamTokenIssuerError::RuntimeSignerQualificationChanged)
                ),
                "{label}"
            );
            assert_eq!(
                signer.calls.load(Ordering::Relaxed),
                expected_sign_calls,
                "{label}"
            );
            assert!(matches!(
                issuer.issue_token(
                    quota_subject("credential-drift"),
                    vec![0xAA],
                    [0x11; 32],
                    "sorafs.sf1@1.0.0".to_owned(),
                    TokenOverrides::default(),
                ),
                Err(StreamTokenIssuerError::IssuanceQuotaExceeded { .. })
            ));
        }
    }
    #[test]
    fn runtime_signer_probe_unavailability_before_signing_is_payload_free() {
        let expected = StreamTokenRuntimeSignerQualificationV1::new(4, [0xb4; 32]);
        let signer = Arc::new(
            TestStreamTokenRuntimeSigner::new(
                "provider:prod/stream-token/v1",
                [0x33; 32],
                TestSignerMode::Sign,
            )
            .with_qualification_results(vec![
                Ok(expected),
                Ok(expected),
                Err(StreamTokenRuntimeSignerProbeErrorV1::Unavailable),
            ]),
        );
        let config = token_config(signer.public_key(), 1);
        let issuer = StreamTokenIssuer::from_config(&config, Some(signer.clone()))
            .expect("startup qualification")
            .expect("enabled issuer");
        let error = issuer
            .issue_token(
                quota_subject("credential-unavailable-probe"),
                vec![0xAA],
                [0x11; 32],
                "sorafs.sf1@1.0.0".to_owned(),
                TokenOverrides::default(),
            )
            .expect_err("unavailable provider probe must fail closed");
        assert!(matches!(
            error,
            StreamTokenIssuerError::RuntimeSignerUnavailable
        ));
        assert_eq!(error.to_string(), "stream-token runtime signer unavailable");
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn runtime_signer_failures_are_payload_free_and_consume_reserved_quota() {
        for (mode, expected) in [
            (
                TestSignerMode::Unavailable,
                StreamTokenIssuerError::RuntimeSignerUnavailable,
            ),
            (
                TestSignerMode::Refused,
                StreamTokenIssuerError::RuntimeSignerRefused,
            ),
        ] {
            let (issuer, signer) = issuer_and_signer(1, mode);
            let error = issuer
                .issue_token(
                    quota_subject("credential-provider"),
                    vec![0xAA],
                    [0x11; 32],
                    "sorafs.sf1@1.0.0".to_owned(),
                    TokenOverrides::default(),
                )
                .expect_err("runtime signer failure must fail issuance");
            assert_eq!(
                std::mem::discriminant(&error),
                std::mem::discriminant(&expected)
            );
            assert_eq!(error.to_string(), expected.to_string());
            assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
            assert!(matches!(
                issuer.issue_token(
                    quota_subject("credential-provider"),
                    vec![0xAA],
                    [0x11; 32],
                    "sorafs.sf1@1.0.0".to_owned(),
                    TokenOverrides::default(),
                ),
                Err(StreamTokenIssuerError::IssuanceQuotaExceeded { .. })
            ));
            assert_eq!(
                signer.calls.load(Ordering::Relaxed),
                1,
                "quota must be reserved before calling the external signer"
            );
        }
    }
    #[test]
    fn invalid_runtime_signer_output_never_releases_a_token() {
        for mode in [TestSignerMode::WrongKey, TestSignerMode::Malformed] {
            let (issuer, signer) = issuer_and_signer(2, mode);
            assert!(matches!(
                issuer.issue_token(
                    quota_subject("credential-invalid-output"),
                    vec![0xAA],
                    [0x11; 32],
                    "sorafs.sf1@1.0.0".to_owned(),
                    TokenOverrides::default(),
                ),
                Err(StreamTokenIssuerError::RuntimeSignerOutputInvalid)
            ));
            assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
        }
    }
    #[test]
    fn authenticated_subject_quota_is_enforced() {
        let issuer = issuer_with_limit(2);
        let provider = [0x11; 32];
        let subject = quota_subject("credential-a");
        let overrides = TokenOverrides {
            requests_per_minute: Some(2),
            ..TokenOverrides::default()
        };
        let first = issuer
            .issue_token(
                subject,
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect("first token");
        assert_eq!(first.remaining_quota, 1);
        let second = issuer
            .issue_token(
                subject,
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect("second token");
        assert_eq!(second.remaining_quota, 0);
        let err = issuer
            .issue_token(
                subject,
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect_err("quota exceeded");
        assert!(matches!(
            err,
            StreamTokenIssuerError::IssuanceQuotaExceeded { .. }
        ));
        if let Some(entry) = issuer
            .issuance_budgets
            .lock()
            .expect("issuance budgets")
            .get_mut(&subject)
        {
            if let Some(reset) =
                Instant::now().checked_sub(ISSUANCE_QUOTA_WINDOW + Duration::from_secs(1))
            {
                entry.window_start = reset;
            }
            entry.used = 2;
        }
        let refreshed = issuer
            .issue_token(
                subject,
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides,
            )
            .expect("quota reset");
        assert_eq!(refreshed.remaining_quota, 1);
    }
    #[test]
    fn zero_and_above_ceiling_overrides_fail_closed() {
        let issuer = issuer_with_limit(2);
        let provider = [0x22; 32];
        for overrides in [
            TokenOverrides {
                ttl_secs: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                ttl_secs: Some(901),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                max_streams: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                max_streams: Some(3),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                rate_limit_bytes: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                rate_limit_bytes: Some(512 * 1024 + 1),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                requests_per_minute: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                requests_per_minute: Some(3),
                ..TokenOverrides::default()
            },
        ] {
            assert!(matches!(
                issuer.issue_token(
                    quota_subject("credential-free"),
                    vec![0xBB],
                    provider,
                    "sorafs.sf1@1.0.0".to_string(),
                    overrides,
                ),
                Err(StreamTokenIssuerError::InvalidPolicy { .. })
            ));
        }
        let valid = issuer
            .issue_token(
                quota_subject("credential-free"),
                vec![0xBB],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            )
            .expect("invalid requests must not consume issuance quota");
        assert_eq!(valid.remaining_quota, 1);
    }
    #[test]
    fn issuance_state_capacity_fails_closed_and_prunes_idle_subjects() {
        let issuer = issuer_with_capacity(2, 2);
        for credential in ["credential-a", "credential-b"] {
            issuer
                .issue_token(
                    quota_subject(credential),
                    vec![0xBB],
                    [0x22; 32],
                    "sorafs.sf1@1.0.0".to_string(),
                    TokenOverrides::default(),
                )
                .expect("client admitted");
        }
        assert!(matches!(
            issuer.issue_token(
                quota_subject("credential-c"),
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            ),
            Err(StreamTokenIssuerError::IssuanceQuotaCapacityExceeded { capacity: 2 })
        ));
        let stale = Instant::now()
            .checked_sub(ISSUANCE_QUOTA_WINDOW + Duration::from_secs(1))
            .expect("stale instant");
        issuer
            .issuance_budgets
            .lock()
            .expect("issuance budgets")
            .get_mut(&quota_subject("credential-a"))
            .expect("credential-a")
            .window_start = stale;
        issuer
            .issue_token(
                quota_subject("credential-c"),
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            )
            .expect("stale client pruned before capacity check");
    }
    #[test]
    fn concurrent_issuance_never_exceeds_authenticated_subject_budget() {
        use std::{
            sync::{Arc, Barrier, atomic::AtomicUsize, atomic::Ordering},
            thread,
        };
        const THREADS: usize = 32;
        const LIMIT: u32 = 7;
        let issuer = Arc::new(issuer_with_limit(LIMIT));
        let barrier = Arc::new(Barrier::new(THREADS));
        let successes = Arc::new(AtomicUsize::new(0));
        let mut joins = Vec::with_capacity(THREADS);
        for _ in 0..THREADS {
            let issuer = Arc::clone(&issuer);
            let barrier = Arc::clone(&barrier);
            let successes = Arc::clone(&successes);
            joins.push(thread::spawn(move || {
                barrier.wait();
                match issuer.issue_token(
                    quota_subject("credential-race"),
                    vec![0xBB],
                    [0x22; 32],
                    "sorafs.sf1@1.0.0".to_string(),
                    TokenOverrides::default(),
                ) {
                    Ok(_) => {
                        successes.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(StreamTokenIssuerError::IssuanceQuotaExceeded { .. }) => {}
                    Err(other) => panic!("unexpected issuance error: {other}"),
                }
            }));
        }
        for join in joins {
            join.join().expect("issuance worker");
        }
        assert_eq!(successes.load(Ordering::Relaxed), LIMIT as usize);
    }
    #[test]
    fn poisoned_issuance_state_fails_closed() {
        use std::{sync::Arc, thread};
        let issuer = Arc::new(issuer_with_limit(2));
        let poisoner = Arc::clone(&issuer);
        let poisoned = thread::spawn(move || {
            let _guard = poisoner.issuance_budgets.lock().expect("issuance lock");
            panic!("poison issuance state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");
        assert!(matches!(
            issuer.issue_token(
                quota_subject("credential-a"),
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            ),
            Err(StreamTokenIssuerError::IssuanceQuotaStateUnavailable)
        ));
    }
    #[test]
    fn issuance_wall_clock_rollback_fails_closed() {
        let issuer = issuer_with_limit(2);
        issuer.observe_epoch(100).expect("initial epoch");
        assert!(matches!(
            issuer.observe_epoch(99),
            Err(StreamTokenIssuerError::ClockRollback {
                observed_epoch: 100,
                current_epoch: 99,
            })
        ));
    }
    #[test]
    fn canonical_body_validation_rejects_each_unsafe_dimension() {
        let mut cases = Vec::new();
        let mut body = sample_body();
        body.token_id = "ABC".to_string();
        cases.push((body, StreamTokenBodyError::TokenId));
        let mut body = sample_body();
        body.manifest_cid.clear();
        cases.push((body, StreamTokenBodyError::ManifestCid));
        let mut body = sample_body();
        body.provider_id = [0; 32];
        cases.push((body, StreamTokenBodyError::ProviderId));
        let mut body = sample_body();
        body.profile_handle = "sorafs profile".to_string();
        cases.push((body, StreamTokenBodyError::ProfileHandle));
        let mut body = sample_body();
        body.max_streams = 0;
        cases.push((body, StreamTokenBodyError::MaxStreams));
        let mut body = sample_body();
        body.ttl_epoch = body.issued_at;
        cases.push((body, StreamTokenBodyError::Lifetime));
        let mut body = sample_body();
        body.rate_limit_bytes = 0;
        cases.push((body, StreamTokenBodyError::RateLimit));
        let mut body = sample_body();
        body.requests_per_minute = 0;
        cases.push((body, StreamTokenBodyError::RequestsPerMinute));
        let mut body = sample_body();
        body.token_pk_version = 0;
        cases.push((body, StreamTokenBodyError::KeyVersion));
        for (body, expected) in cases {
            assert_eq!(validate_token_body(&body), Err(expected));
        }
    }
    #[test]
    fn canonical_body_accepts_exact_maximum_lifetime_and_rejects_max_plus_one() {
        let mut maximum = sample_body();
        maximum.issued_at = 1_700_000_000;
        maximum.ttl_epoch = maximum.issued_at + STREAM_TOKEN_MAX_TTL_SECS_V1;
        validate_token_body(&maximum).expect("exact maximum lifetime");
        maximum.ttl_epoch += 1;
        assert_eq!(
            validate_token_body(&maximum),
            Err(StreamTokenBodyError::Lifetime)
        );
    }
    #[test]
    fn base64_decoder_enforces_canonical_bounded_frame_and_body() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
        let encoded = encode_token_base64(&token).expect("encode");
        assert_eq!(decode_token_base64(&encoded).expect("decode"), token);
        assert!(matches!(
            decode_token_base64(&"A".repeat(MAX_STREAM_TOKEN_BASE64_BYTES + 1)),
            Err(StreamTokenHeaderError::HeaderTooLong { .. })
        ));
        let oversized_wire =
            base64::engine::general_purpose::STANDARD
                .encode(vec![0_u8; MAX_STREAM_TOKEN_WIRE_BYTES + 1]);
        assert!(matches!(
            decode_token_base64(&oversized_wire),
            Err(StreamTokenHeaderError::PayloadTooLong { .. })
        ));
        let mut invalid_body = sample_body();
        invalid_body.provider_id = [0; 32];
        let invalid_token = StreamTokenV1::sign(invalid_body, &signing).expect("sign invalid body");
        let invalid_encoded = encode_token_base64(&invalid_token).expect("encode invalid body");
        assert!(matches!(
            decode_token_base64(&invalid_encoded),
            Err(StreamTokenHeaderError::InvalidBody(
                StreamTokenBodyError::ProviderId
            ))
        ));
        let mut short_signature = token;
        short_signature.signature.pop();
        let short_encoded = encode_token_base64(&short_signature).expect("encode short signature");
        assert!(matches!(
            decode_token_base64(&short_encoded),
            Err(StreamTokenHeaderError::InvalidSignatureLength)
        ));
    }
}
