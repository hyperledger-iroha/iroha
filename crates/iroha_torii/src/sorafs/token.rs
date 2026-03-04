//! Stream token issuance helpers for Torii chunk-range gateways.

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use dashmap::{DashMap, mapref::entry::Entry};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use iroha_config::parameters::actual;
use rand::random;
use sorafs_manifest::{StreamTokenBodyV1, StreamTokenError, StreamTokenV1};
use thiserror::Error;

/// Fixed rolling window applied to per-client issuance quotas.
const CLIENT_QUOTA_WINDOW: Duration = Duration::from_mins(1);

/// Issuer used to sign stream tokens with configured defaults.
pub struct StreamTokenIssuer {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    defaults: TokenDefaults,
    client_budgets: DashMap<String, ClientBudget>,
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
    /// Maximum issuances permitted within the window.
    limit: u32,
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
    /// Optional override for the per-client issuance quota (requests per minute).
    pub requests_per_minute: Option<u32>,
}

/// Result of a successful token issuance.
#[derive(Debug)]
pub struct TokenIssue {
    /// Signed stream token.
    pub token: StreamTokenV1,
    /// Remaining issuance quota within the current window when the quota is finite.
    pub remaining_quota: Option<u32>,
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
        let defaults = TokenDefaults {
            key_version: config.key_version,
            ttl_secs: config.default_ttl_secs,
            max_streams: config.default_max_streams,
            rate_limit_bytes: config.default_rate_limit_bytes,
            requests_per_minute: config.default_requests_per_minute,
        };

        Ok(Some(Self {
            signing_key,
            verifying_key,
            defaults,
            client_budgets: DashMap::new(),
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StreamTokenIssuerError::TimeOverflow)?
            .as_secs();

        let window_limit = overrides
            .requests_per_minute
            .unwrap_or(self.defaults.requests_per_minute);

        let remaining_quota = if window_limit == 0 {
            // Unlimited quota — drop any existing budget state.
            self.client_budgets.remove(client_id);
            None
        } else {
            let now_instant = Instant::now();
            let remaining = match self.client_budgets.entry(client_id.to_owned()) {
                Entry::Occupied(mut entry) => {
                    let budget = entry.get_mut();
                    let elapsed = now_instant.duration_since(budget.window_start);
                    if elapsed >= CLIENT_QUOTA_WINDOW || budget.limit != window_limit {
                        budget.window_start = now_instant;
                        budget.limit = window_limit;
                        budget.used = 0;
                    }
                    if budget.used >= budget.limit {
                        let retry_after_secs = CLIENT_QUOTA_WINDOW
                            .saturating_sub(elapsed.min(CLIENT_QUOTA_WINDOW))
                            .as_secs()
                            .max(1);
                        return Err(StreamTokenIssuerError::ClientQuotaExceeded {
                            client_id: client_id.to_owned(),
                            limit: budget.limit,
                            retry_after_secs,
                        });
                    }
                    budget.used += 1;
                    budget.limit.saturating_sub(budget.used)
                }
                Entry::Vacant(entry) => {
                    entry.insert(ClientBudget {
                        window_start: now_instant,
                        limit: window_limit,
                        used: 1,
                    });
                    window_limit.saturating_sub(1)
                }
            };
            Some(remaining)
        };

        let ttl_secs = overrides.ttl_secs.unwrap_or(self.defaults.ttl_secs);
        let ttl_epoch = now
            .checked_add(ttl_secs)
            .ok_or(StreamTokenIssuerError::TimeOverflow)?;
        let max_streams = overrides.max_streams.unwrap_or(self.defaults.max_streams);
        let rate_limit_bytes = overrides
            .rate_limit_bytes
            .unwrap_or(self.defaults.rate_limit_bytes);
        let requests_per_minute = window_limit;

        let body = StreamTokenBodyV1 {
            token_id: new_token_id(),
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

        let token = StreamTokenV1::sign(body, &self.signing_key)
            .map_err(StreamTokenIssuerError::StreamToken)?;

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
}

fn new_token_id() -> String {
    let bytes: [u8; 16] = random();
    hex::encode(bytes)
}

fn load_signing_key(path: &PathBuf) -> Result<SigningKey, StreamTokenIssuerError> {
    let raw = fs::read(path).map_err(|err| StreamTokenIssuerError::SigningKeyIo {
        path: path.clone(),
        source: err,
    })?;

    let trimmed = String::from_utf8_lossy(&raw).trim().to_owned();
    let key_bytes = if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(trimmed).map_err(|err| StreamTokenIssuerError::SigningKeyDecode {
            path: path.clone(),
            source: err,
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

    let mut array = [0u8; 32];
    array.copy_from_slice(&key_bytes);
    Ok(SigningKey::from_bytes(&array))
}

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
    /// System clock produced a timestamp prior to the Unix epoch.
    #[error("system time before UNIX epoch")]
    TimeOverflow,
    /// Serialising or signing the stream token body failed.
    #[error("failed to create stream token: {0}")]
    StreamToken(#[from] StreamTokenError),
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
}

/// Errors produced while decoding stream tokens from client headers.
#[derive(Debug, Error)]
pub enum StreamTokenHeaderError {
    /// Header value was not valid base64.
    #[error("stream token header must be base64-encoded")]
    InvalidEncoding,
    /// The decoded token payload failed Norito deserialisation.
    #[error("invalid stream token payload: {0}")]
    InvalidPayload(norito::Error),
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
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| StreamTokenHeaderError::InvalidEncoding)?;
    norito::decode_from_bytes::<StreamTokenV1>(&bytes)
        .map_err(StreamTokenHeaderError::InvalidPayload)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn sample_body() -> StreamTokenBodyV1 {
        StreamTokenBodyV1 {
            token_id: "01J3E4ZCMQ3GP2H3R5PSNF6Z7X".to_string(),
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
    fn verify_rejects_modified_body() {
        let signing = SigningKey::from_bytes(&[0x24; 32]);
        let verifying = signing.verifying_key();
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
        let mut tampered = token.clone();
        tampered.body.max_streams = 8;
        let err = tampered.verify(&verifying).expect_err("should fail");
        matches!(err, StreamTokenError::SignatureInvalid(_));
    }

    fn issuer_with_limit(limit: u32) -> StreamTokenIssuer {
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
            client_budgets: DashMap::new(),
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
        assert_eq!(first.remaining_quota, Some(1));

        let second = issuer
            .issue_token(
                "client-a",
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect("second token");
        assert_eq!(second.remaining_quota, Some(0));

        let err = issuer
            .issue_token(
                "client-a",
                vec![0xAA],
                provider,
                "sorafs.sf1@1.0.0".to_string(),
                overrides.clone(),
            )
            .expect_err("quota exceeded");
        matches!(err, StreamTokenIssuerError::ClientQuotaExceeded { .. });

        if let Some(mut entry) = issuer.client_budgets.get_mut("client-a") {
            if let Some(reset) =
                Instant::now().checked_sub(CLIENT_QUOTA_WINDOW + Duration::from_secs(1))
            {
                entry.window_start = reset;
            }
            entry.used = entry.limit;
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
        assert_eq!(refreshed.remaining_quota, Some(1));
    }

    #[test]
    fn unlimited_quota_skips_tracking() {
        let issuer = issuer_with_limit(0);
        let provider = [0x22; 32];
        let overrides = TokenOverrides {
            requests_per_minute: Some(0),
            ..TokenOverrides::default()
        };

        for _ in 0..5 {
            let issue = issuer
                .issue_token(
                    "client-free",
                    vec![0xBB],
                    provider,
                    "sorafs.sf1@1.0.0".to_string(),
                    overrides.clone(),
                )
                .expect("token issuance");
            assert!(issue.remaining_quota.is_none());
        }

        assert!(issuer.client_budgets.is_empty());
    }
}
