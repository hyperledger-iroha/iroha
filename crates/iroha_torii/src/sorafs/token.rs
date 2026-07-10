//! Stream token issuance helpers for Torii chunk-range gateways.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use iroha_config::parameters::actual;
use rand::{
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::OsRng,
};
use sorafs_manifest::{
    STREAM_TOKEN_MAX_TTL_SECS_V1, StreamTokenBodyV1, StreamTokenError, StreamTokenV1,
};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

/// Fixed rolling window applied to per-client issuance quotas.
const CLIENT_QUOTA_WINDOW: Duration = Duration::from_mins(1);
/// Maximum number of active issuance-client budgets retained by one gateway.
const MAX_ISSUANCE_CLIENTS: usize = 4_096;
/// Maximum accepted encoded token header length.
pub(crate) const MAX_STREAM_TOKEN_BASE64_BYTES: usize = 4_096;
/// Maximum accepted decoded token frame length.
const MAX_STREAM_TOKEN_WIRE_BYTES: usize = 2_048;
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
/// Maximum per-token and per-client request budget.
const MAX_TOKEN_REQUESTS_PER_MINUTE: u32 = 10_000;
/// Maximum tolerated positive clock skew for an otherwise valid token.
pub(crate) const MAX_TOKEN_FUTURE_SKEW_SECS: u64 = 60;
/// Maximum supported signing-key file size (hex seed plus one newline).
const MAX_SIGNING_KEY_FILE_BYTES: u64 = 65;

/// Issuer used to sign stream tokens with configured defaults.
pub struct StreamTokenIssuer {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    defaults: TokenDefaults,
    client_budgets: Mutex<BTreeMap<String, ClientBudget>>,
    max_client_budgets: usize,
    max_seen_epoch: AtomicU64,
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
    /// Default per-client issuance quota (requests per minute).
    requests_per_minute: u32,
}

/// Quota accounting snapshot for a client.
#[derive(Debug, Clone, Copy)]
struct ClientBudget {
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
    /// Returns [`StreamTokenIssuerError`] if the signing key is not configured or fails to load.
    pub fn from_config(
        config: &actual::SorafsTokenConfig,
    ) -> Result<Option<Self>, StreamTokenIssuerError> {
        if !config.enabled {
            return Ok(None);
        }

        let path = config
            .signing_key_path
            .as_ref()
            .ok_or(StreamTokenIssuerError::MissingSigningKeyPath)?;
        let signing_key = load_signing_key(path)?;
        let verifying_key = signing_key.verifying_key();
        if verifying_key.is_weak() {
            return Err(StreamTokenIssuerError::WeakSigningKey { path: path.clone() });
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
            signing_key,
            verifying_key,
            defaults,
            client_budgets: Mutex::new(BTreeMap::new()),
            max_client_budgets: MAX_ISSUANCE_CLIENTS,
            max_seen_epoch: AtomicU64::new(0),
        }))
    }

    /// Issue a signed stream token for the provided manifest details.
    ///
    /// # Errors
    ///
    /// Returns [`StreamTokenIssuerError`] when system time overflows, key material is invalid,
    /// or the request violates the configured issuance quotas.
    pub fn issue_token(
        &self,
        client_id: &str,
        manifest_cid: Vec<u8>,
        provider_id: [u8; 32],
        profile_handle: String,
        overrides: TokenOverrides,
    ) -> Result<TokenIssue, StreamTokenIssuerError> {
        validate_client_id(client_id)?;
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

        let token = StreamTokenV1::sign(body, &self.signing_key)
            .map_err(StreamTokenIssuerError::StreamToken)?;
        let remaining_quota = self.reserve_client_budget(client_id, Instant::now())?;

        Ok(TokenIssue {
            token,
            remaining_quota,
        })
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

    /// Sign an arbitrary payload with the gateway's Ed25519 signing key.
    ///
    /// PoTR receipts reuse this helper until dedicated key rotation lands.
    pub fn sign_bytes(&self, message: &[u8]) -> ed25519_dalek::Signature {
        self.signing_key.sign(message)
    }

    /// Return the default key version embedded in issued tokens.
    #[must_use]
    pub fn key_version(&self) -> u32 {
        self.defaults.key_version
    }

    fn reserve_client_budget(
        &self,
        client_id: &str,
        now: Instant,
    ) -> Result<u32, StreamTokenIssuerError> {
        let limit = self.defaults.requests_per_minute;
        let mut budgets = self
            .client_budgets
            .lock()
            .map_err(|_| StreamTokenIssuerError::ClientQuotaStateUnavailable)?;
        budgets.retain(|_, budget| {
            now.saturating_duration_since(budget.window_start) < CLIENT_QUOTA_WINDOW
        });

        if let Some(budget) = budgets.get_mut(client_id) {
            let elapsed = now.saturating_duration_since(budget.window_start);
            if budget.used >= limit {
                let remaining =
                    CLIENT_QUOTA_WINDOW.saturating_sub(elapsed.min(CLIENT_QUOTA_WINDOW));
                let retry_after_secs = remaining
                    .as_secs()
                    .saturating_add(u64::from(remaining.subsec_nanos() != 0))
                    .max(1);
                return Err(StreamTokenIssuerError::ClientQuotaExceeded {
                    client_id: client_id.to_owned(),
                    limit,
                    retry_after_secs,
                });
            }
            budget.used += 1;
            return Ok(limit - budget.used);
        }

        if budgets.len() >= self.max_client_budgets {
            return Err(StreamTokenIssuerError::ClientQuotaCapacityExceeded {
                capacity: self.max_client_budgets,
            });
        }
        budgets.insert(
            client_id.to_owned(),
            ClientBudget {
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

fn validate_client_id(client_id: &str) -> Result<(), StreamTokenIssuerError> {
    if client_id.is_empty() || client_id.len() > MAX_CLIENT_ID_BYTES {
        return Err(StreamTokenIssuerError::InvalidClientId);
    }
    if !client_id.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(StreamTokenIssuerError::InvalidClientId);
    }
    Ok(())
}

/// Validate the context-free, canonical v1 stream-token body policy.
pub(crate) fn validate_token_body(body: &StreamTokenBodyV1) -> Result<(), StreamTokenBodyError> {
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

fn load_signing_key(path: &Path) -> Result<SigningKey, StreamTokenIssuerError> {
    let path = path.to_path_buf();
    let path_metadata =
        fs::symlink_metadata(&path).map_err(|source| StreamTokenIssuerError::SigningKeyIo {
            path: path.clone(),
            source,
        })?;
    validate_signing_key_metadata(&path, &path_metadata)?;

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options
        .open(&path)
        .map_err(|source| StreamTokenIssuerError::SigningKeyIo {
            path: path.clone(),
            source,
        })?;
    let opened_metadata =
        file.metadata()
            .map_err(|source| StreamTokenIssuerError::SigningKeyIo {
                path: path.clone(),
                source,
            })?;
    validate_signing_key_metadata(&path, &opened_metadata)?;
    if !metadata_identifies_same_file(&path_metadata, &opened_metadata) {
        return Err(StreamTokenIssuerError::SigningKeyChanged { path });
    }

    let mut raw = Vec::with_capacity(MAX_SIGNING_KEY_FILE_BYTES as usize);
    (&mut file)
        .take(MAX_SIGNING_KEY_FILE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|source| StreamTokenIssuerError::SigningKeyIo {
            path: path.clone(),
            source,
        })?;
    if raw.len() as u64 > MAX_SIGNING_KEY_FILE_BYTES {
        return Err(StreamTokenIssuerError::SigningKeyTooLarge {
            path,
            maximum: MAX_SIGNING_KEY_FILE_BYTES,
        });
    }

    let final_opened_metadata =
        file.metadata()
            .map_err(|source| StreamTokenIssuerError::SigningKeyIo {
                path: path.clone(),
                source,
            })?;
    let final_path_metadata =
        fs::symlink_metadata(&path).map_err(|source| StreamTokenIssuerError::SigningKeyIo {
            path: path.clone(),
            source,
        })?;
    validate_signing_key_metadata(&path, &final_path_metadata)?;
    if opened_metadata.len() != raw.len() as u64
        || !metadata_identifies_same_file(&opened_metadata, &final_opened_metadata)
        || !metadata_identifies_same_file(&opened_metadata, &final_path_metadata)
    {
        return Err(StreamTokenIssuerError::SigningKeyChanged { path });
    }

    let hex_bytes = raw.strip_suffix(b"\n").unwrap_or(&raw);
    let key_bytes = if hex_bytes.len() == 64
        && hex_bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        hex::decode(hex_bytes).map_err(|source| StreamTokenIssuerError::SigningKeyDecode {
            path: path.clone(),
            source,
        })?
    } else {
        raw
    };

    if key_bytes.len() != 32 {
        return Err(StreamTokenIssuerError::SigningKeyLength {
            path: path.clone(),
            len: key_bytes.len(),
        });
    }
    if key_bytes.iter().all(|byte| *byte == 0) {
        return Err(StreamTokenIssuerError::SigningKeyMaterial { path: path.clone() });
    }

    let mut array = [0u8; 32];
    array.copy_from_slice(&key_bytes);
    Ok(SigningKey::from_bytes(&array))
}

fn validate_signing_key_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), StreamTokenIssuerError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StreamTokenIssuerError::SigningKeyNotRegular {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > MAX_SIGNING_KEY_FILE_BYTES {
        return Err(StreamTokenIssuerError::SigningKeyTooLarge {
            path: path.to_path_buf(),
            maximum: MAX_SIGNING_KEY_FILE_BYTES,
        });
    }
    #[cfg(unix)]
    {
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(StreamTokenIssuerError::SigningKeyPermissions {
                path: path.to_path_buf(),
                mode: metadata.permissions().mode() & 0o777,
            });
        }
        if metadata.nlink() != 1 {
            return Err(StreamTokenIssuerError::SigningKeyLinkCount {
                path: path.to_path_buf(),
                links: metadata.nlink(),
            });
        }
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut OpenOptions) {}

/// Errors encountered while configuring or issuing stream tokens.
#[derive(Debug, Error)]
pub enum StreamTokenIssuerError {
    /// Stream tokens are enabled in configuration but no signing key path was supplied.
    #[error("stream tokens enabled but signing key path not configured")]
    MissingSigningKeyPath,
    /// Reading the configured signing key file failed.
    #[error("failed to read signing key from {path:?}: {source}")]
    SigningKeyIo {
        /// Path to the Ed25519 signing key file.
        path: PathBuf,
        /// Underlying I/O error raised while reading the file.
        source: std::io::Error,
    },
    /// The configured key path did not name one regular, non-symlink file.
    #[error("stream-token signing key at {path:?} must be a regular non-symlink file")]
    SigningKeyNotRegular {
        /// Configured signing-key path.
        path: PathBuf,
    },
    /// The key file exceeded the bounded canonical representation.
    #[error("stream-token signing key at {path:?} exceeds {maximum} bytes")]
    SigningKeyTooLarge {
        /// Configured signing-key path.
        path: PathBuf,
        /// Maximum accepted file length.
        maximum: u64,
    },
    /// The key file grants access to group or other users.
    #[error("stream-token signing key at {path:?} has insecure mode {mode:o}")]
    SigningKeyPermissions {
        /// Configured signing-key path.
        path: PathBuf,
        /// Observed Unix permission mode.
        mode: u32,
    },
    /// The key file has another hard-link name and cannot be trusted as an isolated secret.
    #[error("stream-token signing key at {path:?} must have one link, found {links}")]
    SigningKeyLinkCount {
        /// Configured signing-key path.
        path: PathBuf,
        /// Observed hard-link count.
        links: u64,
    },
    /// The key file changed while it was being read.
    #[error("stream-token signing key at {path:?} changed while being read")]
    SigningKeyChanged {
        /// Configured signing-key path.
        path: PathBuf,
    },
    /// The signing key file contents could not be decoded as hex.
    #[error("failed to decode signing key from {path:?}: {source}")]
    SigningKeyDecode {
        /// Path to the Ed25519 signing key file.
        path: PathBuf,
        /// Hex decoding error describing the failure.
        source: hex::FromHexError,
    },
    /// The signing key file did not have the expected length in bytes.
    #[error("signing key at {path:?} must be 32 bytes, found {len}")]
    SigningKeyLength {
        /// Path to the Ed25519 signing key file.
        path: PathBuf,
        /// Actual byte length present in the file.
        len: usize,
    },
    /// The signing key file contained inert all-zero seed material.
    #[error("signing key at {path:?} must not be all zero")]
    SigningKeyMaterial {
        /// Path to the Ed25519 signing key file.
        path: PathBuf,
    },
    /// The seed resolved to a weak Ed25519 public key.
    #[error("stream-token signing key at {path:?} resolves to a weak public key")]
    WeakSigningKey {
        /// Configured signing-key path.
        path: PathBuf,
    },
    /// A configured or requested token policy was zero, unsafe, or above its ceiling.
    #[error("invalid stream-token policy {field}: {reason}")]
    InvalidPolicy {
        /// Policy field that failed validation.
        field: &'static str,
        /// Human-readable constraint violation.
        reason: String,
    },
    /// The issuance client identifier was empty, oversized, or non-canonical.
    #[error("stream-token client identifier must be 1-{MAX_CLIENT_ID_BYTES} visible ASCII bytes")]
    InvalidClientId,
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
    /// Serialising or signing the stream token body failed.
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
    /// The issuing client exceeded their per-minute token quota.
    #[error("client {client_id} exceeded token issuance quota ({limit} requests/minute)")]
    ClientQuotaExceeded {
        /// Identifier of the client whose quota was exceeded.
        client_id: String,
        /// Configured quota limit in requests per minute.
        limit: u32,
        /// Recommended retry delay in seconds before issuing another token.
        retry_after_secs: u64,
    },
    /// The bounded set of active issuance clients is full.
    #[error("stream-token issuance state capacity exhausted ({capacity} active clients)")]
    ClientQuotaCapacityExceeded {
        /// Maximum active client budgets retained by this process.
        capacity: usize,
    },
    /// The issuance accounting lock was poisoned; issuance fails closed.
    #[error("stream-token issuance quota state is unavailable")]
    ClientQuotaStateUnavailable,
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
    use ed25519_dalek::{Signer, SigningKey};
    use tempfile::NamedTempFile;

    use super::*;

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
    fn load_signing_key_rejects_all_zero_raw_seed_material() {
        let key_file = NamedTempFile::new().expect("create key file");
        std::fs::write(key_file.path(), [0u8; 32]).expect("write key file");
        let path = key_file.path().to_path_buf();

        match load_signing_key(&path) {
            Err(StreamTokenIssuerError::SigningKeyMaterial { path: err_path }) => {
                assert_eq!(err_path, path);
            }
            Ok(_) => panic!("all-zero raw signing key must fail"),
            Err(other) => panic!("expected all-zero signing key error, got {other:?}"),
        }
    }

    #[test]
    fn load_signing_key_rejects_all_zero_hex_seed_material() {
        let key_file = NamedTempFile::new().expect("create key file");
        std::fs::write(key_file.path(), "00".repeat(32)).expect("write key file");
        let path = key_file.path().to_path_buf();

        match load_signing_key(&path) {
            Err(StreamTokenIssuerError::SigningKeyMaterial { path: err_path }) => {
                assert_eq!(err_path, path);
            }
            Ok(_) => panic!("all-zero hex signing key must fail"),
            Err(other) => panic!("expected all-zero signing key error, got {other:?}"),
        }
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
        issuer_with_capacity(limit, MAX_ISSUANCE_CLIENTS)
    }

    fn issuer_with_capacity(limit: u32, max_client_budgets: usize) -> StreamTokenIssuer {
        StreamTokenIssuer {
            signing_key: SigningKey::from_bytes(&[0x33; 32]),
            verifying_key: SigningKey::from_bytes(&[0x33; 32]).verifying_key(),
            defaults: TokenDefaults {
                key_version: 1,
                ttl_secs: 900,
                max_streams: 2,
                rate_limit_bytes: 512 * 1024,
                requests_per_minute: limit,
            },
            client_budgets: Mutex::new(BTreeMap::new()),
            max_client_budgets,
            max_seen_epoch: AtomicU64::new(0),
        }
    }

    #[test]
    fn client_quota_is_enforced() {
        let issuer = issuer_with_limit(2);
        let provider = [0x11; 32];
        let overrides = TokenOverrides {
            requests_per_minute: Some(2),
            ..TokenOverrides::default()
        };

        let first = issuer
            .issue_token(
                "client-a",
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect("first token");
        assert_eq!(first.remaining_quota, 1);

        let second = issuer
            .issue_token(
                "client-a",
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect("second token");
        assert_eq!(second.remaining_quota, 0);

        let err = issuer
            .issue_token(
                "client-a",
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect_err("quota exceeded");
        assert!(matches!(
            err,
            StreamTokenIssuerError::ClientQuotaExceeded { .. }
        ));

        if let Some(entry) = issuer
            .client_budgets
            .lock()
            .expect("client budgets")
            .get_mut("client-a")
        {
            if let Some(reset) =
                Instant::now().checked_sub(CLIENT_QUOTA_WINDOW + Duration::from_secs(1))
            {
                entry.window_start = reset;
            }
            entry.used = 2;
        }

        let refreshed = issuer
            .issue_token(
                "client-a",
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
                requests_per_minute: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                requests_per_minute: Some(3),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                max_streams: Some(0),
                ..TokenOverrides::default()
            },
            TokenOverrides {
                ttl_secs: Some(901),
                ..TokenOverrides::default()
            },
        ] {
            assert!(matches!(
                issuer.issue_token(
                    "client-free",
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
                "client-free",
                vec![0xBB],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            )
            .expect("invalid requests must not consume issuance quota");
        assert_eq!(valid.remaining_quota, 1);
    }

    #[test]
    fn issuance_state_capacity_fails_closed_and_prunes_idle_clients() {
        let issuer = issuer_with_capacity(2, 2);
        for client in ["client-a", "client-b"] {
            issuer
                .issue_token(
                    client,
                    vec![0xBB],
                    [0x22; 32],
                    "sorafs.sf1@1.0.0".to_string(),
                    TokenOverrides::default(),
                )
                .expect("client admitted");
        }
        assert!(matches!(
            issuer.issue_token(
                "client-c",
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            ),
            Err(StreamTokenIssuerError::ClientQuotaCapacityExceeded { capacity: 2 })
        ));

        let stale = Instant::now()
            .checked_sub(CLIENT_QUOTA_WINDOW + Duration::from_secs(1))
            .expect("stale instant");
        issuer
            .client_budgets
            .lock()
            .expect("client budgets")
            .get_mut("client-a")
            .expect("client-a")
            .window_start = stale;
        issuer
            .issue_token(
                "client-c",
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            )
            .expect("stale client pruned before capacity check");
    }

    #[test]
    fn concurrent_issuance_never_exceeds_client_budget() {
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
                    "client-race",
                    vec![0xBB],
                    [0x22; 32],
                    "sorafs.sf1@1.0.0".to_string(),
                    TokenOverrides::default(),
                ) {
                    Ok(_) => {
                        successes.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(StreamTokenIssuerError::ClientQuotaExceeded { .. }) => {}
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
            let _guard = poisoner.client_budgets.lock().expect("issuance lock");
            panic!("poison issuance state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        assert!(matches!(
            issuer.issue_token(
                "client-a",
                vec![0xBB],
                [0x22; 32],
                "sorafs.sf1@1.0.0".to_string(),
                TokenOverrides::default(),
            ),
            Err(StreamTokenIssuerError::ClientQuotaStateUnavailable)
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

    #[cfg(unix)]
    #[test]
    fn signing_key_loader_rejects_symlinks_hardlinks_and_permissive_modes() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target.sk");
        fs::write(&target, [0x11; 32]).expect("write target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("chmod target");
        let link = dir.path().join("link.sk");
        symlink(&target, &link).expect("create symlink");
        assert!(matches!(
            load_signing_key(&link),
            Err(StreamTokenIssuerError::SigningKeyNotRegular { .. })
        ));

        let hardlink = dir.path().join("hardlink.sk");
        fs::hard_link(&target, &hardlink).expect("create hardlink");
        assert!(matches!(
            load_signing_key(&target),
            Err(StreamTokenIssuerError::SigningKeyLinkCount { links: 2, .. })
        ));
        fs::remove_file(hardlink).expect("remove hardlink");

        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).expect("chmod target");
        assert!(matches!(
            load_signing_key(&target),
            Err(StreamTokenIssuerError::SigningKeyPermissions { .. })
        ));
    }

    #[test]
    fn signing_key_loader_accepts_canonical_hex_newline_and_rejects_oversize() {
        let key_file = NamedTempFile::new().expect("create key file");
        fs::write(key_file.path(), format!("{}\n", "11".repeat(32))).expect("write hex key");
        let key = load_signing_key(key_file.path()).expect("canonical hex key");
        assert_eq!(key.to_bytes(), [0x11; 32]);

        fs::write(key_file.path(), [0x11; 66]).expect("write oversized key");
        assert!(matches!(
            load_signing_key(key_file.path()),
            Err(StreamTokenIssuerError::SigningKeyTooLarge { .. })
        ));
    }
}
