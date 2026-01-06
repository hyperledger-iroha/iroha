//! `PoW` ticket helpers for the `SoraNet` admission protocol.

use std::{
    collections::HashMap,
    fmt, fs, io,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::hash;
use norito::{
    codec::{decode_adaptive, encode_adaptive},
    derive::{NoritoDeserialize, NoritoSerialize},
};
use rand::{CryptoRng, RngCore};
use soranet_pq::{MlDsaError, MlDsaSuite, sign_mldsa, verify_mldsa};
use thiserror::Error;

/// Domain separator used when deriving `PoW` challenges.
pub const CHALLENGE_DOMAIN: &[u8] = b"soranet.pow.challenge.v1";
/// Domain separator used when hashing `PoW` solutions.
pub const SOLUTION_DOMAIN: &[u8] = b"soranet.pow.solution.v1";
/// Domain separator used when signing `SignedTicket` payloads.
pub const SIGNING_DOMAIN: &[u8] = b"soranet.pow.signed_ticket.v1";
/// Domain separator used when hashing revocation fingerprints.
pub const REVOCATION_DOMAIN: &[u8] = b"soranet.pow.revocation.v1";

/// Length of the serialized `PoW` ticket payload.
pub const TICKET_LEN: usize = 74;
/// Slack tolerated when validating the remaining TTL to account for second-level truncation.
const TTL_GRACE: Duration = Duration::from_secs(1);

/// Hashcash-style ticket attached to `SoraNet` circuit establishment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct Ticket {
    /// Ticket format version (currently `1`).
    pub version: u8,
    /// Number of leading zero bits required in the solution.
    pub difficulty: u8,
    /// UNIX timestamp (seconds) when the ticket expires.
    pub expires_at: u64,
    /// Client-provided nonce mixed into the challenge hash.
    pub client_nonce: [u8; 32],
    /// Solution nonce satisfying the difficulty predicate.
    pub solution: [u8; 32],
}

impl Ticket {
    /// Current ticket format version.
    pub const VERSION: u8 = 1;

    /// Serialize the ticket to a fixed-length byte array.
    #[must_use]
    pub fn to_bytes(self) -> [u8; TICKET_LEN] {
        let mut out = [0u8; TICKET_LEN];
        out[0] = self.version;
        out[1] = self.difficulty;
        out[2..10].copy_from_slice(&self.expires_at.to_be_bytes());
        out[10..42].copy_from_slice(&self.client_nonce);
        out[42..74].copy_from_slice(&self.solution);
        out
    }

    /// Serialize the ticket to a `Vec<u8>`.
    #[must_use]
    pub fn to_vec(self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    /// Parse a ticket from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if the payload length is not `TICKET_LEN`.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != TICKET_LEN {
            return Err(Error::Malformed(format!(
                "expected {TICKET_LEN} bytes, got {}",
                bytes.len()
            )));
        }
        let version = bytes[0];
        if version != Self::VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let difficulty = bytes[1];
        let mut expires_at_bytes = [0u8; 8];
        expires_at_bytes.copy_from_slice(&bytes[2..10]);
        let expires_at = u64::from_be_bytes(expires_at_bytes);
        let mut client_nonce = [0u8; 32];
        client_nonce.copy_from_slice(&bytes[10..42]);
        let mut solution = [0u8; 32];
        solution.copy_from_slice(&bytes[42..74]);

        Ok(Self {
            version,
            difficulty,
            expires_at,
            client_nonce,
            solution,
        })
    }

    /// Returns the ticket expiration timestamp as a `SystemTime`.
    #[must_use]
    pub fn expires_at_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.expires_at)
    }

    /// Compute the revocation fingerprint for the ticket payload.
    #[must_use]
    pub fn revocation_fingerprint(&self) -> [u8; 32] {
        compute_revocation_fingerprint(&self.to_bytes())
    }
}

/// A `PoW` ticket signed by a relay using ML-DSA-44 (Dilithium2).
///
/// Signed tickets act as reusable tokens or "fast passes" for clients, binding
/// the proof-of-work (or a difficulty-0 grant) to a specific relay and session
/// context. The signature covers the ticket bytes, the relay ID, and an optional
/// transcript hash to prevent replay across different sessions or relays.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct SignedTicket {
    /// The underlying `PoW` ticket.
    pub ticket: Ticket,
    /// The relay identifier (32 bytes) that signed this ticket.
    pub relay_id: [u8; 32],
    /// Optional transcript hash binding the ticket to a specific session.
    pub transcript_hash: Option<[u8; 32]>,
    /// ML-DSA-44 signature over `(ticket || relay_id || transcript_hash)`.
    pub signature: Vec<u8>,
}

impl SignedTicket {
    /// Create a new `SignedTicket` by signing the provided `ticket` and bindings.
    ///
    /// # Errors
    /// Returns [`Error::Signing`] if the ML-DSA operation fails.
    pub fn sign(
        ticket: Ticket,
        relay_id: &[u8; 32],
        transcript_hash: Option<&[u8; 32]>,
        secret_key: &[u8],
    ) -> Result<Self, Error> {
        let payload = Self::build_payload(&ticket, relay_id, transcript_hash);
        let signature = sign_mldsa(MlDsaSuite::MlDsa44, secret_key, &payload)
            .map_err(|e| Error::Signing(e.to_string()))?;

        Ok(Self {
            ticket,
            relay_id: *relay_id,
            transcript_hash: transcript_hash.copied(),
            signature: signature.as_bytes().to_vec(),
        })
    }

    /// Decode a signed ticket from a Norito payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when the payload fails to parse.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let decoded: Self = decode_adaptive(bytes)
            .map_err(|err| Error::Malformed(format!("signed ticket decode failed: {err}")))?;
        if decoded.ticket.version != Ticket::VERSION {
            return Err(Error::UnsupportedVersion(decoded.ticket.version));
        }
        Ok(decoded)
    }

    /// Encode the signed ticket using the adaptive Norito codec.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_adaptive(self)
    }

    /// Verify the signature on this ticket against the provided public key.
    ///
    /// # Errors
    /// Returns [`Error::InvalidSignature`] if verification fails or [`Error::PostQuantum`]
    /// if the key/signature format is invalid.
    pub fn verify(&self, public_key: &[u8]) -> Result<(), Error> {
        let payload =
            Self::build_payload(&self.ticket, &self.relay_id, self.transcript_hash.as_ref());
        verify_mldsa(MlDsaSuite::MlDsa44, public_key, &payload, &self.signature).map_err(
            |e| match e {
                MlDsaError::VerificationFailed(_) => Error::InvalidSignature,
                other => Error::PostQuantum(other.to_string()),
            },
        )
    }

    fn build_payload(
        ticket: &Ticket,
        relay_id: &[u8; 32],
        transcript_hash: Option<&[u8; 32]>,
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(SIGNING_DOMAIN.len() + TICKET_LEN + 32 + 32);
        payload.extend_from_slice(SIGNING_DOMAIN);
        payload.extend_from_slice(&ticket.to_bytes());
        payload.extend_from_slice(relay_id);
        if let Some(hash) = transcript_hash {
            payload.extend_from_slice(hash);
        }
        payload
    }

    /// Returns the ticket expiration timestamp as a `SystemTime`.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.ticket.expires_at)
    }

    /// Compute the revocation fingerprint for the embedded signature.
    #[must_use]
    pub fn revocation_fingerprint(&self) -> [u8; 32] {
        compute_revocation_fingerprint(&self.signature)
    }
}

/// Limits applied to the revocation store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TicketRevocationStoreLimits {
    /// Maximum number of fingerprints to retain.
    pub max_entries: usize,
    /// Maximum TTL allowed for a revoked ticket relative to insertion time.
    pub max_ttl: Duration,
}

impl TicketRevocationStoreLimits {
    /// Create limits, rejecting zero capacity or zero TTL.
    ///
    /// # Errors
    ///
    /// Returns [`TicketRevocationStoreError`] when `max_entries` is zero or `max_ttl` is zero.
    pub fn new(max_entries: usize, max_ttl: Duration) -> Result<Self, TicketRevocationStoreError> {
        if max_entries == 0 {
            return Err(TicketRevocationStoreError::CapacityZero);
        }
        if max_ttl.is_zero() {
            return Err(TicketRevocationStoreError::TtlZero);
        }
        Ok(Self {
            max_entries,
            max_ttl,
        })
    }
}

/// Status returned when inserting a revoked fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketRevocationInsertStatus {
    /// Fingerprint inserted successfully.
    Accepted,
    /// Fingerprint already existed.
    Duplicate,
    /// Ticket expired before insertion.
    Expired,
    /// Ticket TTL exceeded configured maximum.
    TtlExceeded,
    /// Store could not make room for a new entry.
    Capacity,
}

/// Outcome of an insertion attempt, including any evicted entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TicketRevocationInsertOutcome {
    /// Final status.
    pub status: TicketRevocationInsertStatus,
    /// Optional fingerprint evicted to make space.
    pub evicted: Option<[u8; 32]>,
}

impl TicketRevocationInsertOutcome {
    const fn accepted(evicted: Option<[u8; 32]>) -> Self {
        Self {
            status: TicketRevocationInsertStatus::Accepted,
            evicted,
        }
    }

    const fn rejected(status: TicketRevocationInsertStatus) -> Self {
        Self {
            status,
            evicted: None,
        }
    }
}

/// Errors surfaced by the revocation store.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TicketRevocationStoreError {
    /// Store capacity cannot be zero.
    #[error("revocation store capacity must be greater than zero")]
    CapacityZero,
    /// TTL bound must be non-zero.
    #[error("revocation store max_ttl must be greater than zero")]
    TtlZero,
    /// Filesystem error while reading or writing the store.
    #[error("revocation store io error: {0}")]
    Io(String),
    /// Persisted snapshot failed to parse.
    #[error("revocation store parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Copy)]
struct RevokedTicketRecord {
    expires_at: SystemTime,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct TicketRevocationSnapshot {
    entries: Vec<TicketRevocationSnapshotEntry>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct TicketRevocationSnapshotEntry {
    fingerprint: [u8; 32],
    expires_at_secs: u64,
}

/// Persistent store for revoked ticket signatures.
#[derive(Debug)]
pub struct TicketRevocationStore {
    limits: TicketRevocationStoreLimits,
    records: HashMap<[u8; 32], RevokedTicketRecord>,
    path: Option<PathBuf>,
}

impl TicketRevocationStore {
    /// Create an in-memory store with the provided limits.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] if the supplied bounds are zero or otherwise invalid.
    pub fn in_memory(
        limits: TicketRevocationStoreLimits,
    ) -> Result<Self, TicketRevocationStoreError> {
        Ok(Self {
            limits: TicketRevocationStoreLimits::new(limits.max_entries, limits.max_ttl)?,
            records: HashMap::new(),
            path: None,
        })
    }

    /// Load or create a persistent store at `path`.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] if the limits are invalid or the on-disk
    /// snapshot cannot be read or parsed.
    pub fn load(
        path: impl Into<PathBuf>,
        limits: TicketRevocationStoreLimits,
        now: SystemTime,
    ) -> Result<Self, TicketRevocationStoreError> {
        let mut store = Self {
            limits: TicketRevocationStoreLimits::new(limits.max_entries, limits.max_ttl)?,
            records: HashMap::new(),
            path: Some(path.into()),
        };
        store.load_from_disk(now)?;
        Ok(store)
    }

    /// Insert a `SignedTicket` into the store using its signature and expiry.
    ///
    /// # Errors
    /// Propagates [`TicketRevocationStoreError`] when persistence fails while
    /// recording the revocation.
    pub fn revoke_ticket(
        &mut self,
        ticket: &SignedTicket,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        self.revoke_signature(&ticket.signature, ticket.expires_at(), now)
    }

    /// Insert a raw signature and expiry into the store.
    ///
    /// # Errors
    /// Propagates [`TicketRevocationStoreError`] when persistence fails while
    /// recording the revocation.
    pub fn revoke_signature(
        &mut self,
        signature: &[u8],
        expires_at: SystemTime,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        let fingerprint = compute_revocation_fingerprint(signature);
        self.insert(fingerprint, expires_at, now)
    }

    /// Insert a ticket by hashing its serialized payload.
    ///
    /// # Errors
    /// Propagates [`TicketRevocationStoreError`] when persistence fails while
    /// recording the revocation.
    pub fn revoke_ticket_bytes(
        &mut self,
        ticket: &Ticket,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        self.insert(
            ticket.revocation_fingerprint(),
            ticket.expires_at_time(),
            now,
        )
    }

    /// Insert a raw ticket payload into the store using its expiry.
    ///
    /// # Errors
    /// Propagates [`TicketRevocationStoreError`] when persistence fails while
    /// recording the revocation.
    pub fn revoke_ticket_payload(
        &mut self,
        ticket: &Ticket,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        let fingerprint = ticket.revocation_fingerprint();
        let expires_at = ticket.expires_at_time();
        self.insert(fingerprint, expires_at, now)
    }

    /// Check if a signature has been revoked and is still within its TTL.
    #[must_use]
    pub fn is_revoked_signature(&self, signature: &[u8], now: SystemTime) -> bool {
        let fingerprint = compute_revocation_fingerprint(signature);
        self.is_revoked_fingerprint(&fingerprint, now)
    }

    /// Check if a ticket payload has been revoked and is still within its TTL.
    #[must_use]
    pub fn is_ticket_payload_revoked(&self, ticket: &Ticket, now: SystemTime) -> bool {
        let fingerprint = ticket.revocation_fingerprint();
        self.is_revoked_fingerprint(&fingerprint, now)
    }

    /// Check if a ticket has been revoked.
    #[must_use]
    pub fn is_ticket_revoked(&self, ticket: &SignedTicket, now: SystemTime) -> bool {
        self.is_revoked_signature(&ticket.signature, now)
    }

    /// Number of active (non-expired) fingerprints.
    #[must_use]
    pub fn len(&self, now: SystemTime) -> usize {
        self.records
            .values()
            .filter(|record| !is_expired(record.expires_at, now))
            .count()
    }

    /// Return the active fingerprints retained by the store.
    #[must_use]
    pub fn active_fingerprints(&self, now: SystemTime) -> Vec<[u8; 32]> {
        self.records
            .iter()
            .filter_map(|(fingerprint, record)| {
                if is_expired(record.expires_at, now) {
                    None
                } else {
                    Some(*fingerprint)
                }
            })
            .collect()
    }

    /// Remove expired entries and persist updates.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] when the updated snapshot cannot be written.
    pub fn purge_expired(&mut self, now: SystemTime) -> Result<usize, TicketRevocationStoreError> {
        let before = self.records.len();
        self.records
            .retain(|_, record| !is_expired(record.expires_at, now));
        let removed = before.saturating_sub(self.records.len());
        if removed > 0 {
            self.persist()?;
        }
        Ok(removed)
    }

    fn insert(
        &mut self,
        fingerprint: [u8; 32],
        expires_at: SystemTime,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        self.records
            .retain(|_, record| !is_expired(record.expires_at, now));
        if is_expired(expires_at, now) {
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::Expired,
            ));
        }
        if exceeds_ttl(expires_at, now, self.limits.max_ttl) {
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::TtlExceeded,
            ));
        }
        if self.records.contains_key(&fingerprint) {
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::Duplicate,
            ));
        }

        let mut evicted = None;
        if self.records.len() >= self.limits.max_entries {
            evicted = self.evict_oldest();
            if evicted.is_none() && self.records.len() >= self.limits.max_entries {
                return Ok(TicketRevocationInsertOutcome::rejected(
                    TicketRevocationInsertStatus::Capacity,
                ));
            }
        }

        self.records
            .insert(fingerprint, RevokedTicketRecord { expires_at });
        self.persist()?;
        Ok(TicketRevocationInsertOutcome::accepted(evicted))
    }

    fn is_revoked_fingerprint(&self, fingerprint: &[u8; 32], now: SystemTime) -> bool {
        self.records
            .get(fingerprint)
            .is_some_and(|record| !is_expired(record.expires_at, now))
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

    fn load_from_disk(&mut self, now: SystemTime) -> Result<(), TicketRevocationStoreError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        }
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(TicketRevocationStoreError::Io(err.to_string())),
        };
        if bytes.is_empty() {
            return Ok(());
        }
        let snapshot: TicketRevocationSnapshot = decode_adaptive(&bytes)
            .map_err(|err| TicketRevocationStoreError::Parse(err.to_string()))?;
        for entry in snapshot.entries {
            let expires_at = UNIX_EPOCH + Duration::from_secs(entry.expires_at_secs);
            if is_expired(expires_at, now) || exceeds_ttl(expires_at, now, self.limits.max_ttl) {
                continue;
            }
            self.records
                .insert(entry.fingerprint, RevokedTicketRecord { expires_at });
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), TicketRevocationStoreError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        }
        let mut entries: Vec<_> = self.records.iter().collect();
        entries.sort_by_key(|(_, record)| record.expires_at);
        let snapshot = TicketRevocationSnapshot {
            entries: entries
                .into_iter()
                .filter_map(|(fingerprint, record)| {
                    let expires_secs = record.expires_at.duration_since(UNIX_EPOCH).ok()?;
                    Some(TicketRevocationSnapshotEntry {
                        fingerprint: *fingerprint,
                        expires_at_secs: expires_secs.as_secs(),
                    })
                })
                .collect(),
        };
        let buf = encode_adaptive(&snapshot);
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, buf).map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        fs::rename(&tmp_path, path).map_err(|err| TicketRevocationStoreError::Io(err.to_string()))
    }
}

/// Policy controlling `PoW` verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameters {
    difficulty: u8,
    max_future_skew: Duration,
    min_ttl: Duration,
}

/// Binding inputs mixed into a `PoW` challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeBinding<'a> {
    /// Descriptor commitment advertised by the relay.
    pub descriptor_commit: &'a [u8],
    /// Relay identifier bound into the challenge.
    pub relay_id: &'a [u8],
    /// Optional transcript hash to distinguish resumed circuits.
    pub transcript_hash: Option<&'a [u8]>,
}

impl<'a> ChallengeBinding<'a> {
    /// Construct a new binding descriptor.
    #[must_use]
    pub fn new(
        descriptor_commit: &'a [u8],
        relay_id: &'a [u8],
        transcript_hash: Option<&'a [u8]>,
    ) -> Self {
        Self {
            descriptor_commit,
            relay_id,
            transcript_hash,
        }
    }
}

impl Parameters {
    /// Construct new `PoW` parameters.
    ///
    /// # Panics
    ///
    /// Panics if `min_ttl` is zero or if `max_future_skew` is less than `min_ttl`.
    #[must_use]
    pub fn new(difficulty: u8, max_future_skew: Duration, min_ttl: Duration) -> Self {
        assert!(min_ttl > Duration::ZERO, "min_ttl must be > 0");
        assert!(
            max_future_skew >= min_ttl,
            "max_future_skew must be >= min_ttl"
        );
        Self {
            difficulty,
            max_future_skew,
            min_ttl,
        }
    }

    /// Returns the number of leading zero bits required in the solution digest.
    #[must_use]
    pub fn difficulty(&self) -> u8 {
        self.difficulty
    }

    /// Maximum allowed future skew (ticket expiry - now).
    #[must_use]
    pub fn max_future_skew(&self) -> Duration {
        self.max_future_skew
    }

    /// Minimum ticket lifetime allowed by the policy.
    #[must_use]
    pub fn min_ticket_ttl(&self) -> Duration {
        self.min_ttl
    }

    /// Returns a copy of the parameters with the supplied difficulty.
    #[must_use]
    pub fn with_difficulty(&self, difficulty: u8) -> Self {
        Self {
            difficulty,
            ..*self
        }
    }
}

/// Errors surfaced while validating tickets.
#[derive(Debug, Error)]
pub enum Error {
    /// Ticket payload failed to parse.
    #[error("malformed pow ticket: {0}")]
    Malformed(String),
    /// Ticket uses an unsupported version.
    #[error("unsupported pow ticket version {0}")]
    UnsupportedVersion(u8),
    /// Ticket difficulty does not match policy.
    #[error("ticket difficulty {ticket} does not match required {required}")]
    DifficultyMismatch {
        /// Difficulty encoded in the incoming ticket.
        ticket: u8,
        /// Difficulty required by local policy.
        required: u8,
    },
    /// Ticket has expired.
    #[error("pow ticket expired at {0}, current time {1}")]
    Expired(u64, u64),
    /// Ticket expires too far in the future relative to the relay clock.
    #[error("pow ticket expires too far in the future (>{0:?})")]
    FutureSkewExceeded(Duration),
    /// Ticket TTL is shorter than the policy minimum.
    #[error("pow ticket ttl shorter than required min ({0:?})")]
    ExpiryWindowTooSmall(Duration),
    /// Ticket failed the hash predicate.
    #[error("pow ticket solution invalid")]
    InvalidSolution,
    /// Signed ticket bound to a different relay than the verifier expects.
    #[error("pow ticket relay mismatch")]
    RelayMismatch,
    /// Signed ticket transcript hash did not match the verifier binding.
    #[error("pow ticket transcript mismatch")]
    TranscriptMismatch,
    /// Signed ticket has already been used or revoked.
    #[error("pow ticket replay detected")]
    Replay,
    /// ML-DSA signature verification failed.
    #[error("ticket signature invalid")]
    InvalidSignature,
    /// Signing operation failed.
    #[error("signing failed: {0}")]
    Signing(String),
    /// Post-quantum crypto error.
    #[error("pq crypto error: {0}")]
    PostQuantum(String),
    /// Revocation store failed to accept or load the entry.
    #[error("revocation store error: {0}")]
    RevocationStore(String),
    /// System clock unavailable.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
}

/// Errors surfaced while minting tickets.
#[derive(Debug, Error)]
pub enum MintError {
    /// Requested TTL was shorter than the policy minimum.
    #[error("requested ttl {requested:?} shorter than required minimum {required:?}")]
    TtlTooShort {
        /// TTL requested by the client.
        requested: Duration,
        /// Minimum TTL allowed by policy.
        required: Duration,
    },
    /// Requested TTL exceeded the allowed future skew.
    #[error("requested ttl {requested:?} exceeds max future skew {max_skew:?}")]
    TtlTooLong {
        /// TTL requested by the client.
        requested: Duration,
        /// Maximum future skew allowed by policy.
        max_skew: Duration,
    },
    /// System clock unavailable.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
}

/// Verify a ticket using the local `PoW` parameters.
///
/// # Errors
/// Returns [`Error`] if the ticket version or difficulty do not match the policy, the
/// ticket has expired, the lifetime falls outside the configured window, or the solution
/// fails the hash predicate.
pub fn verify(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
) -> Result<(), Error> {
    verify_at(ticket, binding, params, SystemTime::now())
}

/// Verify a ticket at a specific time (exposed for testing).
///
/// # Errors
/// Mirrors [`verify`] while allowing the caller to supply the reference timestamp.
pub fn verify_at(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    now: SystemTime,
) -> Result<(), Error> {
    if ticket.version != Ticket::VERSION {
        return Err(Error::UnsupportedVersion(ticket.version));
    }
    if ticket.difficulty != params.difficulty {
        return Err(Error::DifficultyMismatch {
            ticket: ticket.difficulty,
            required: params.difficulty,
        });
    }
    let now_duration = now.duration_since(UNIX_EPOCH)?;
    let now_secs = now_duration.as_secs();
    let expires_at = Duration::from_secs(ticket.expires_at);
    if expires_at <= now_duration {
        return Err(Error::Expired(ticket.expires_at, now_secs));
    }
    let ttl_remaining = expires_at - now_duration;
    let deficit = params.min_ttl.saturating_sub(ttl_remaining);
    if deficit > TTL_GRACE {
        return Err(Error::ExpiryWindowTooSmall(params.min_ttl));
    }
    if ttl_remaining > params.max_future_skew {
        return Err(Error::FutureSkewExceeded(params.max_future_skew));
    }

    let challenge = derive_challenge(binding, ticket.client_nonce, ticket.expires_at);
    let digest = derive_solution_digest(&challenge, &ticket.solution);
    if !leading_zero_bits_at_least(digest.as_bytes(), params.difficulty) {
        return Err(Error::InvalidSolution);
    }
    Ok(())
}

/// Verify a signed ticket, enforcing relay/transcript bindings and replay protection.
///
/// # Errors
/// Mirrors [`verify_signed_ticket_at`] while using the current system time.
pub fn verify_signed_ticket(
    signed_ticket: &SignedTicket,
    public_key: &[u8],
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    revocations: Option<&mut TicketRevocationStore>,
) -> Result<(), Error> {
    verify_signed_ticket_at(
        signed_ticket,
        public_key,
        binding,
        params,
        revocations,
        SystemTime::now(),
    )
}

/// Verify a signed ticket at a fixed timestamp (exposed for testing).
///
/// The signature is verified against `public_key`, relay and transcript bindings
/// must match the supplied `binding`, and (when provided) the revocation store
/// is consulted to reject and persist replays.
///
/// # Errors
/// Returns [`Error`] when signature verification fails, relay/transcript bindings
/// mismatch, `PoW` validation fails, or the revocation store refuses the entry.
pub fn verify_signed_ticket_at(
    signed_ticket: &SignedTicket,
    public_key: &[u8],
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    revocations: Option<&mut TicketRevocationStore>,
    now: SystemTime,
) -> Result<(), Error> {
    signed_ticket.verify(public_key)?;

    if signed_ticket.relay_id != *binding.relay_id {
        return Err(Error::RelayMismatch);
    }
    match (
        signed_ticket.transcript_hash.as_ref(),
        binding.transcript_hash,
    ) {
        (None, None) => {}
        (Some(ticket), Some(binding_hash)) if ticket == binding_hash => {}
        _ => return Err(Error::TranscriptMismatch),
    }

    verify_at(&signed_ticket.ticket, binding, params, now)?;

    if let Some(store) = revocations {
        if store.is_ticket_revoked(signed_ticket, now) {
            return Err(Error::Replay);
        }
        let outcome = store
            .revoke_ticket(signed_ticket, now)
            .map_err(|err| Error::RevocationStore(err.to_string()))?;
        handle_revocation_outcome(outcome, signed_ticket.ticket.expires_at, now)?;
    }

    Ok(())
}

/// Verify an unsigned ticket using the provided policy and bindings, recording
/// revocations in the supplied store when present.
///
/// # Errors
/// Mirrors [`verify_with_revocations_at`] while using the current system time.
pub fn verify_with_revocations(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    revocations: Option<&mut TicketRevocationStore>,
) -> Result<(), Error> {
    verify_with_revocations_at(ticket, binding, params, revocations, SystemTime::now())
}

/// Verify an unsigned ticket at a fixed timestamp and record its revocation.
///
/// # Errors
/// Returns [`Error`] for validation failures or [`Error::RevocationStore`] when
/// the replay guard rejects or fails to persist the ticket.
pub fn verify_with_revocations_at(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    revocations: Option<&mut TicketRevocationStore>,
    now: SystemTime,
) -> Result<(), Error> {
    verify_at(ticket, binding, params, now)?;
    record_revocation(ticket, revocations, now)
}

/// Record a ticket revocation in the provided store.
///
/// # Errors
/// Returns [`Error::Replay`] for duplicates, [`Error::Expired`] when the ticket
/// is stale, or [`Error::RevocationStore`] for persistence failures.
pub fn record_revocation(
    ticket: &Ticket,
    revocations: Option<&mut TicketRevocationStore>,
    now: SystemTime,
) -> Result<(), Error> {
    let Some(store) = revocations else {
        return Ok(());
    };
    if store.is_ticket_payload_revoked(ticket, now) {
        return Err(Error::Replay);
    }
    let outcome = store
        .revoke_ticket_payload(ticket, now)
        .map_err(|err| Error::RevocationStore(err.to_string()))?;
    handle_revocation_outcome(outcome, ticket.expires_at, now)
}

/// Mint a ticket satisfying the policy, returning the serialized structure.
///
/// # Errors
/// Returns [`MintError`] when the requested TTL violates the policy constraints or if the
/// system clock cannot be queried.
pub fn mint_ticket<R: RngCore + CryptoRng>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
) -> Result<Ticket, MintError> {
    if ttl < params.min_ttl {
        return Err(MintError::TtlTooShort {
            requested: ttl,
            required: params.min_ttl,
        });
    }
    if ttl > params.max_future_skew {
        return Err(MintError::TtlTooLong {
            requested: ttl,
            max_skew: params.max_future_skew,
        });
    }
    let now = SystemTime::now();
    let expires_at = now + ttl;
    let expires_at_secs = expires_at.duration_since(UNIX_EPOCH)?.as_secs();
    let mut client_nonce = [0u8; 32];
    rng.fill_bytes(&mut client_nonce);
    let challenge = derive_challenge(binding, client_nonce, expires_at_secs);

    loop {
        let mut solution = [0u8; 32];
        rng.fill_bytes(&mut solution);
        let digest = derive_solution_digest(&challenge, &solution);
        if leading_zero_bits_at_least(digest.as_bytes(), params.difficulty) {
            return Ok(Ticket {
                version: 1,
                difficulty: params.difficulty,
                expires_at: expires_at_secs,
                client_nonce,
                solution,
            });
        }
    }
}

fn derive_challenge(
    binding: &ChallengeBinding<'_>,
    client_nonce: [u8; 32],
    expires_at: u64,
) -> blake3::Hash {
    let transcript_len = binding.transcript_hash.map_or(0, <[u8]>::len);
    let relay_len = binding.relay_id.len();
    let mut input = Vec::with_capacity(
        CHALLENGE_DOMAIN.len()
            + binding.descriptor_commit.len()
            + relay_len
            + transcript_len
            + client_nonce.len()
            + 8,
    );
    input.extend_from_slice(CHALLENGE_DOMAIN);
    input.extend_from_slice(binding.descriptor_commit);
    input.extend_from_slice(binding.relay_id);
    if let Some(transcript) = binding.transcript_hash {
        input.extend_from_slice(transcript);
    }
    input.extend_from_slice(&client_nonce);
    input.extend_from_slice(&expires_at.to_be_bytes());
    hash(&input)
}

fn derive_solution_digest(challenge: &blake3::Hash, solution: &[u8; 32]) -> blake3::Hash {
    let mut input =
        Vec::with_capacity(SOLUTION_DOMAIN.len() + challenge.as_bytes().len() + solution.len());
    input.extend_from_slice(SOLUTION_DOMAIN);
    input.extend_from_slice(challenge.as_bytes());
    input.extend_from_slice(solution);
    hash(&input)
}

fn leading_zero_bits_at_least(bytes: &[u8], bits: u8) -> bool {
    if bits == 0 {
        return true;
    }
    let full_bytes = (bits / 8) as usize;
    let rem_bits = bits % 8;
    if bytes.len() < full_bytes {
        return false;
    }
    if bytes[..full_bytes].iter().any(|&byte| byte != 0) {
        return false;
    }
    if rem_bits == 0 {
        return true;
    }
    if bytes.len() <= full_bytes {
        return false;
    }
    let mask = 0xFF << (8 - rem_bits);
    bytes[full_bytes] & mask == 0
}

fn handle_revocation_outcome(
    outcome: TicketRevocationInsertOutcome,
    expires_at_secs: u64,
    now: SystemTime,
) -> Result<(), Error> {
    match outcome.status {
        TicketRevocationInsertStatus::Accepted => Ok(()),
        TicketRevocationInsertStatus::Duplicate => Err(Error::Replay),
        TicketRevocationInsertStatus::Expired => {
            let now_secs = now.duration_since(UNIX_EPOCH)?;
            Err(Error::Expired(expires_at_secs, now_secs.as_secs()))
        }
        TicketRevocationInsertStatus::TtlExceeded => Err(Error::RevocationStore(
            "revocation ttl exceeded configured maximum".to_string(),
        )),
        TicketRevocationInsertStatus::Capacity => Err(Error::RevocationStore(
            "revocation store at capacity".to_string(),
        )),
    }
}

fn compute_revocation_fingerprint(signature: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(REVOCATION_DOMAIN.len() + signature.len());
    input.extend_from_slice(REVOCATION_DOMAIN);
    input.extend_from_slice(signature);
    hash(&input).into()
}

fn is_expired(expires_at: SystemTime, now: SystemTime) -> bool {
    expires_at
        .duration_since(now)
        .map_or(true, |remaining| remaining.is_zero())
}

fn exceeds_ttl(expires_at: SystemTime, now: SystemTime, max_ttl: Duration) -> bool {
    expires_at
        .duration_since(now)
        .map_or(true, |ttl| ttl > max_ttl)
}

impl fmt::Display for Parameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "difficulty={}, max_future_skew={}s, min_ttl={}s",
            self.difficulty,
            self.max_future_skew.as_secs(),
            self.min_ttl.as_secs(),
        )
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use tempfile::tempdir;

    use super::*;

    const RELAY_A: [u8; 32] = [0xCC; 32];
    const RELAY_B: [u8; 32] = [0xDD; 32];

    fn params() -> Parameters {
        Parameters::new(5, Duration::from_secs(600), Duration::from_secs(30))
    }

    fn binding(descriptor: &[u8; 32]) -> ChallengeBinding<'_> {
        ChallengeBinding::new(descriptor, &RELAY_A, None)
    }

    fn other_binding(descriptor: &[u8; 32]) -> ChallengeBinding<'_> {
        ChallengeBinding::new(descriptor, &RELAY_B, None)
    }

    fn signed_ticket_with_expiry(expires_at: u64, signature_byte: u8) -> SignedTicket {
        SignedTicket {
            ticket: Ticket {
                version: 1,
                difficulty: 0,
                expires_at,
                client_nonce: [0u8; 32],
                solution: [0u8; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![signature_byte; 48],
        }
    }

    #[test]
    fn revocation_limits_require_positive_bounds() {
        assert!(
            TicketRevocationStoreLimits::new(0, Duration::from_secs(1)).is_err(),
            "capacity must be non-zero"
        );
        assert!(
            TicketRevocationStoreLimits::new(1, Duration::ZERO).is_err(),
            "max ttl must be non-zero"
        );
    }

    #[test]
    fn ticket_round_trip() {
        let mut rng = rand::rngs::StdRng::from_seed([0x42; 32]);
        let params = params();
        let descriptor = [0xAA; 32];
        let ttl = Duration::from_secs(90);
        let binding = binding(&descriptor);
        let ticket = mint_ticket(&params, &binding, ttl, &mut rng).expect("mint");
        let bytes = ticket.to_bytes();
        let parsed = Ticket::parse(&bytes).expect("parse");
        assert_eq!(ticket, parsed);
        verify(&parsed, &binding, &params).expect("verify");
    }

    #[test]
    fn rejects_unsupported_ticket_version() {
        let mut bytes = [0u8; TICKET_LEN];
        bytes[0] = Ticket::VERSION + 1;
        let err = Ticket::parse(&bytes).expect_err("unsupported version should fail");
        assert!(matches!(err, Error::UnsupportedVersion(_)));
    }

    #[test]
    fn rejects_bad_length() {
        let err = Ticket::parse(&[0u8; 10]).expect_err("should fail");
        matches!(err, Error::Malformed(_));
    }

    #[test]
    fn detects_invalid_solution() {
        let params = params();
        let descriptor = [0xAA; 32];
        let binding = binding(&descriptor);
        let mut ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: SystemTime::now()
                .checked_add(params.min_ticket_ttl())
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            client_nonce: [0x11; 32],
            solution: [0x00; 32],
        };
        let err = verify(&ticket, &binding, &params).expect_err("should fail");
        matches!(err, Error::InvalidSolution);

        ticket.difficulty = 0;
        assert!(verify(&ticket, &binding, &params).is_err());
    }

    #[test]
    fn detects_future_skew() {
        let params = params();
        let descriptor = [0u8; 32];
        let ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: SystemTime::now()
                .checked_add(params.max_future_skew() + Duration::from_secs(60))
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            client_nonce: [0x22; 32],
            solution: [0x33; 32],
        };
        let binding = binding(&descriptor);
        let err = verify(&ticket, &binding, &params).expect_err("should fail");
        matches!(err, Error::FutureSkewExceeded(_));
    }

    #[test]
    fn accepts_min_ttl_with_boundary_grace() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let base = 1_700_000_000;
        let descriptor = [0u8; 32];
        let ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: base + params.min_ticket_ttl().as_secs(),
            client_nonce: [0x44; 32],
            solution: [0x55; 32],
        };
        let now = UNIX_EPOCH + Duration::from_secs(base + 1);
        let binding = binding(&descriptor);
        verify_at(&ticket, &binding, &params, now).expect("slack should accept ticket");
    }

    #[test]
    fn rejects_ttl_far_below_min_even_with_grace() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let base = 1_700_000_000;
        let descriptor = [0u8; 32];
        let ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: base + params.min_ticket_ttl().as_secs(),
            client_nonce: [0x55; 32],
            solution: [0x66; 32],
        };
        let now = UNIX_EPOCH + Duration::from_secs(base + 2);
        let binding = binding(&descriptor);
        let err =
            verify_at(&ticket, &binding, &params, now).expect_err("should reject insufficient ttl");
        matches!(err, Error::ExpiryWindowTooSmall(_));
    }

    #[test]
    fn signed_ticket_roundtrip() {
        use soranet_pq::generate_mldsa_keypair;
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let ticket = Ticket {
            version: 1,
            difficulty: 5,
            expires_at: 100,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let relay_id = [0x33; 32];
        let transcript = [0x44; 32];

        let signed = SignedTicket::sign(ticket, &relay_id, Some(&transcript), kp.secret_key())
            .expect("sign");

        signed.verify(kp.public_key()).expect("verify");

        // Tamper with ticket
        let mut tampered = signed.clone();
        tampered.ticket.difficulty = 0;
        tampered
            .verify(kp.public_key())
            .expect_err("tampered ticket");

        // Tamper with relay_id
        let mut tampered_relay = signed.clone();
        tampered_relay.relay_id[0] ^= 0xFF;
        tampered_relay
            .verify(kp.public_key())
            .expect_err("tampered relay");
    }

    #[test]
    fn signed_ticket_encode_decode_roundtrip() {
        use soranet_pq::generate_mldsa_keypair;

        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed =
            SignedTicket::sign(ticket, &RELAY_A, None, kp.secret_key()).expect("sign ticket");
        let encoded = signed.encode();
        let decoded = SignedTicket::decode(&encoded).expect("decode");
        assert_eq!(decoded, signed);

        let err = SignedTicket::decode(&[]).expect_err("empty payload should fail");
        assert!(matches!(err, Error::Malformed(_)));
    }

    #[test]
    fn signed_ticket_decode_rejects_unsupported_version() {
        let ticket = Ticket {
            version: Ticket::VERSION + 1,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0x11],
        };
        let encoded = signed.encode();
        let err = SignedTicket::decode(&encoded).expect_err("unsupported version should fail");
        assert!(matches!(err, Error::UnsupportedVersion(_)));
    }

    #[test]
    fn ticket_reuse_is_allowed_with_same_binding() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let mut rng = rand::rngs::StdRng::from_seed([0x77; 32]);
        let descriptor = [0x42; 32];
        let binding = binding(&descriptor);
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng).expect("mint");

        verify(&ticket, &binding, &params).expect("first verify");
        verify(&ticket, &binding, &params).expect("replay verify");
    }

    #[test]
    fn ticket_reuse_rejected_with_mismatched_binding() {
        let params = Parameters::new(3, Duration::from_secs(600), Duration::from_secs(30));
        let mut rng = rand::rngs::StdRng::from_seed([0x55; 32]);
        let descriptor = [0x24; 32];
        let binding = binding(&descriptor);
        let other = other_binding(&descriptor);
        for _ in 0..8 {
            let ticket =
                mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng).expect("mint");

            verify(&ticket, &binding, &params).expect("verify with original binding");
            if let Err(err) = verify(&ticket, &other, &params) {
                assert!(matches!(err, Error::InvalidSolution));
                return;
            }
        }
        panic!("mismatched relay should fail");
    }

    #[test]
    fn rejects_mismatched_transcript_hash() {
        let params = Parameters::new(8, Duration::from_secs(300), Duration::from_secs(45));
        let mut rng = rand::rngs::StdRng::from_seed([0x12; 32]);
        let descriptor = [0xAC; 32];
        let transcript_a = [0x01; 32];
        let transcript_b = [0x02; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, Some(&transcript_a));
        let mismatched = ChallengeBinding::new(&descriptor, &RELAY_A, Some(&transcript_b));

        let mut observed_failure = false;
        for _ in 0..64 {
            let ticket =
                mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng).expect("mint");
            verify(&ticket, &binding, &params).expect("expected transcript to verify");
            if verify(&ticket, &mismatched, &params).is_err() {
                observed_failure = true;
                break;
            }
        }
        assert!(
            observed_failure,
            "mismatched transcript should reject minted tickets"
        );
    }

    #[test]
    fn revocation_store_persists_and_evicts_oldest() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("load");

        let ticket_a = signed_ticket_with_expiry(1_120, 0xAA);
        let ticket_b = signed_ticket_with_expiry(1_140, 0xBB);
        let ticket_c = signed_ticket_with_expiry(1_160, 0xCC);

        let outcome_a = store.revoke_ticket(&ticket_a, now).expect("insert a");
        assert_eq!(outcome_a.status, TicketRevocationInsertStatus::Accepted);
        let outcome_b = store.revoke_ticket(&ticket_b, now).expect("insert b");
        assert_eq!(outcome_b.status, TicketRevocationInsertStatus::Accepted);
        let outcome_c = store.revoke_ticket(&ticket_c, now).expect("insert c");
        assert_eq!(outcome_c.status, TicketRevocationInsertStatus::Accepted);
        assert_eq!(
            outcome_c.evicted,
            Some(ticket_a.revocation_fingerprint()),
            "oldest ticket should be evicted when capacity is reached"
        );
        assert!(store.is_ticket_revoked(&ticket_c, now));
        assert!(!store.is_ticket_revoked(&ticket_a, now));

        let reload_now = UNIX_EPOCH + Duration::from_secs(1_250);
        let reloaded =
            TicketRevocationStore::load(&path, limits, reload_now).expect("reload from disk");
        assert_eq!(
            reloaded.len(reload_now),
            0,
            "expired entries must be pruned on load"
        );
    }

    #[test]
    fn revocation_store_rejects_ttl_overflow_and_expiry() {
        let now = UNIX_EPOCH + Duration::from_secs(5_000);
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(60)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");

        let future = signed_ticket_with_expiry(5_200, 0x01);
        let overdue = signed_ticket_with_expiry(4_900, 0x02);

        let too_long = store.revoke_ticket(&future, now).expect("insert");
        assert_eq!(too_long.status, TicketRevocationInsertStatus::TtlExceeded);
        assert_eq!(store.len(now), 0);

        let expired = store.revoke_ticket(&overdue, now).expect("insert overdue");
        assert_eq!(expired.status, TicketRevocationInsertStatus::Expired);
        assert_eq!(store.len(now), 0);
    }

    #[test]
    fn revocation_store_purges_and_persists() {
        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(3, Duration::from_secs(180)).expect("limits");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("load");

        let short = signed_ticket_with_expiry(10_050, 0x10);
        let long = signed_ticket_with_expiry(10_140, 0x20);
        store.revoke_ticket(&short, now).expect("short insert");
        store.revoke_ticket(&long, now).expect("long insert");

        let later = UNIX_EPOCH + Duration::from_secs(10_120);
        let removed = store.purge_expired(later).expect("purge");
        assert_eq!(removed, 1);
        assert!(store.is_ticket_revoked(&long, later));
        assert!(!store.is_ticket_revoked(&short, later));

        let reloaded =
            TicketRevocationStore::load(&path, limits, later).expect("reload after purge");
        assert_eq!(reloaded.len(later), 1);
        assert!(reloaded.is_ticket_revoked(&long, later));
    }

    #[test]
    fn revocation_store_handles_raw_ticket_bytes() {
        let now = UNIX_EPOCH + Duration::from_secs(12_000);
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(600)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");

        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at: 12_120,
            client_nonce: [0xAB; 32],
            solution: [0xCD; 32],
        };

        let first = store
            .revoke_ticket_bytes(&ticket, now)
            .expect("first insert should succeed");
        assert_eq!(first.status, TicketRevocationInsertStatus::Accepted);

        let duplicate = store
            .revoke_ticket_bytes(&ticket, now)
            .expect("duplicate insert should succeed");
        assert_eq!(duplicate.status, TicketRevocationInsertStatus::Duplicate);
    }

    #[test]
    fn verify_with_revocations_rejects_replay_and_persists() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(600)).expect("limits");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("load");

        let descriptor = [0xAB; 32];
        let binding = binding(&descriptor);
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(60));
        let ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: now
                .checked_add(Duration::from_secs(300))
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            client_nonce: [0x10; 32],
            solution: [0x20; 32],
        };

        verify_with_revocations_at(&ticket, &binding, &params, Some(&mut store), now)
            .expect("first verification");
        let err = verify_with_revocations_at(&ticket, &binding, &params, Some(&mut store), now)
            .expect_err("replay should be rejected");
        matches!(err, Error::Replay);

        let later = now + Duration::from_secs(30);
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, later).expect("reload revocations");
        let err =
            verify_with_revocations_at(&ticket, &binding, &params, Some(&mut reloaded), later)
                .expect_err("replay should persist");
        matches!(err, Error::Replay);
    }

    #[test]
    fn revocation_store_accepts_new_ticket_after_eviction() {
        let now = UNIX_EPOCH + Duration::from_secs(8_000);
        let limits = TicketRevocationStoreLimits::new(1, Duration::from_secs(300)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");

        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(60));
        let descriptor = [0xBC; 32];
        let binding = binding(&descriptor);

        let mk_ticket = |expires_at_secs: u64, nonce: u8| Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: expires_at_secs,
            client_nonce: [nonce; 32],
            solution: [nonce.wrapping_add(1); 32],
        };

        let ticket_a = mk_ticket(8_120, 0x01);
        let ticket_b = mk_ticket(8_140, 0x02);

        verify_with_revocations_at(&ticket_a, &binding, &params, Some(&mut store), now)
            .expect("accept ticket a");
        verify_with_revocations_at(&ticket_b, &binding, &params, Some(&mut store), now)
            .expect("accept ticket b and evict a");

        assert!(
            store.is_ticket_payload_revoked(&ticket_b, now),
            "newest ticket should remain"
        );
        assert!(
            !store.is_ticket_payload_revoked(&ticket_a, now),
            "evicted ticket should be allowed again"
        );

        verify_with_revocations_at(&ticket_a, &binding, &params, Some(&mut store), now)
            .expect("evicted ticket should insert again");
    }

    #[test]
    fn signed_ticket_replay_rejected_after_reload() {
        use soranet_pq::generate_mldsa_keypair;

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let now = UNIX_EPOCH + Duration::from_secs(42_000);
        let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(600)).expect("limits");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("replay_store.norito");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("store");

        let params = Parameters::new(0, Duration::from_secs(900), Duration::from_secs(60));
        let descriptor = [0xAA; 32];
        let expires_at = now.duration_since(UNIX_EPOCH).expect("epoch").as_secs() + 120;
        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, None);
        let signed =
            SignedTicket::sign(ticket, &RELAY_A, None, keypair.secret_key()).expect("sign");

        verify_signed_ticket_at(
            &signed,
            keypair.public_key(),
            &binding,
            &params,
            Some(&mut store),
            now,
        )
        .expect("first verification");

        let err = verify_signed_ticket_at(
            &signed,
            keypair.public_key(),
            &binding,
            &params,
            Some(&mut store),
            now,
        )
        .expect_err("replay must fail");
        assert!(matches!(err, Error::Replay));

        let later = now + Duration::from_secs(30);
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, later).expect("reload from disk");
        let err = verify_signed_ticket_at(
            &signed,
            keypair.public_key(),
            &binding,
            &params,
            Some(&mut reloaded),
            later,
        )
        .expect_err("replay should persist across reload");
        assert!(matches!(err, Error::Replay));
    }

    #[test]
    fn signed_ticket_relay_mismatch_is_reported() {
        use soranet_pq::generate_mldsa_keypair;

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let now = UNIX_EPOCH + Duration::from_secs(50_000);
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(45));
        let descriptor = [0x55; 32];
        let expires_at = now.duration_since(UNIX_EPOCH).expect("epoch").as_secs() + 90;
        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let signed =
            SignedTicket::sign(ticket, &RELAY_A, None, keypair.secret_key()).expect("sign");
        let mismatched = ChallengeBinding::new(&descriptor, &RELAY_B, None);

        let err = verify_signed_ticket_at(
            &signed,
            keypair.public_key(),
            &mismatched,
            &params,
            None,
            now,
        )
        .expect_err("relay mismatch should fail");
        assert!(matches!(err, Error::RelayMismatch));
    }
}
