//! Admission token primitives for the `SoraNet` handshake.
//!
//! Tokens provide an optional alternative to memory-hard puzzles during relay
//! admission. Each token binds to a specific relay identity and handshake
//! transcript hash and is signed with an ML-DSA key managed by the relay or a
//! delegated issuer.

use std::{
    collections::HashMap,
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
use rand::{CryptoRng, RngCore};
use soranet_pq::{MlDsaError, MlDsaSuite, sign_mldsa, verify_mldsa};
use thiserror::Error;

const TOKEN_MAGIC: &[u8; 4] = b"SNTK";
const BODY_DOMAIN: &[u8] = b"soranet.token.body.v1";
const ID_DOMAIN: &[u8] = b"soranet.token.id.v1";
const ISSUER_DOMAIN: &[u8] = b"soranet.token.issuer.v1";

/// Length of the serialized token body (excluding magic, version, and signature).
const BODY_LEN: usize = 1 + 1 + 8 + 8 + 32 + 32 + 16 + 32;
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
        let flags = bytes[cursor];
        cursor += 1;
        if flags & !TOKEN_FLAG_MASK != 0 {
            return Err(DecodeError::InvalidFlags(flags));
        }

        let issued_at = u64::from_be_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;
        let expires_at = u64::from_be_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;

        let mut relay_id = [0u8; 32];
        relay_id.copy_from_slice(&bytes[cursor..cursor + 32]);
        cursor += 32;

        let mut transcript_hash = [0u8; 32];
        transcript_hash.copy_from_slice(&bytes[cursor..cursor + 32]);
        cursor += 32;

        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&bytes[cursor..cursor + 16]);
        cursor += 16;

        let mut issuer_fingerprint = [0u8; 32];
        issuer_fingerprint.copy_from_slice(&bytes[cursor..cursor + 32]);
        cursor += 32;

        if cursor + 2 > bytes.len() {
            return Err(DecodeError::Truncated {
                expected: cursor + 2,
                actual: bytes.len(),
            });
        }
        let sig_len = u16::from_be_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        if cursor + sig_len != bytes.len() {
            return Err(DecodeError::SignatureLength {
                expected: sig_len,
                actual: bytes.len() - cursor,
            });
        }
        if issued_at >= expires_at {
            return Err(DecodeError::InvalidTemporalBounds);
        }

        let signature = bytes[cursor..].to_vec();
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

    /// Serialize the token frame.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
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
        let sig_len = u16::try_from(self.signature.len()).expect("signature length fits in u16");
        out.extend_from_slice(&sig_len.to_be_bytes());
        out.extend_from_slice(&self.signature);
        out
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

    /// UNIX timestamp (seconds) when the token expires.
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.expires_at
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
        hasher.update(ID_DOMAIN);
        hasher.update(&self.body_bytes());
        hasher.update(self.signature());
        hasher.finalize().into()
    }

    /// Mint a new admission token using the provided issuer secret key.
    ///
    /// # Errors
    /// Returns [`MintError`] if the time bounds are invalid or signing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn mint<R: RngCore + CryptoRng>(
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

        let mut nonce = [0u8; 16];
        rng.fill_bytes(&mut nonce);
        let body = encode_body(
            flags,
            issued_secs,
            expires_secs,
            &relay_id,
            &transcript_hash,
            &nonce,
            &issuer_fingerprint,
        );
        let signature = sign_mldsa(suite, issuer_secret_key, &body)
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

    fn body_bytes(&self) -> Vec<u8> {
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
        let now_secs = now
            .duration_since(UNIX_EPOCH)
            .map_err(VerifyError::Clock)?
            .as_secs();

        if now_secs + self.clock_skew.as_secs() < token.issued_at {
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

        verify_mldsa(
            self.suite,
            &self.public_key,
            &token.body_bytes(),
            token.signature(),
        )
        .map_err(VerifyError::Signature)?;

        if let Some(store) = &self.replay_store {
            let token_id = token.token_id();
            let expires_at = UNIX_EPOCH + Duration::from_secs(token.expires_at());
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
        self.ingest_snapshot(snapshot, now);

        self.prune_expired(now);
        self.enforce_capacity();
        self.persist()
    }

    fn ingest_snapshot(&mut self, snapshot: TokenStoreSnapshot, now: SystemTime) {
        for entry in snapshot.entries {
            let expires_at = UNIX_EPOCH + Duration::from_secs(entry.expires_at_secs);
            if is_expired(expires_at, now) || exceeds_ttl(expires_at, now, self.limits.max_ttl) {
                continue;
            }
            let _ = self.records.insert(entry.id, TokenRecord { expires_at });
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
) -> Vec<u8> {
    let mut body = Vec::with_capacity(BODY_DOMAIN.len() + BODY_LEN);
    body.extend_from_slice(BODY_DOMAIN);
    body.push(flags);
    body.extend_from_slice(&issued_at.to_be_bytes());
    body.extend_from_slice(&expires_at.to_be_bytes());
    body.extend_from_slice(relay_id);
    body.extend_from_slice(transcript_hash);
    body.extend_from_slice(nonce);
    body.extend_from_slice(issuer_fingerprint);
    body
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
    use soranet_pq::generate_mldsa_keypair;
    use tempfile::tempdir;

    use super::*;

    const RELAY_ID: [u8; 32] = [0xAB; 32];
    const TRANSCRIPT: [u8; 32] = [0xCD; 32];

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
        let snapshot = TokenStoreSnapshot {
            entries: vec![
                TokenStoreEntry {
                    id: [0xAA; 32],
                    expires_at_secs: expired_secs,
                },
                TokenStoreEntry {
                    id: [0xBB; 32],
                    expires_at_secs: valid_secs,
                },
            ],
        };
        let content = encode_adaptive(&snapshot);
        std::fs::write(&path, content).expect("write snapshot");

        let store = PersistentTokenStore::load(&path, limits, now).expect("load");
        assert!(!store.contains(&[0xAA; 32], now));
        assert!(store.contains(&[0xBB; 32], now));
        assert_eq!(store.len(now), 1);
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
