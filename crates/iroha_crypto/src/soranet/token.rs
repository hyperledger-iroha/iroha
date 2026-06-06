//! Admission token primitives for the `SoraNet` handshake.
//!
//! Tokens provide an optional alternative to memory-hard puzzles during relay
//! admission. Each token binds to a specific relay identity and handshake
//! transcript hash and is signed with an ML-DSA key managed by the relay or a
//! delegated issuer.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use norito::{
    codec::{decode_adaptive, encode_adaptive},
    derive::{NoritoDeserialize, NoritoSerialize},
};
use rand_core::TryCryptoRng;
use soranet_pq::{MlDsaError, MlDsaSuite, sign_mldsa_from_os, verify_mldsa};
use thiserror::Error;

const TOKEN_MAGIC: &[u8; 4] = b"SNTK";
const BODY_DOMAIN: &[u8; 21] = b"soranet.token.body.v1";
const ID_DOMAIN: &[u8] = b"soranet.token.id.v1";
const ISSUER_DOMAIN: &[u8] = b"soranet.token.issuer.v1";

/// Length of the version-prefixed token body (excluding magic and signature).
const BODY_LEN: usize = 1 + 1 + 8 + 8 + 32 + 32 + 16 + 32;
/// Length of the domain-separated body signed by the issuer.
const SIGNING_BODY_LEN: usize = BODY_DOMAIN.len() + BODY_LEN - 1;
/// Minimum envelope length (magic + version + body + signature length prefix).
const MIN_FRAME_LEN: usize = TOKEN_MAGIC.len() + 1 + BODY_LEN + 2;
/// Flags defined for v1 tokens (all bits reserved).
const TOKEN_FLAG_MASK: u8 = 0;

/// Admission token issued by a relay operator or delegated gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionToken {
    /// Reserved flags (must be zero in v1).
    flags: u8,
    issued_at: u64,
    expires_at: u64,
    relay_id: [u8; 32],
    transcript_hash: [u8; 32],
    nonce: [u8; 16],
    issuer_fingerprint: [u8; 32],
    signature: Vec<u8>,
}

impl AdmissionToken {
    /// Current token format version.
    pub const VERSION: u8 = 1;

    /// Deserialize a token frame.
    ///
    /// # Errors
    /// Returns [`DecodeError`] when the payload fails structural validation.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < MIN_FRAME_LEN {
            return Err(DecodeError::Truncated {
                expected: MIN_FRAME_LEN,
                actual: bytes.len(),
            });
        }
        if &bytes[..TOKEN_MAGIC.len()] != TOKEN_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let version = bytes[TOKEN_MAGIC.len()];
        if version != Self::VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        let mut cursor = TOKEN_MAGIC.len() + 1;
        let flags = read_token_field::<1>(bytes, &mut cursor)?[0];
        if flags & !TOKEN_FLAG_MASK != 0 {
            return Err(DecodeError::InvalidFlags(flags));
        }

        let issued_at = u64::from_be_bytes(read_token_field::<8>(bytes, &mut cursor)?);
        let expires_at = u64::from_be_bytes(read_token_field::<8>(bytes, &mut cursor)?);
        let relay_id = read_token_field::<32>(bytes, &mut cursor)?;
        let transcript_hash = read_token_field::<32>(bytes, &mut cursor)?;
        let nonce = read_token_field::<16>(bytes, &mut cursor)?;
        let issuer_fingerprint = read_token_field::<32>(bytes, &mut cursor)?;
        let sig_len = u16::from_be_bytes(read_token_field::<2>(bytes, &mut cursor)?) as usize;
        let signature = read_token_signature(bytes, &mut cursor, sig_len)?;
        if issued_at >= expires_at {
            return Err(DecodeError::InvalidTemporalBounds);
        }
        if unix_time_from_secs(issued_at).is_none() {
            return Err(DecodeError::TimestampOutOfRange {
                field: "issued_at",
                value: issued_at,
            });
        }
        if unix_time_from_secs(expires_at).is_none() {
            return Err(DecodeError::TimestampOutOfRange {
                field: "expires_at",
                value: expires_at,
            });
        }

        Ok(Self {
            flags,
            issued_at,
            expires_at,
            relay_id,
            transcript_hash,
            nonce,
            issuer_fingerprint,
            signature,
        })
    }

    /// Try to serialize the token frame.
    ///
    /// # Errors
    /// Returns [`EncodeError`] when directly constructed token state cannot fit
    /// the v1 fixed-width frame.
    pub fn try_encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut out = Vec::with_capacity(MIN_FRAME_LEN + self.signature.len());
        out.extend_from_slice(TOKEN_MAGIC);
        out.push(Self::VERSION);
        out.push(self.flags);
        out.extend_from_slice(&self.issued_at.to_be_bytes());
        out.extend_from_slice(&self.expires_at.to_be_bytes());
        out.extend_from_slice(&self.relay_id);
        out.extend_from_slice(&self.transcript_hash);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.issuer_fingerprint);
        let sig_len =
            u16::try_from(self.signature.len()).map_err(|_| EncodeError::SignatureTooLong {
                max: usize::from(u16::MAX),
                actual: self.signature.len(),
            })?;
        out.extend_from_slice(&sig_len.to_be_bytes());
        out.extend_from_slice(&self.signature);
        Ok(out)
    }

    /// Serialize the token frame.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.try_encode().unwrap_or_else(|_| TOKEN_MAGIC.to_vec())
    }

    /// Flags embedded in the token body.
    /// Reserved for future use (must be zero in v1).
    #[must_use]
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// UNIX timestamp (seconds) when the token becomes valid.
    #[must_use]
    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    /// UNIX timestamp when the token becomes valid, if representable by
    /// [`SystemTime`].
    #[must_use]
    pub fn checked_issued_at(&self) -> Option<SystemTime> {
        unix_time_from_secs(self.issued_at)
    }

    /// UNIX timestamp (seconds) when the token expires.
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// UNIX timestamp when the token expires, if representable by
    /// [`SystemTime`].
    #[must_use]
    pub fn checked_expires_at(&self) -> Option<SystemTime> {
        unix_time_from_secs(self.expires_at)
    }

    /// Relay identifier bound into the token.
    #[must_use]
    pub fn relay_id(&self) -> &[u8; 32] {
        &self.relay_id
    }

    /// Transcript hash bound into the token.
    #[must_use]
    pub fn transcript_hash(&self) -> &[u8; 32] {
        &self.transcript_hash
    }

    /// Issuer fingerprint advertised in the token body.
    #[must_use]
    pub fn issuer_fingerprint(&self) -> &[u8; 32] {
        &self.issuer_fingerprint
    }

    /// Access the detached ML-DSA signature.
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// Compute a stable token identifier used for revocation lists.
    #[must_use]
    pub fn token_id(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        let body = self.body_bytes();
        hasher.update(ID_DOMAIN);
        hasher.update(&body);
        hasher.update(self.signature());
        hasher.finalize().into()
    }

    /// Mint a new admission token using the provided issuer secret key.
    ///
    /// # Errors
    /// Returns [`MintError`] if the time bounds are invalid, random bytes cannot be
    /// generated, or signing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn mint<R: TryCryptoRng>(
        suite: MlDsaSuite,
        issuer_secret_key: &[u8],
        issuer_fingerprint: [u8; 32],
        relay_id: [u8; 32],
        transcript_hash: [u8; 32],
        issued_at: SystemTime,
        expires_at: SystemTime,
        flags: u8,
        rng: &mut R,
    ) -> Result<Self, MintError> {
        let issued_secs = issued_at
            .duration_since(UNIX_EPOCH)
            .map_err(MintError::Clock)?
            .as_secs();
        let expires_secs = expires_at
            .duration_since(UNIX_EPOCH)
            .map_err(MintError::Clock)?
            .as_secs();
        if expires_secs <= issued_secs {
            return Err(MintError::InvalidTemporalBounds);
        }
        if flags & !TOKEN_FLAG_MASK != 0 {
            return Err(MintError::InvalidFlags(flags));
        }
        suite
            .validate_secret_key(issuer_secret_key)
            .map_err(MintError::Signature)?;

        let mut nonce = [0u8; 16];
        fill_random(rng, "minting admission token nonce", &mut nonce)?;
        let body = encode_body(
            flags,
            issued_secs,
            expires_secs,
            &relay_id,
            &transcript_hash,
            &nonce,
            &issuer_fingerprint,
        );
        let signature = sign_mldsa_from_os(suite, issuer_secret_key, &[], &body)
            .map_err(MintError::Signature)?
            .as_bytes()
            .to_vec();

        Ok(Self {
            flags,
            issued_at: issued_secs,
            expires_at: expires_secs,
            relay_id,
            transcript_hash,
            nonce,
            issuer_fingerprint,
            signature,
        })
    }

    fn body_bytes(&self) -> [u8; SIGNING_BODY_LEN] {
        encode_body(
            self.flags,
            self.issued_at,
            self.expires_at,
            &self.relay_id,
            &self.transcript_hash,
            &self.nonce,
            &self.issuer_fingerprint,
        )
    }
}

fn read_token_field<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], DecodeError> {
    let expected = cursor.checked_add(N).ok_or(DecodeError::Truncated {
        expected: usize::MAX,
        actual: bytes.len(),
    })?;
    if expected > bytes.len() {
        return Err(DecodeError::Truncated {
            expected,
            actual: bytes.len(),
        });
    }

    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[*cursor..expected]);
    *cursor = expected;
    Ok(out)
}

fn read_token_signature(
    bytes: &[u8],
    cursor: &mut usize,
    len: usize,
) -> Result<Vec<u8>, DecodeError> {
    let start = *cursor;
    let actual = bytes.len().saturating_sub(start);
    let end = start.checked_add(len).ok_or(DecodeError::SignatureLength {
        expected: len,
        actual,
    })?;
    if end != bytes.len() {
        return Err(DecodeError::SignatureLength {
            expected: len,
            actual,
        });
    }
    let signature = bytes.get(start..end).ok_or(DecodeError::SignatureLength {
        expected: len,
        actual,
    })?;
    *cursor = end;
    Ok(signature.to_vec())
}

/// Admission token verifier configured with an issuer key.
#[derive(Clone, Debug)]
pub struct AdmissionTokenVerifier {
    suite: MlDsaSuite,
    public_key: Vec<u8>,
    issuer_fingerprint: [u8; 32],
    max_ttl: Duration,
    clock_skew: Duration,
    replay_store: Option<Arc<Mutex<dyn TokenStore + Send>>>,
}

impl AdmissionTokenVerifier {
    /// Construct a new verifier.
    ///
    /// Runtime configuration loaders should prefer
    /// [`AdmissionTokenVerifier::try_new`] so invalid key material can fail at
    /// configuration load time. This compatibility constructor keeps malformed
    /// issuer keys as fail-closed verifier state; verification preflights reject
    /// them before backend signature checks or replay-store mutation.
    pub fn new(
        suite: MlDsaSuite,
        public_key: Vec<u8>,
        max_ttl: Duration,
        clock_skew: Duration,
    ) -> Self {
        let issuer_fingerprint = compute_issuer_fingerprint(&public_key);
        Self {
            suite,
            public_key,
            issuer_fingerprint,
            max_ttl,
            clock_skew,
            replay_store: None,
        }
    }

    /// Construct a new verifier.
    ///
    /// # Errors
    /// Returns [`VerifierConfigError`] if the configured issuer public key does
    /// not match the selected ML-DSA suite.
    pub fn try_new(
        suite: MlDsaSuite,
        public_key: Vec<u8>,
        max_ttl: Duration,
        clock_skew: Duration,
    ) -> Result<Self, VerifierConfigError> {
        suite
            .validate_public_key(&public_key)
            .map_err(VerifierConfigError::PublicKey)?;
        let issuer_fingerprint = compute_issuer_fingerprint(&public_key);
        Ok(Self {
            suite,
            public_key,
            issuer_fingerprint,
            max_ttl,
            clock_skew,
            replay_store: None,
        })
    }

    /// Attach a replay store used to enforce single-use semantics.
    #[must_use]
    pub fn with_replay_store(mut self, store: Arc<Mutex<dyn TokenStore + Send>>) -> Self {
        self.replay_store = Some(store);
        self
    }

    /// Set or replace the replay store in place.
    pub fn set_replay_store(&mut self, store: Arc<Mutex<dyn TokenStore + Send>>) {
        self.replay_store = Some(store);
    }

    /// Fingerprint associated with the issuer public key.
    #[must_use]
    pub fn issuer_fingerprint(&self) -> &[u8; 32] {
        &self.issuer_fingerprint
    }

    /// Verify a token against the provided relay identifier and transcript hash.
    ///
    /// # Errors
    /// Returns [`VerifyError`] if the token fails any validation step.
    pub fn verify(
        &self,
        token: &AdmissionToken,
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
        now: SystemTime,
    ) -> Result<(), VerifyError> {
        if token.issuer_fingerprint != self.issuer_fingerprint {
            return Err(VerifyError::IssuerMismatch(token.issuer_fingerprint));
        }
        if token.relay_id != *relay_id {
            return Err(VerifyError::RelayMismatch);
        }
        if token.transcript_hash != *transcript_hash {
            return Err(VerifyError::TranscriptMismatch);
        }
        if token.expires_at <= token.issued_at {
            return Err(VerifyError::InvalidTemporalBounds);
        }
        let now_secs = now
            .duration_since(UNIX_EPOCH)
            .map_err(VerifyError::Clock)?
            .as_secs();

        if now_secs.saturating_add(self.clock_skew.as_secs()) < token.issued_at {
            return Err(VerifyError::NotYetValid {
                issued_at: token.issued_at,
                now: now_secs,
            });
        }
        if now_secs.saturating_sub(self.clock_skew.as_secs()) >= token.expires_at {
            return Err(VerifyError::Expired {
                expires_at: token.expires_at,
                now: now_secs,
            });
        }

        let ttl_secs = token.expires_at.saturating_sub(token.issued_at);
        if ttl_secs > self.max_ttl.as_secs() {
            return Err(VerifyError::TtlExceeded {
                ttl: Duration::from_secs(ttl_secs),
                max: self.max_ttl,
            });
        }

        self.preflight_crypto_material(token)?;

        let body = token.body_bytes();
        verify_mldsa(self.suite, &self.public_key, &[], &body, token.signature())
            .map_err(VerifyError::Signature)?;

        if let Some(store) = &self.replay_store {
            let token_id = token.token_id();
            let expires_at = UNIX_EPOCH
                .checked_add(Duration::from_secs(token.expires_at()))
                .ok_or_else(|| {
                    VerifyError::Store(TokenStoreError::Parse(format!(
                        "token expiry timestamp {} overflows system time",
                        token.expires_at()
                    )))
                })?;
            let mut guard = store
                .lock()
                .map_err(|_| VerifyError::Store(TokenStoreError::Poisoned))?;
            let outcome = guard
                .insert(token_id, expires_at, now)
                .map_err(VerifyError::Store)?;
            match outcome.status {
                TokenInsertStatus::Accepted => {}
                TokenInsertStatus::Duplicate => return Err(VerifyError::Replay(token_id)),
                TokenInsertStatus::Expired
                | TokenInsertStatus::TtlExceeded
                | TokenInsertStatus::Capacity => {
                    return Err(VerifyError::Store(TokenStoreError::InsertFailed {
                        status: outcome.status,
                    }));
                }
            }
        }

        Ok(())
    }

    fn preflight_crypto_material(&self, token: &AdmissionToken) -> Result<(), VerifyError> {
        self.suite
            .validate_public_key(&self.public_key)
            .map_err(VerifyError::Signature)?;
        self.suite
            .validate_signature(token.signature())
            .map_err(VerifyError::Signature)
    }
}

/// Errors surfaced while constructing an admission-token verifier.
#[derive(Debug, Error)]
pub enum VerifierConfigError {
    /// The configured issuer public key does not match the selected suite.
    #[error("admission token issuer public key is invalid: {0}")]
    PublicKey(MlDsaError),
}

/// Compute the canonical issuer fingerprint from a public key.
#[must_use]
pub fn compute_issuer_fingerprint(public_key: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ISSUER_DOMAIN);
    hasher.update(public_key);
    hasher.finalize().into()
}

/// Policy for admission token stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenStoreLimits {
    /// Maximum number of entries to keep; oldest entries are evicted when capacity is exceeded.
    pub max_entries: usize,
    /// Maximum allowed time-to-live for a token relative to insertion.
    pub max_ttl: Duration,
}

impl TokenStoreLimits {
    /// Create new limits, rejecting zero capacity so callers avoid silent disablement.
    ///
    /// # Errors
    /// Returns [`TokenStoreError::CapacityZero`] when `max_entries` is zero or
    /// [`TokenStoreError::TtlZero`] when `max_ttl` is zero.
    pub fn new(max_entries: usize, max_ttl: Duration) -> Result<Self, TokenStoreError> {
        if max_entries == 0 {
            return Err(TokenStoreError::CapacityZero);
        }
        if max_ttl.is_zero() {
            return Err(TokenStoreError::TtlZero);
        }
        Ok(Self {
            max_entries,
            max_ttl,
        })
    }
}

/// Outcome when inserting a token into the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenInsertStatus {
    /// Token was inserted successfully.
    Accepted,
    /// Token already existed in the store.
    Duplicate,
    /// Token expired before insertion.
    Expired,
    /// Token TTL exceeded the configured maximum.
    TtlExceeded,
    /// Token could not be inserted because the store was at capacity and eviction was not possible.
    Capacity,
}

/// Result of an insertion attempt, including any evicted record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenInsertOutcome {
    /// Final status for the insertion attempt.
    pub status: TokenInsertStatus,
    /// Optional token id evicted to make room for the new entry.
    pub evicted: Option<[u8; 32]>,
}

impl TokenInsertOutcome {
    fn accepted(evicted: Option<[u8; 32]>) -> Self {
        Self {
            status: TokenInsertStatus::Accepted,
            evicted,
        }
    }

    fn rejected(status: TokenInsertStatus) -> Self {
        Self {
            status,
            evicted: None,
        }
    }
}

/// Errors emitted by token store implementations.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum TokenStoreError {
    /// Store was configured with zero capacity.
    #[error("token store capacity must be greater than zero")]
    CapacityZero,
    /// Store was configured with a zero TTL.
    #[error("token store max_ttl must be greater than zero")]
    TtlZero,
    /// Store rejected an insertion attempt.
    #[error("token store insertion failed: {status:?}")]
    InsertFailed {
        /// Final status reported by the store.
        status: TokenInsertStatus,
    },
    /// Mutex guarding the store was poisoned.
    #[error("token store is poisoned")]
    Poisoned,
    /// Store encountered an IO error.
    #[error("token store io error: {0}")]
    Io(String),
    /// Store encountered malformed persisted data.
    #[error("token store parse error: {0}")]
    Parse(String),
}

/// Admission token store interface used to enforce replay/TTL policies.
pub trait TokenStore: std::fmt::Debug + Send {
    /// Insert a token by id with its expiry, returning the outcome of the operation.
    ///
    /// # Errors
    /// Returns [`TokenStoreError`] if the store cannot record the token (for example,
    /// due to persistence errors).
    fn insert(
        &mut self,
        token_id: [u8; 32],
        expires_at: SystemTime,
        now: SystemTime,
    ) -> Result<TokenInsertOutcome, TokenStoreError>;

    /// Check if the store currently contains a non-expired token id.
    fn contains(&self, token_id: &[u8; 32], now: SystemTime) -> bool;

    /// Number of non-expired entries tracked by the store.
    fn len(&self, now: SystemTime) -> usize;

    /// Purge expired entries and return the number removed.
    ///
    /// # Errors
    /// Returns [`TokenStoreError`] if the store cannot persist updates after pruning.
    fn purge_expired(&mut self, now: SystemTime) -> Result<usize, TokenStoreError>;
}

#[derive(Debug, Clone)]
struct TokenRecord {
    expires_at: SystemTime,
}

#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct TokenStoreEntry {
    id: [u8; 32],
    expires_at_secs: u64,
}

#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct TokenStoreSnapshot {
    entries: Vec<TokenStoreEntry>,
}

/// In-memory implementation of a bounded admission token store.
///
/// Tokens that are expired at the time of insertion are rejected. When the store reaches capacity,
/// the oldest entry (by `expires_at`) is evicted to make room for a new token. If eviction is not
/// possible (e.g., capacity is zero), the insert is rejected with `Capacity`.
#[derive(Debug)]
pub struct InMemoryTokenStore {
    limits: TokenStoreLimits,
    records: HashMap<[u8; 32], TokenRecord>,
}

impl InMemoryTokenStore {
    /// Create a new store with the provided limits.
    pub fn new(limits: TokenStoreLimits) -> Self {
        Self {
            limits,
            records: HashMap::new(),
        }
    }

    fn prune_expired(&mut self, now: SystemTime) {
        self.records
            .retain(|_, record| !is_expired(record.expires_at, now));
    }

    fn evict_oldest(&mut self) -> Option<[u8; 32]> {
        let oldest = self
            .records
            .iter()
            .min_by_key(|(_, record)| record.expires_at)
            .map(|(id, _)| *id);
        if let Some(id) = oldest {
            self.records.remove(&id);
            return Some(id);
        }
        None
    }
}

impl TokenStore for InMemoryTokenStore {
    fn insert(
        &mut self,
        token_id: [u8; 32],
        expires_at: SystemTime,
        now: SystemTime,
    ) -> Result<TokenInsertOutcome, TokenStoreError> {
        self.prune_expired(now);
        if is_expired(expires_at, now) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Expired));
        }
        if exceeds_ttl(expires_at, now, self.limits.max_ttl) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::TtlExceeded));
        }
        if self.records.contains_key(&token_id) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Duplicate));
        }
        let mut evicted = None;
        if self.records.len() >= self.limits.max_entries {
            evicted = self.evict_oldest();
            if evicted.is_none() && self.records.len() >= self.limits.max_entries {
                return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Capacity));
            }
        }
        self.records.insert(token_id, TokenRecord { expires_at });
        Ok(TokenInsertOutcome::accepted(evicted))
    }

    fn contains(&self, token_id: &[u8; 32], now: SystemTime) -> bool {
        self.records
            .get(token_id)
            .is_some_and(|record| !is_expired(record.expires_at, now))
    }

    fn len(&self, now: SystemTime) -> usize {
        self.records
            .values()
            .filter(|record| !is_expired(record.expires_at, now))
            .count()
    }

    fn purge_expired(&mut self, now: SystemTime) -> Result<usize, TokenStoreError> {
        let before = self.records.len();
        self.prune_expired(now);
        Ok(before.saturating_sub(self.records.len()))
    }
}

/// Persistent admission token store backed by a newline-delimited file.
///
/// Each entry is encoded as `<hex_token_id> <expires_at_unix>` on its own line. The store prunes
/// expired and over-TTL entries on load and before every insert. Inserts evict the oldest record
/// (earliest expiry) when capacity is exceeded.
#[derive(Debug)]
pub struct PersistentTokenStore {
    limits: TokenStoreLimits,
    records: HashMap<[u8; 32], TokenRecord>,
    path: PathBuf,
}

impl PersistentTokenStore {
    /// Load or create a persistent token store at `path`.
    ///
    /// # Errors
    /// Returns [`TokenStoreError`] if the snapshot cannot be read or parsed or if the
    /// backing directory cannot be created.
    pub fn load(
        path: impl Into<PathBuf>,
        limits: TokenStoreLimits,
        now: SystemTime,
    ) -> Result<Self, TokenStoreError> {
        let mut store = Self {
            limits,
            records: HashMap::new(),
            path: path.into(),
        };
        if let Some(parent) = store.path.parent() {
            fs::create_dir_all(parent).map_err(|err| TokenStoreError::Io(err.to_string()))?;
        }
        store.load_from_disk(now)?;
        Ok(store)
    }

    fn load_from_disk(&mut self, now: SystemTime) -> Result<(), TokenStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(TokenStoreError::Io(err.to_string())),
        };
        if bytes.is_empty() {
            return Ok(());
        }

        let snapshot = decode_adaptive::<TokenStoreSnapshot>(&bytes).map_err(|decode_err| {
            TokenStoreError::Parse(format!("norito decode failed: {decode_err}"))
        })?;
        self.ingest_snapshot(snapshot, now)?;

        self.prune_expired(now);
        self.enforce_capacity();
        self.persist()
    }

    fn ingest_snapshot(
        &mut self,
        snapshot: TokenStoreSnapshot,
        now: SystemTime,
    ) -> Result<(), TokenStoreError> {
        let mut seen_ids = HashSet::with_capacity(snapshot.entries.len());
        for entry in snapshot.entries {
            if !seen_ids.insert(entry.id) {
                return Err(TokenStoreError::Parse(
                    "duplicate token id in snapshot".to_string(),
                ));
            }
            let expires_at = UNIX_EPOCH
                .checked_add(Duration::from_secs(entry.expires_at_secs))
                .ok_or_else(|| {
                    TokenStoreError::Parse(format!(
                        "token expiry timestamp {} overflows system time",
                        entry.expires_at_secs
                    ))
                })?;
            if is_expired(expires_at, now) || exceeds_ttl(expires_at, now, self.limits.max_ttl) {
                continue;
            }
            let _ = self.records.insert(entry.id, TokenRecord { expires_at });
        }
        Ok(())
    }

    fn prune_expired(&mut self, now: SystemTime) {
        self.records
            .retain(|_, record| !is_expired(record.expires_at, now));
    }

    fn evict_oldest(&mut self) -> Option<[u8; 32]> {
        let oldest = self
            .records
            .iter()
            .min_by_key(|(_, record)| record.expires_at)
            .map(|(id, _)| *id);
        if let Some(id) = oldest {
            self.records.remove(&id);
            return Some(id);
        }
        None
    }

    fn enforce_capacity(&mut self) {
        while self.records.len() > self.limits.max_entries {
            if self.evict_oldest().is_none() {
                break;
            }
        }
    }

    fn persist(&self) -> Result<(), TokenStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| TokenStoreError::Io(err.to_string()))?;
        }
        let mut entries: Vec<_> = self.records.iter().collect();
        entries.sort_by_key(|(_, record)| record.expires_at);
        let snapshot = TokenStoreSnapshot {
            entries: entries
                .into_iter()
                .filter_map(|(id, record)| {
                    let expires_secs = record.expires_at.duration_since(UNIX_EPOCH).ok()?;
                    Some(TokenStoreEntry {
                        id: *id,
                        expires_at_secs: expires_secs.as_secs(),
                    })
                })
                .collect(),
        };
        let buf = encode_adaptive(&snapshot);
        let tmp_path = self.path.with_extension("tmp");
        fs::write(&tmp_path, buf).map_err(|err| TokenStoreError::Io(err.to_string()))?;
        fs::rename(&tmp_path, &self.path).map_err(|err| TokenStoreError::Io(err.to_string()))
    }
}

impl TokenStore for PersistentTokenStore {
    fn insert(
        &mut self,
        token_id: [u8; 32],
        expires_at: SystemTime,
        now: SystemTime,
    ) -> Result<TokenInsertOutcome, TokenStoreError> {
        self.prune_expired(now);
        if is_expired(expires_at, now) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Expired));
        }
        if exceeds_ttl(expires_at, now, self.limits.max_ttl) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::TtlExceeded));
        }
        if self.records.contains_key(&token_id) {
            return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Duplicate));
        }
        let mut evicted = None;
        if self.records.len() >= self.limits.max_entries {
            evicted = self.evict_oldest();
            if evicted.is_none() && self.records.len() >= self.limits.max_entries {
                return Ok(TokenInsertOutcome::rejected(TokenInsertStatus::Capacity));
            }
        }
        self.records.insert(token_id, TokenRecord { expires_at });
        self.persist()?;
        Ok(TokenInsertOutcome::accepted(evicted))
    }

    fn contains(&self, token_id: &[u8; 32], now: SystemTime) -> bool {
        self.records
            .get(token_id)
            .is_some_and(|record| !is_expired(record.expires_at, now))
    }

    fn len(&self, now: SystemTime) -> usize {
        self.records
            .values()
            .filter(|record| !is_expired(record.expires_at, now))
            .count()
    }

    fn purge_expired(&mut self, now: SystemTime) -> Result<usize, TokenStoreError> {
        let before = self.records.len();
        self.prune_expired(now);
        let removed = before.saturating_sub(self.records.len());
        if removed > 0 {
            self.persist()?;
        }
        Ok(removed)
    }
}

fn is_expired(expires_at: SystemTime, now: SystemTime) -> bool {
    expires_at <= now
}

fn exceeds_ttl(expires_at: SystemTime, now: SystemTime, max_ttl: Duration) -> bool {
    expires_at
        .duration_since(now)
        .map_or(true, |delta| delta > max_ttl)
}

fn encode_body(
    flags: u8,
    issued_at: u64,
    expires_at: u64,
    relay_id: &[u8; 32],
    transcript_hash: &[u8; 32],
    nonce: &[u8; 16],
    issuer_fingerprint: &[u8; 32],
) -> [u8; SIGNING_BODY_LEN] {
    let mut body = [0u8; SIGNING_BODY_LEN];
    let mut cursor = 0;

    body[cursor..cursor + BODY_DOMAIN.len()].copy_from_slice(BODY_DOMAIN);
    cursor += BODY_DOMAIN.len();
    body[cursor] = flags;
    cursor += 1;
    body[cursor..cursor + 8].copy_from_slice(&issued_at.to_be_bytes());
    cursor += 8;
    body[cursor..cursor + 8].copy_from_slice(&expires_at.to_be_bytes());
    cursor += 8;
    body[cursor..cursor + relay_id.len()].copy_from_slice(relay_id);
    cursor += relay_id.len();
    body[cursor..cursor + transcript_hash.len()].copy_from_slice(transcript_hash);
    cursor += transcript_hash.len();
    body[cursor..cursor + nonce.len()].copy_from_slice(nonce);
    cursor += nonce.len();
    body[cursor..cursor + issuer_fingerprint.len()].copy_from_slice(issuer_fingerprint);
    cursor += issuer_fingerprint.len();
    debug_assert_eq!(cursor, SIGNING_BODY_LEN);

    body
}

fn unix_time_from_secs(secs: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
}

fn fill_random<R: TryCryptoRng>(
    rng: &mut R,
    operation: &'static str,
    dest: &mut [u8],
) -> Result<(), MintError> {
    rng.try_fill_bytes(dest)
        .map_err(|err| MintError::RandomBytes {
            operation,
            message: err.to_string(),
        })
}

/// Errors surfaced while decoding a token frame.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
pub enum DecodeError {
    /// Token magic prefix did not match `SNTK`.
    #[error("token magic mismatch")]
    BadMagic,
    /// Unsupported token version.
    #[error("unsupported token version {0}")]
    UnsupportedVersion(u8),
    /// Frame was shorter than the minimum length.
    #[error("token truncated (expected at least {expected} bytes, got {actual})")]
    Truncated {
        /// Expected minimum frame length in bytes.
        expected: usize,
        /// Actual frame length observed during decoding.
        actual: usize,
    },
    /// Signature length prefix did not match the remaining payload.
    #[error("signature length mismatch (expected {expected} bytes, got {actual})")]
    SignatureLength {
        /// Declared signature length in bytes.
        expected: usize,
        /// Remaining payload length following the prefix.
        actual: usize,
    },
    /// Flags contained undefined bits.
    #[error("token flags contain unknown bits ({0:#04x})")]
    InvalidFlags(u8),
    /// `issued_at` was not earlier than `expires_at`.
    #[error("token issued_at must be earlier than expires_at")]
    InvalidTemporalBounds,
    /// Timestamp could not be represented as `SystemTime`.
    #[error("{field} timestamp {value} is out of range for system time")]
    TimestampOutOfRange {
        /// Timestamp field name.
        field: &'static str,
        /// UNIX-second timestamp carried by the frame.
        value: u64,
    },
}

/// Errors surfaced while serializing token frames.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
pub enum EncodeError {
    /// Signature bytes exceeded the v1 length prefix range.
    #[error("signature too long to encode: max {max} bytes, got {actual}")]
    SignatureTooLong {
        /// Maximum signature size encodable by the v1 frame.
        max: usize,
        /// Actual signature size observed.
        actual: usize,
    },
}

/// Errors raised while minting a token.
#[derive(Debug, Error)]
pub enum MintError {
    /// System clock not available.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
    /// `expires_at` was not greater than `issued_at`.
    #[error("token expires_at must be greater than issued_at")]
    InvalidTemporalBounds,
    /// Flags contained undefined bits.
    #[error("token flags contain unknown bits ({0:#04x})")]
    InvalidFlags(u8),
    /// Random byte generation failed while minting the token.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
    /// ML-DSA signing failure.
    #[error("ml-dsa signing failed: {0}")]
    Signature(MlDsaError),
}

/// Errors raised while verifying a token.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// Token issuer fingerprint did not match the configured public key.
    #[error("token issuer fingerprint mismatch")]
    IssuerMismatch([u8; 32]),
    /// Relay identifier embedded in the token does not match the local relay.
    #[error("token relay id mismatch")]
    RelayMismatch,
    /// Token transcript hash did not match the handshake transcript.
    #[error("token transcript hash mismatch")]
    TranscriptMismatch,
    /// Token is not yet valid.
    #[error("token not yet valid (issued_at={issued_at}, now={now})")]
    NotYetValid {
        /// Token issuance timestamp (UTC seconds).
        issued_at: u64,
        /// Current timestamp when verification was attempted.
        now: u64,
    },
    /// Token expired.
    #[error("token expired (expires_at={expires_at}, now={now})")]
    Expired {
        /// Token expiration timestamp (UTC seconds).
        expires_at: u64,
        /// Current timestamp when verification was attempted.
        now: u64,
    },
    /// Token validity window exceeds the configured maximum.
    #[error("token ttl {ttl:?} exceeds configured maximum {max:?}")]
    TtlExceeded {
        /// Token time-to-live derived from the encoded bounds.
        ttl: Duration,
        /// Maximum allowed validity window.
        max: Duration,
    },
    /// Token validity bounds were inverted or zero-length.
    #[error("token expires_at must be greater than issued_at")]
    InvalidTemporalBounds,
    /// Clock error while obtaining the current time.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
    /// Signature verification failed.
    #[error("ml-dsa verification failed: {0}")]
    Signature(MlDsaError),
    /// Replay store failure.
    #[error("token replay store error: {0}")]
    Store(TokenStoreError),
    /// Token was already consumed.
    #[error("token replay detected")]
    Replay([u8; 32]),
}

/// Check whether a frame begins with the token magic prefix.
#[must_use]
pub fn frame_looks_like_token(frame: &[u8]) -> bool {
    frame.len() > TOKEN_MAGIC.len() && &frame[..TOKEN_MAGIC.len()] == TOKEN_MAGIC
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};
    use rand_core::{TryCryptoRng, TryRngCore};
    use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;
    use tempfile::tempdir;

    use super::*;

    const RELAY_ID: [u8; 32] = [0xAB; 32];
    const TRANSCRIPT: [u8; 32] = [0xCD; 32];

    fn write_token_store_snapshot(path: &std::path::Path, entries: Vec<TokenStoreEntry>) {
        let snapshot = TokenStoreSnapshot { entries };
        let content = encode_adaptive(&snapshot);
        std::fs::write(path, content).expect("write token store snapshot");
    }

    fn assert_mldsa_bad_encoding(err: VerifyError, field: &str) {
        match err {
            VerifyError::Signature(MlDsaError::BadEncoding(err)) => {
                assert!(err.to_string().contains(field));
            }
            other => panic!("expected ML-DSA bad encoding error, got {other:?}"),
        }
    }

    fn assert_mint_mldsa_bad_encoding(err: MintError, field: &str) {
        match err {
            MintError::Signature(MlDsaError::BadEncoding(err)) => {
                assert!(err.to_string().contains(field));
            }
            other => panic!("expected ML-DSA bad encoding error, got {other:?}"),
        }
    }

    struct FailingTryRng;

    #[derive(Debug)]
    struct FailingTryRngError;

    impl std::fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing admission token RNG")
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

    #[test]
    fn signing_body_matches_legacy_contiguous_layout() {
        let token = AdmissionToken {
            flags: 0,
            issued_at: 1_700_000_000,
            expires_at: 1_700_000_600,
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            nonce: [0xAB; 16],
            issuer_fingerprint: [0xEF; 32],
            signature: vec![0x55; 128],
        };

        let mut legacy = Vec::with_capacity(BODY_DOMAIN.len() + BODY_LEN);
        legacy.extend_from_slice(BODY_DOMAIN);
        legacy.push(token.flags);
        legacy.extend_from_slice(&token.issued_at.to_be_bytes());
        legacy.extend_from_slice(&token.expires_at.to_be_bytes());
        legacy.extend_from_slice(&token.relay_id);
        legacy.extend_from_slice(&token.transcript_hash);
        legacy.extend_from_slice(&token.nonce);
        legacy.extend_from_slice(&token.issuer_fingerprint);

        assert_eq!(legacy.len(), SIGNING_BODY_LEN);
        assert_eq!(token.body_bytes().as_slice(), legacy.as_slice());

        let mut legacy_id = Hasher::new();
        legacy_id.update(ID_DOMAIN);
        legacy_id.update(&legacy);
        legacy_id.update(token.signature());
        let expected_id: [u8; 32] = legacy_id.finalize().into();
        assert_eq!(token.token_id(), expected_id);
    }

    #[test]
    fn encode_decode_round_trip() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(0xDEAD_BEEF);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");

        let encoded = token.encode();
        let decoded = AdmissionToken::decode(&encoded).expect("decode");
        assert_eq!(token.token_id(), decoded.token_id());
        assert_eq!(token.relay_id, decoded.relay_id);
        assert_eq!(token.transcript_hash, decoded.transcript_hash);
    }

    #[test]
    fn decode_truncated_token_prefixes_fail_closed() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let mut rng = StdRng::seed_from_u64(0xFACE_FEED);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            UNIX_EPOCH + Duration::from_secs(1_700_000_600),
            0,
            &mut rng,
        )
        .expect("mint");
        let encoded = token.encode();

        for len in 0..encoded.len() {
            assert!(
                AdmissionToken::decode(&encoded[..len]).is_err(),
                "truncated prefix of length {len} must fail closed"
            );
        }
    }

    #[test]
    fn decode_rejects_unrepresentable_timestamps() {
        let issued_overflow = AdmissionToken {
            flags: 0,
            issued_at: u64::MAX - 1,
            expires_at: u64::MAX,
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            nonce: [0xAA; 16],
            issuer_fingerprint: [0xBB; 32],
            signature: vec![0xCC],
        };
        assert!(issued_overflow.checked_issued_at().is_none());
        let err = AdmissionToken::decode(&issued_overflow.encode())
            .expect_err("unrepresentable issued_at should fail closed");
        assert!(matches!(
            err,
            DecodeError::TimestampOutOfRange {
                field: "issued_at",
                value
            } if value == u64::MAX - 1
        ));

        let expires_overflow = AdmissionToken {
            flags: 0,
            issued_at: 10,
            expires_at: u64::MAX,
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            nonce: [0xAA; 16],
            issuer_fingerprint: [0xBB; 32],
            signature: vec![0xCC],
        };
        assert!(expires_overflow.checked_expires_at().is_none());
        let err = AdmissionToken::decode(&expires_overflow.encode())
            .expect_err("unrepresentable expires_at should fail closed");
        assert!(matches!(
            err,
            DecodeError::TimestampOutOfRange {
                field: "expires_at",
                value
            } if value == u64::MAX
        ));
    }

    #[test]
    fn token_signature_reader_rejects_mismatch_and_overflow_without_advancing() {
        let mut valid_cursor = 1;
        let signature =
            read_token_signature(&[0xAA, 0xBB, 0xCC], &mut valid_cursor, 2).expect("signature");
        assert_eq!(signature, vec![0xBB, 0xCC]);
        assert_eq!(valid_cursor, 3);

        let mut extra_cursor = 1;
        let err = read_token_signature(&[0xAA, 0xBB, 0xCC], &mut extra_cursor, 1)
            .expect_err("extra tail bytes must fail closed");
        assert!(matches!(
            err,
            DecodeError::SignatureLength {
                expected: 1,
                actual: 2
            }
        ));
        assert_eq!(extra_cursor, 1);

        let mut truncated_cursor = 1;
        let err = read_token_signature(&[0xAA, 0xBB], &mut truncated_cursor, 2)
            .expect_err("truncated tail bytes must fail closed");
        assert!(matches!(
            err,
            DecodeError::SignatureLength {
                expected: 2,
                actual: 1
            }
        ));
        assert_eq!(truncated_cursor, 1);

        let mut overflowed_cursor = usize::MAX;
        let err = read_token_signature(&[], &mut overflowed_cursor, 1)
            .expect_err("overflowed signature cursor must fail closed");
        assert!(matches!(
            err,
            DecodeError::SignatureLength {
                expected: 1,
                actual: 0
            }
        ));
        assert_eq!(overflowed_cursor, usize::MAX);
    }

    #[test]
    fn try_encode_rejects_oversized_direct_signature_without_panic() {
        let token = AdmissionToken {
            flags: 0,
            issued_at: 10,
            expires_at: 20,
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            nonce: [0xAA; 16],
            issuer_fingerprint: [0xBB; 32],
            signature: vec![0xCC; usize::from(u16::MAX) + 1],
        };

        let err = token
            .try_encode()
            .expect_err("oversized direct signature should not encode");
        assert!(matches!(
            err,
            EncodeError::SignatureTooLong {
                max,
                actual
            } if max == usize::from(u16::MAX) && actual == usize::from(u16::MAX) + 1
        ));
        assert!(matches!(
            AdmissionToken::decode(&token.encode()),
            Err(DecodeError::Truncated { .. })
        ));
    }

    #[test]
    fn verify_accepts_valid_token() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(42);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        );
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_100);
        verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect("verify");
    }

    #[test]
    fn verifier_try_new_rejects_invalid_public_key_before_fingerprint() {
        let err = AdmissionTokenVerifier::try_new(
            MlDsaSuite::MlDsa44,
            Vec::new(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .expect_err("invalid issuer public key must fail at verifier construction");

        match err {
            VerifierConfigError::PublicKey(MlDsaError::BadEncoding(err)) => {
                assert!(err.to_string().contains("public key"));
            }
            other => panic!("expected ML-DSA public-key config error, got {other:?}"),
        }
    }

    #[test]
    fn admission_token_reuse_is_currently_allowed() {
        // Without a replay store attached, tokens remain reusable.
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(7);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(1),
        );

        verifier
            .verify(
                &token,
                &RELAY_ID,
                &TRANSCRIPT,
                issued + Duration::from_secs(1),
            )
            .expect("first use");
        verifier
            .verify(
                &token,
                &RELAY_ID,
                &TRANSCRIPT,
                issued + Duration::from_secs(2),
            )
            .expect("replay use");
    }

    #[test]
    fn verify_rejects_relay_mismatch() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(7);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        );
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_100);
        let result = verifier.verify(&token, &[0xEF; 32], &TRANSCRIPT, now);
        assert!(matches!(result, Err(VerifyError::RelayMismatch)));
    }

    #[test]
    fn verify_rejects_invalid_temporal_bounds_before_signature_preflight() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(0x0BAD_5EED);
        let mut token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        token.expires_at = token.issued_at;
        token.signature.clear();

        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        );
        let err = verifier
            .verify(
                &token,
                &RELAY_ID,
                &TRANSCRIPT,
                issued + Duration::from_secs(1),
            )
            .expect_err("invalid temporal bounds must fail before signature checks");
        assert!(matches!(err, VerifyError::InvalidTemporalBounds));
    }

    #[test]
    fn frame_detection() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let mut rng = StdRng::seed_from_u64(99);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            UNIX_EPOCH + Duration::from_secs(1_700_000_600),
            0,
            &mut rng,
        )
        .expect("mint");
        let encoded = token.encode();
        assert!(frame_looks_like_token(&encoded));
    }

    #[test]
    fn decode_rejects_non_zero_flags() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let mut rng = StdRng::seed_from_u64(123);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            UNIX_EPOCH + Duration::from_secs(1_700_000_600),
            0,
            &mut rng,
        )
        .expect("mint");
        let mut encoded = token.encode();
        encoded[TOKEN_MAGIC.len() + 1] = 0x01;
        let err = AdmissionToken::decode(&encoded).expect_err("flags must be zero");
        assert!(matches!(err, DecodeError::InvalidFlags(0x01)));
    }

    #[test]
    fn mint_rejects_non_zero_flags() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let mut rng = StdRng::seed_from_u64(321);
        let err = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            UNIX_EPOCH + Duration::from_secs(1_700_000_600),
            0x01,
            &mut rng,
        )
        .expect_err("mint should reject non-zero flags");
        assert!(matches!(err, MintError::InvalidFlags(0x01)));
    }

    #[test]
    fn mint_rejects_invalid_secret_key_length_before_backend() {
        let suite = MlDsaSuite::MlDsa44;
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = StdRng::seed_from_u64(0x5EC);

        let err = AdmissionToken::mint(
            suite,
            &[],
            [0xEF; 32],
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect_err("invalid secret key length must fail before signing");
        assert_mint_mldsa_bad_encoding(err, "secret key");
    }

    #[test]
    fn mint_reports_rng_failure() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair(suite).expect("ML-DSA keypair generation");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = UNIX_EPOCH + Duration::from_secs(1_700_000_600);
        let mut rng = FailingTryRng;

        let err = AdmissionToken::mint(
            suite,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect_err("mint should surface RNG failure");

        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting admission token nonce");
                assert!(message.contains("failing admission token RNG"));
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_zero_ttl() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let mut rng = StdRng::seed_from_u64(456);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            UNIX_EPOCH + Duration::from_secs(1_700_000_600),
            0,
            &mut rng,
        )
        .expect("mint");
        let mut encoded = token.encode();
        let issued_range = TOKEN_MAGIC.len() + 2..TOKEN_MAGIC.len() + 10;
        let expires_range = TOKEN_MAGIC.len() + 10..TOKEN_MAGIC.len() + 18;
        let issued = encoded[issued_range.clone()].to_vec();
        encoded[expires_range].copy_from_slice(&issued);
        let err = AdmissionToken::decode(&encoded).expect_err("zero ttl must be rejected");
        assert!(matches!(err, DecodeError::InvalidTemporalBounds));
    }

    #[test]
    fn token_store_rejects_expired_and_ttl_overflow() {
        let limits = TokenStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let mut store = InMemoryTokenStore::new(limits);
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let expired = now - Duration::from_secs(1);
        let too_far = now + Duration::from_secs(301);

        let expired_outcome = store
            .insert([0xAA; 32], expired, now)
            .expect("expired insert");
        assert_eq!(expired_outcome.status, TokenInsertStatus::Expired);
        let ttl_outcome = store.insert([0xBB; 32], too_far, now).expect("ttl insert");
        assert_eq!(ttl_outcome.status, TokenInsertStatus::TtlExceeded);
        assert_eq!(store.len(now), 0);
    }

    #[test]
    fn token_store_evicts_oldest_when_full() {
        let limits = TokenStoreLimits::new(2, Duration::from_secs(120)).expect("limits");
        let mut store = InMemoryTokenStore::new(limits);
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let a_exp = now + Duration::from_secs(60);
        let b_exp = now + Duration::from_secs(90);
        let c_exp = now + Duration::from_secs(30);

        let insert_a = store.insert([0x01; 32], a_exp, now).expect("insert a");
        assert_eq!(insert_a.status, TokenInsertStatus::Accepted);
        assert!(insert_a.evicted.is_none());

        let insert_b = store.insert([0x02; 32], b_exp, now).expect("insert b");
        assert_eq!(insert_b.status, TokenInsertStatus::Accepted);
        assert!(insert_b.evicted.is_none());

        // Third insert should evict the oldest (token 0x01)
        let insert_c = store.insert([0x03; 32], c_exp, now).expect("insert c");
        assert_eq!(insert_c.status, TokenInsertStatus::Accepted);
        assert_eq!(insert_c.evicted, Some([0x01; 32]));
        assert!(!store.contains(&[0x01; 32], now));
        assert!(store.contains(&[0x02; 32], now));
        assert!(store.contains(&[0x03; 32], now));
        assert_eq!(store.len(now), 2);
    }

    #[test]
    fn verifier_rejects_replay_with_store() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(13);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");

        let limits = TokenStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store = Arc::new(Mutex::new(InMemoryTokenStore::new(limits)));
        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .with_replay_store(store);

        let now = issued + Duration::from_secs(5);
        verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect("first use");
        let err = verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect_err("replay must be blocked");
        assert!(matches!(err, VerifyError::Replay(_)));
    }

    #[test]
    fn invalid_signatures_do_not_poison_replay_store() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(77);
        let mut token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        token.signature[0] ^= 0xFF;

        let limits = TokenStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store: Arc<Mutex<dyn TokenStore + Send>> =
            Arc::new(Mutex::new(InMemoryTokenStore::new(limits)));
        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .with_replay_store(store.clone());

        let now = issued + Duration::from_secs(5);
        let err = verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect_err("invalid signature should be rejected");
        assert!(matches!(err, VerifyError::Signature(_)));
        assert_eq!(store.lock().expect("store lock").len(now), 0);
    }

    #[test]
    fn verifier_rejects_invalid_public_key_length_before_backend() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair(suite).expect("ML-DSA keypair generation");
        let mut bad_public_key = keypair.public_key().to_vec();
        bad_public_key.pop();

        let err = AdmissionTokenVerifier::try_new(
            suite,
            bad_public_key,
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .expect_err("bad verifier public key length must fail during construction");

        match err {
            VerifierConfigError::PublicKey(MlDsaError::BadEncoding(err)) => {
                assert!(err.to_string().contains("public key"));
            }
            other => panic!("expected ML-DSA public-key config error, got {other:?}"),
        }
    }

    #[test]
    fn verifier_new_with_invalid_public_key_fails_closed_during_verify() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair(suite).expect("ML-DSA keypair generation");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(0x0BAD_5EED);
        let mut token = AdmissionToken::mint(
            suite,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");

        let mut bad_public_key = keypair.public_key().to_vec();
        bad_public_key.pop();
        let limits = TokenStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store: Arc<Mutex<dyn TokenStore + Send>> =
            Arc::new(Mutex::new(InMemoryTokenStore::new(limits)));
        let verifier = AdmissionTokenVerifier::new(
            suite,
            bad_public_key,
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .with_replay_store(store.clone());
        token.issuer_fingerprint = *verifier.issuer_fingerprint();

        let now = issued + Duration::from_secs(5);
        let err = verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect_err("malformed verifier public key must fail closed");
        assert_mldsa_bad_encoding(err, "public key");
        assert_eq!(store.lock().expect("store lock").len(now), 0);
    }

    #[test]
    fn verifier_rejects_signature_length_before_replay_store() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair(suite).expect("ML-DSA keypair generation");
        let fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let issued = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(0x51A);
        let mut token = AdmissionToken::mint(
            suite,
            keypair.secret_key(),
            fingerprint,
            RELAY_ID,
            TRANSCRIPT,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint");
        token.signature.truncate(token.signature.len() - 1);

        let limits = TokenStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store: Arc<Mutex<dyn TokenStore + Send>> =
            Arc::new(Mutex::new(InMemoryTokenStore::new(limits)));
        let verifier = AdmissionTokenVerifier::new(
            suite,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        )
        .with_replay_store(store.clone());
        let now = issued + Duration::from_secs(5);

        let err = verifier
            .verify(&token, &RELAY_ID, &TRANSCRIPT, now)
            .expect_err("bad signature length must fail");
        assert_mldsa_bad_encoding(err, "signature");
        assert_eq!(store.lock().expect("store lock").len(now), 0);
    }

    #[test]
    fn persistent_store_blocks_replay_after_restart() {
        let limits = TokenStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("replay_store.txt");
        {
            let mut store = PersistentTokenStore::load(&path, limits, now).expect("load");
            let expires = now + Duration::from_secs(60);
            let outcome = store.insert([0xAA; 32], expires, now).expect("insert");
            assert_eq!(outcome.status, TokenInsertStatus::Accepted);
            assert!(store.contains(&[0xAA; 32], now));
        }
        let mut store = PersistentTokenStore::load(&path, limits, now).expect("reload");
        assert!(store.contains(&[0xAA; 32], now));
        let duplicate = store
            .insert([0xAA; 32], now + Duration::from_secs(30), now)
            .expect("duplicate insert");
        assert_eq!(duplicate.status, TokenInsertStatus::Duplicate);
    }

    #[test]
    fn persistent_store_eviction_persists() {
        let limits = TokenStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(50_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("persist_store.txt");

        let mut store = PersistentTokenStore::load(&path, limits, now).expect("load");
        let _ = store
            .insert([0x01; 32], now + Duration::from_secs(10), now)
            .expect("insert a");
        let _ = store
            .insert([0x02; 32], now + Duration::from_secs(20), now)
            .expect("insert b");
        let eviction = store
            .insert([0x03; 32], now + Duration::from_secs(30), now)
            .expect("insert c");
        assert_eq!(eviction.evicted, Some([0x01; 32]));

        let store = PersistentTokenStore::load(&path, limits, now).expect("reload");
        assert!(!store.contains(&[0x01; 32], now));
        assert!(store.contains(&[0x02; 32], now));
        assert!(store.contains(&[0x03; 32], now));
        assert_eq!(store.len(now), 2);
    }

    #[test]
    fn persistent_store_prunes_expired_on_load() {
        let limits = TokenStoreLimits::new(2, Duration::from_secs(120)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("prune_store.txt");
        let expired_secs = (now - Duration::from_secs(10))
            .duration_since(UNIX_EPOCH)
            .expect("expired >= epoch")
            .as_secs();
        let valid_secs = (now + Duration::from_secs(10))
            .duration_since(UNIX_EPOCH)
            .expect("valid >= epoch")
            .as_secs();
        write_token_store_snapshot(
            &path,
            vec![
                TokenStoreEntry {
                    id: [0xAA; 32],
                    expires_at_secs: expired_secs,
                },
                TokenStoreEntry {
                    id: [0xBB; 32],
                    expires_at_secs: valid_secs,
                },
            ],
        );

        let store = PersistentTokenStore::load(&path, limits, now).expect("load");
        assert!(!store.contains(&[0xAA; 32], now));
        assert!(store.contains(&[0xBB; 32], now));
        assert_eq!(store.len(now), 1);
    }

    #[test]
    fn persistent_store_rejects_duplicate_token_ids_on_load() {
        let limits = TokenStoreLimits::new(4, Duration::from_secs(120)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("duplicate_store.txt");
        write_token_store_snapshot(
            &path,
            vec![
                TokenStoreEntry {
                    id: [0xAA; 32],
                    expires_at_secs: 10_030,
                },
                TokenStoreEntry {
                    id: [0xAA; 32],
                    expires_at_secs: 10_060,
                },
            ],
        );

        let err =
            PersistentTokenStore::load(&path, limits, now).expect_err("duplicate id should fail");
        match err {
            TokenStoreError::Parse(message) => {
                assert!(message.contains("duplicate"));
                assert!(message.contains("token id"));
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn persistent_store_rejects_overflowing_expiry_on_load() {
        let limits = TokenStoreLimits::new(4, Duration::from_secs(120)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("overflow_store.txt");
        write_token_store_snapshot(
            &path,
            vec![TokenStoreEntry {
                id: [0xBB; 32],
                expires_at_secs: u64::MAX,
            }],
        );

        let err = PersistentTokenStore::load(&path, limits, now).expect_err("overflow should fail");
        match err {
            TokenStoreError::Parse(message) => {
                assert!(message.contains("expiry"));
                assert!(message.contains("overflows"));
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn persistent_store_rejects_non_norito_snapshot() {
        let limits = TokenStoreLimits::new(2, Duration::from_secs(120)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("invalid_store.txt");

        std::fs::write(&path, b"not norito").expect("write invalid");
        let err = PersistentTokenStore::load(&path, limits, now)
            .expect_err("invalid snapshot should fail");
        assert!(matches!(err, TokenStoreError::Parse(_)));
    }
}
