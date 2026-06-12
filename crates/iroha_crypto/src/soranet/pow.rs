//! `PoW` ticket helpers for the `SoraNet` admission protocol.

use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use norito::{
    codec::{decode_adaptive, encode_adaptive},
    decode_from_bytes,
    derive::{NoritoDeserialize, NoritoSerialize},
    to_bytes,
};
use rand_core::TryCryptoRng;
use soranet_pq::{MlDsaError, MlDsaSuite, sign_mldsa_from_os, verify_mldsa};
use thiserror::Error;

/// Domain separator used when deriving `PoW` challenges.
pub const CHALLENGE_DOMAIN: &[u8] = b"soranet.pow.challenge.v1";
/// Domain separator used when hashing `PoW` solutions.
pub const SOLUTION_DOMAIN: &[u8] = b"soranet.pow.solution.v1";
/// Domain separator used when signing `SignedTicket` payloads.
pub const SIGNING_DOMAIN: &[u8; 28] = b"soranet.pow.signed_ticket.v1";
/// Domain separator used when hashing revocation fingerprints.
pub const REVOCATION_DOMAIN: &[u8] = b"soranet.pow.revocation.v1";

/// Length of the serialized `PoW` ticket payload.
pub const TICKET_LEN: usize = 74;
const SIGNED_TICKET_PAYLOAD_BASE_LEN: usize = SIGNING_DOMAIN.len() + TICKET_LEN + 32;
const SIGNED_TICKET_PAYLOAD_MAX_LEN: usize = SIGNED_TICKET_PAYLOAD_BASE_LEN + 32;
/// Slack tolerated when validating the remaining TTL to account for second-level truncation.
const TTL_GRACE: Duration = Duration::from_secs(1);
const BINDING_FIELD_LEN: usize = 32;

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
        let mut cursor = 0usize;
        let version = read_ticket_byte(bytes, &mut cursor)?;
        if version != Self::VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let difficulty = read_ticket_byte(bytes, &mut cursor)?;
        let expires_at_bytes = read_ticket_field::<8>(bytes, &mut cursor)?;
        let expires_at = u64::from_be_bytes(expires_at_bytes);
        let client_nonce = read_ticket_field::<32>(bytes, &mut cursor)?;
        let solution = read_ticket_field::<32>(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(Error::Malformed(format!(
                "expected {TICKET_LEN} bytes, got {}",
                bytes.len()
            )));
        }

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
        self.checked_expires_at_time().unwrap_or(UNIX_EPOCH)
    }

    /// Returns the ticket expiration timestamp if it is representable by
    /// `SystemTime`.
    #[must_use]
    pub fn checked_expires_at_time(&self) -> Option<SystemTime> {
        unix_time_from_secs(self.expires_at)
    }

    /// Compute the revocation fingerprint for the ticket payload.
    #[must_use]
    pub fn revocation_fingerprint(&self) -> [u8; 32] {
        compute_revocation_fingerprint(&self.to_bytes())
    }
}

fn read_ticket_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, Error> {
    let end = (*cursor).checked_add(1).ok_or_else(|| {
        Error::Malformed(format!("expected {TICKET_LEN} bytes, got {}", bytes.len()))
    })?;
    let value = *bytes.get(*cursor).ok_or_else(|| {
        Error::Malformed(format!("expected {TICKET_LEN} bytes, got {}", bytes.len()))
    })?;
    *cursor = end;
    Ok(value)
}

fn read_ticket_field<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], Error> {
    let start = *cursor;
    let end = start.checked_add(N).ok_or_else(|| {
        Error::Malformed(format!("expected {TICKET_LEN} bytes, got {}", bytes.len()))
    })?;
    let slice = bytes.get(start..end).ok_or_else(|| {
        Error::Malformed(format!("expected {TICKET_LEN} bytes, got {}", bytes.len()))
    })?;
    let mut out = [0u8; N];
    out.copy_from_slice(slice);
    *cursor = end;
    Ok(out)
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

struct SignedTicketPayload {
    bytes: [u8; SIGNED_TICKET_PAYLOAD_MAX_LEN],
    len: usize,
}

impl SignedTicketPayload {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
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
        MlDsaSuite::MlDsa44
            .validate_secret_key(secret_key)
            .map_err(|err| Error::Signing(format!("ML-DSA secret key is invalid: {err}")))?;
        let payload = Self::build_payload(&ticket, relay_id, transcript_hash);
        let signature =
            sign_mldsa_from_os(MlDsaSuite::MlDsa44, secret_key, &[], payload.as_slice())
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
        Self::validate_signature_material(&decoded.signature)?;
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
    /// Returns [`Error::Malformed`] if the signature length is invalid,
    /// [`Error::InvalidSignature`] if verification fails, or [`Error::PostQuantum`]
    /// if the key format is invalid.
    pub fn verify(&self, public_key: &[u8]) -> Result<(), Error> {
        if self.ticket.version != Ticket::VERSION {
            return Err(Error::UnsupportedVersion(self.ticket.version));
        }
        Self::validate_signature_material(&self.signature)?;
        MlDsaSuite::MlDsa44
            .validate_public_key(public_key)
            .map_err(|err| Error::PostQuantum(err.to_string()))?;
        let payload =
            Self::build_payload(&self.ticket, &self.relay_id, self.transcript_hash.as_ref());
        verify_mldsa(
            MlDsaSuite::MlDsa44,
            public_key,
            &[],
            payload.as_slice(),
            &self.signature,
        )
        .map_err(|e| match e {
            MlDsaError::VerificationFailed(_) => Error::InvalidSignature,
            other => Error::PostQuantum(other.to_string()),
        })
    }

    fn validate_signature_material(signature: &[u8]) -> Result<(), Error> {
        validate_signed_ticket_signature_material(signature).map_err(Error::Malformed)
    }

    fn build_payload(
        ticket: &Ticket,
        relay_id: &[u8; 32],
        transcript_hash: Option<&[u8; 32]>,
    ) -> SignedTicketPayload {
        let mut payload = SignedTicketPayload {
            bytes: [0u8; SIGNED_TICKET_PAYLOAD_MAX_LEN],
            len: 0,
        };

        payload.bytes[payload.len..payload.len + SIGNING_DOMAIN.len()]
            .copy_from_slice(SIGNING_DOMAIN);
        payload.len += SIGNING_DOMAIN.len();
        payload.bytes[payload.len..payload.len + TICKET_LEN].copy_from_slice(&ticket.to_bytes());
        payload.len += TICKET_LEN;
        payload.bytes[payload.len..payload.len + relay_id.len()].copy_from_slice(relay_id);
        payload.len += relay_id.len();
        if let Some(hash) = transcript_hash {
            payload.bytes[payload.len..payload.len + hash.len()].copy_from_slice(hash);
            payload.len += hash.len();
        }
        debug_assert!(matches!(
            payload.len,
            SIGNED_TICKET_PAYLOAD_BASE_LEN | SIGNED_TICKET_PAYLOAD_MAX_LEN
        ));
        payload
    }

    /// Returns the ticket expiration timestamp as a `SystemTime`.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        self.checked_expires_at().unwrap_or(UNIX_EPOCH)
    }

    /// Returns the signed ticket expiration timestamp if it is representable by
    /// `SystemTime`.
    #[must_use]
    pub fn checked_expires_at(&self) -> Option<SystemTime> {
        self.ticket.checked_expires_at_time()
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
    /// Ticket expiry timestamp cannot be represented by `SystemTime`.
    #[error("revocation expiry timestamp {0} overflows system time")]
    ExpiryTimestampOverflow(u64),
    /// Raw signature material is not a valid signed-ticket signature.
    #[error("revocation signature malformed: {0}")]
    MalformedSignature(String),
}

#[derive(Debug, Clone, Copy)]
struct RevokedTicketRecord {
    expires_at: SystemTime,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
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
        let expires_at = ticket.checked_expires_at().ok_or(
            TicketRevocationStoreError::ExpiryTimestampOverflow(ticket.ticket.expires_at),
        )?;
        self.revoke_signature(&ticket.signature, expires_at, now)
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
        validate_signed_ticket_signature_material(signature)
            .map_err(TicketRevocationStoreError::MalformedSignature)?;
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
        let expires_at = ticket.checked_expires_at_time().ok_or(
            TicketRevocationStoreError::ExpiryTimestampOverflow(ticket.expires_at),
        )?;
        self.insert(ticket.revocation_fingerprint(), expires_at, now)
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
        let expires_at = ticket.checked_expires_at_time().ok_or(
            TicketRevocationStoreError::ExpiryTimestampOverflow(ticket.expires_at),
        )?;
        self.insert(fingerprint, expires_at, now)
    }

    /// Check if a signature has been revoked and is still within its TTL.
    #[must_use]
    pub fn is_revoked_signature(&self, signature: &[u8], now: SystemTime) -> bool {
        if validate_signed_ticket_signature_material(signature).is_err() {
            return false;
        }
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
        let snapshot: TicketRevocationSnapshot = decode_from_bytes(&bytes)
            .map_err(|err| TicketRevocationStoreError::Parse(err.to_string()))?;
        let mut seen_fingerprints = HashSet::with_capacity(snapshot.entries.len());
        for entry in snapshot.entries {
            if !seen_fingerprints.insert(entry.fingerprint) {
                return Err(TicketRevocationStoreError::Parse(
                    "duplicate revocation fingerprint in snapshot".to_owned(),
                ));
            }
            let expires_at = UNIX_EPOCH
                .checked_add(Duration::from_secs(entry.expires_at_secs))
                .ok_or_else(|| {
                    TicketRevocationStoreError::Parse(format!(
                        "revocation expiry timestamp {} overflows system time",
                        entry.expires_at_secs
                    ))
                })?;
            if is_expired(expires_at, now) || exceeds_ttl(expires_at, now, self.limits.max_ttl) {
                continue;
            }
            self.records
                .insert(entry.fingerprint, RevokedTicketRecord { expires_at });
            while self.records.len() > self.limits.max_entries {
                if self.evict_oldest().is_none() {
                    return Err(TicketRevocationStoreError::Parse(
                        "revocation snapshot exceeds capacity".to_owned(),
                    ));
                }
            }
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
        let buf =
            to_bytes(&snapshot).map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
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

/// Errors surfaced while constructing `PoW` policy parameters.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum ParameterError {
    /// The minimum ticket TTL must be non-zero.
    #[error("pow min_ttl must be greater than zero")]
    MinTtlZero,
    /// The maximum future skew must cover the minimum ticket TTL.
    #[error("pow max_future_skew {max_future_skew:?} is shorter than min_ttl {min_ttl:?}")]
    MaxFutureSkewTooShort {
        /// Configured maximum future skew.
        max_future_skew: Duration,
        /// Configured minimum ticket TTL.
        min_ttl: Duration,
    },
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
    /// Invalid bounds produce a fail-closed policy that rejects all minted and
    /// verified tickets. Runtime configuration loaders should prefer
    /// [`Parameters::try_new`] so invalid policy input can be surfaced as a
    /// configuration error.
    #[must_use]
    pub fn new(difficulty: u8, max_future_skew: Duration, min_ttl: Duration) -> Self {
        Self::try_new(difficulty, max_future_skew, min_ttl)
            .unwrap_or_else(|_| Self::fail_closed(difficulty))
    }

    fn fail_closed(difficulty: u8) -> Self {
        Self {
            difficulty,
            max_future_skew: Duration::ZERO,
            min_ttl: Duration::MAX,
        }
    }

    /// Construct new `PoW` parameters.
    ///
    /// # Errors
    /// Returns [`ParameterError`] if the minimum ticket TTL is zero or if the
    /// maximum future skew is shorter than the minimum ticket TTL.
    pub fn try_new(
        difficulty: u8,
        max_future_skew: Duration,
        min_ttl: Duration,
    ) -> Result<Self, ParameterError> {
        if min_ttl.is_zero() {
            return Err(ParameterError::MinTtlZero);
        }
        if max_future_skew < min_ttl {
            return Err(ParameterError::MaxFutureSkewTooShort {
                max_future_skew,
                min_ttl,
            });
        }
        Ok(Self {
            difficulty,
            max_future_skew,
            min_ttl,
        })
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
    /// Binding material has an invalid field length.
    #[error("malformed pow binding: {0}")]
    MalformedBinding(String),
}

/// Errors surfaced while minting tickets.
#[derive(Debug, Error)]
pub enum MintError {
    /// Binding material has an invalid field length.
    #[error("malformed pow binding: {0}")]
    MalformedBinding(String),
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
    /// Requested TTL cannot be represented as a `SystemTime` expiry.
    #[error("requested ttl {0:?} overflows system time")]
    ExpiryTimestampOverflow(Duration),
    /// Random byte generation failed while minting the ticket.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
    /// System clock unavailable.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
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
        })?;
    if dest.iter().all(|&byte| byte == 0) {
        return Err(MintError::RandomBytes {
            operation,
            message: "rng returned all-zero material".to_owned(),
        });
    }
    Ok(())
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
    validate_binding(binding).map_err(Error::MalformedBinding)?;
    validate_ticket_policy_at(ticket, params, now)?;
    verify_ticket_solution(ticket, binding, params)
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
    validate_binding(binding).map_err(Error::MalformedBinding)?;

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

    validate_ticket_policy_at(&signed_ticket.ticket, params, now)?;
    signed_ticket.verify(public_key)?;
    verify_ticket_solution(&signed_ticket.ticket, binding, params)?;

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

fn validate_ticket_policy_at(
    ticket: &Ticket,
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
    let ttl_remaining = expires_at
        .checked_sub(now_duration)
        .ok_or(Error::Expired(ticket.expires_at, now_secs))?;
    let deficit = params.min_ttl.saturating_sub(ttl_remaining);
    if deficit > TTL_GRACE {
        return Err(Error::ExpiryWindowTooSmall(params.min_ttl));
    }
    if ttl_remaining > params.max_future_skew {
        return Err(Error::FutureSkewExceeded(params.max_future_skew));
    }
    Ok(())
}

fn verify_ticket_solution(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
) -> Result<(), Error> {
    let challenge = derive_challenge(binding, ticket.client_nonce, ticket.expires_at);
    let digest = derive_solution_digest(&challenge, &ticket.solution);
    if !leading_zero_bits_at_least(digest.as_bytes(), params.difficulty) {
        return Err(Error::InvalidSolution);
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
/// Returns [`MintError`] when the requested TTL violates the policy constraints,
/// random bytes cannot be generated, or the system clock cannot be queried.
pub fn mint_ticket<R: TryCryptoRng>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
) -> Result<Ticket, MintError> {
    validate_binding(binding).map_err(MintError::MalformedBinding)?;
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
    let expires_at = now
        .checked_add(ttl)
        .ok_or(MintError::ExpiryTimestampOverflow(ttl))?;
    let expires_at_secs = expires_at.duration_since(UNIX_EPOCH)?.as_secs();
    let mut client_nonce = [0u8; 32];
    fill_random(rng, "minting PoW client nonce", &mut client_nonce)?;
    let challenge = derive_challenge(binding, client_nonce, expires_at_secs);

    loop {
        let mut solution = [0u8; 32];
        fill_random(rng, "minting PoW solution nonce", &mut solution)?;
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

fn validate_binding(binding: &ChallengeBinding<'_>) -> Result<(), String> {
    if binding.descriptor_commit.len() != BINDING_FIELD_LEN {
        return Err(format!(
            "descriptor_commit must be {BINDING_FIELD_LEN} bytes, got {}",
            binding.descriptor_commit.len()
        ));
    }
    if binding.relay_id.len() != BINDING_FIELD_LEN {
        return Err(format!(
            "relay_id must be {BINDING_FIELD_LEN} bytes, got {}",
            binding.relay_id.len()
        ));
    }
    if let Some(transcript_hash) = binding.transcript_hash
        && transcript_hash.len() != BINDING_FIELD_LEN
    {
        return Err(format!(
            "transcript_hash must be {BINDING_FIELD_LEN} bytes, got {}",
            transcript_hash.len()
        ));
    }
    Ok(())
}

fn derive_challenge(
    binding: &ChallengeBinding<'_>,
    client_nonce: [u8; 32],
    expires_at: u64,
) -> blake3::Hash {
    let mut hasher = Hasher::new();
    hasher.update(CHALLENGE_DOMAIN);
    hasher.update(binding.descriptor_commit);
    hasher.update(binding.relay_id);
    if let Some(transcript) = binding.transcript_hash {
        hasher.update(transcript);
    }
    hasher.update(&client_nonce);
    hasher.update(&expires_at.to_be_bytes());
    hasher.finalize()
}

fn derive_solution_digest(challenge: &blake3::Hash, solution: &[u8; 32]) -> blake3::Hash {
    let mut hasher = Hasher::new();
    hasher.update(SOLUTION_DOMAIN);
    hasher.update(challenge.as_bytes());
    hasher.update(solution);
    hasher.finalize()
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

fn unix_time_from_secs(secs: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
}

fn validate_signed_ticket_signature_material(signature: &[u8]) -> Result<(), String> {
    let expected = MlDsaSuite::MlDsa44.signature_len();
    if signature.len() != expected {
        return Err(format!(
            "signed ticket signature must be {expected} bytes, got {}",
            signature.len()
        ));
    }
    if signature.iter().all(|&byte| byte == 0) {
        return Err("signed ticket signature must not be all zero".to_owned());
    }
    Ok(())
}

fn compute_revocation_fingerprint(signature: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(REVOCATION_DOMAIN);
    hasher.update(signature);
    hasher.finalize().into()
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
    use rand_core::{TryCryptoRng, TryRngCore};
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

    struct FailingTryRng;

    #[derive(Debug)]
    struct FailingTryRngError;

    impl fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("failing PoW ticket RNG")
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

    struct FixedTryRng {
        byte: u8,
    }

    impl TryRngCore for FixedTryRng {
        type Error = core::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(u32::from_le_bytes([self.byte; 4]))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(u64::from_le_bytes([self.byte; 8]))
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
            dest.fill(self.byte);
            Ok(())
        }
    }

    impl TryCryptoRng for FixedTryRng {}

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
            signature: vec![signature_byte; MlDsaSuite::MlDsa44.signature_len()],
        }
    }

    fn write_revocation_snapshot(
        path: &std::path::Path,
        entries: Vec<TicketRevocationSnapshotEntry>,
    ) {
        let snapshot = TicketRevocationSnapshot { entries };
        let bytes = to_bytes(&snapshot).expect("encode revocation snapshot");
        std::fs::write(path, bytes).expect("write revocation snapshot");
    }

    fn invalid_solution_for(
        binding: &ChallengeBinding<'_>,
        client_nonce: [u8; 32],
        expires_at: u64,
        difficulty: u8,
    ) -> [u8; 32] {
        let challenge = derive_challenge(binding, client_nonce, expires_at);
        for suffix in u8::MIN..=u8::MAX {
            let mut solution = [0u8; 32];
            solution[31] = suffix;
            let digest = derive_solution_digest(&challenge, &solution);
            if !leading_zero_bits_at_least(digest.as_bytes(), difficulty) {
                return solution;
            }
        }
        panic!("expected at least one invalid solution for difficulty {difficulty}");
    }

    #[test]
    fn transcript_hashes_match_legacy_contiguous_layout() {
        let descriptor = [0x11; 32];
        let relay = [0x22; 32];
        let transcript = [0x33; 32];
        let client_nonce = [0x44; 32];
        let expires_at = 1_700_000_123_u64;

        for transcript_hash in [None, Some(transcript.as_slice())] {
            let binding = ChallengeBinding::new(&descriptor, &relay, transcript_hash);
            let mut legacy = Vec::with_capacity(
                CHALLENGE_DOMAIN.len()
                    + descriptor.len()
                    + relay.len()
                    + transcript_hash.map_or(0, <[u8]>::len)
                    + client_nonce.len()
                    + 8,
            );
            legacy.extend_from_slice(CHALLENGE_DOMAIN);
            legacy.extend_from_slice(&descriptor);
            legacy.extend_from_slice(&relay);
            if let Some(transcript_hash) = transcript_hash {
                legacy.extend_from_slice(transcript_hash);
            }
            legacy.extend_from_slice(&client_nonce);
            legacy.extend_from_slice(&expires_at.to_be_bytes());

            assert_eq!(
                derive_challenge(&binding, client_nonce, expires_at),
                blake3::hash(&legacy)
            );
        }

        let challenge = blake3::hash(b"challenge");
        let solution = [0x55; 32];
        let mut legacy_solution =
            Vec::with_capacity(SOLUTION_DOMAIN.len() + challenge.as_bytes().len() + solution.len());
        legacy_solution.extend_from_slice(SOLUTION_DOMAIN);
        legacy_solution.extend_from_slice(challenge.as_bytes());
        legacy_solution.extend_from_slice(&solution);
        assert_eq!(
            derive_solution_digest(&challenge, &solution),
            blake3::hash(&legacy_solution)
        );

        let signature = vec![0x66; 97];
        let mut legacy_revocation = Vec::with_capacity(REVOCATION_DOMAIN.len() + signature.len());
        legacy_revocation.extend_from_slice(REVOCATION_DOMAIN);
        legacy_revocation.extend_from_slice(&signature);
        let expected_revocation: [u8; 32] = blake3::hash(&legacy_revocation).into();
        assert_eq!(
            compute_revocation_fingerprint(&signature),
            expected_revocation
        );
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
    fn parameters_try_new_rejects_invalid_runtime_bounds() {
        let valid =
            Parameters::try_new(5, Duration::from_secs(600), Duration::from_secs(30)).expect("ok");
        assert_eq!(valid.difficulty(), 5);

        let zero_ttl = Parameters::try_new(5, Duration::from_secs(600), Duration::ZERO)
            .expect_err("zero min ttl must fail");
        assert!(matches!(zero_ttl, ParameterError::MinTtlZero));

        let inverted = Parameters::try_new(5, Duration::from_secs(29), Duration::from_secs(30))
            .expect_err("max future skew shorter than min ttl must fail");
        assert!(matches!(
            inverted,
            ParameterError::MaxFutureSkewTooShort {
                max_future_skew,
                min_ttl
            } if max_future_skew == Duration::from_secs(29)
                && min_ttl == Duration::from_secs(30)
        ));
    }

    #[test]
    fn parameters_new_invalid_bounds_fail_closed_without_panic() {
        let zero_ttl = Parameters::new(0, Duration::from_secs(600), Duration::ZERO);
        assert_eq!(zero_ttl.max_future_skew(), Duration::ZERO);
        assert_eq!(zero_ttl.min_ticket_ttl(), Duration::MAX);

        let inverted = Parameters::new(0, Duration::from_secs(29), Duration::from_secs(30));
        assert_eq!(inverted.max_future_skew(), Duration::ZERO);
        assert_eq!(inverted.min_ticket_ttl(), Duration::MAX);

        let descriptor = [0xAA; 32];
        let binding = binding(&descriptor);
        let mut rng = rand::rngs::StdRng::from_seed([0x24; 32]);
        let mint_err = mint_ticket(&zero_ttl, &binding, Duration::from_secs(30), &mut rng)
            .expect_err("fail-closed params must reject minting");
        assert!(matches!(
            mint_err,
            MintError::TtlTooShort {
                required: Duration::MAX,
                ..
            }
        ));

        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 1_120,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let verify_err = verify_at(
            &ticket,
            &binding,
            &inverted,
            UNIX_EPOCH + Duration::from_secs(1_000),
        )
        .expect_err("fail-closed params must reject verification");
        assert!(matches!(
            verify_err,
            Error::ExpiryWindowTooSmall(Duration::MAX)
        ));
    }

    #[test]
    fn mint_ticket_reports_rng_failure() {
        let descriptor = [0xAB; 32];
        let binding = binding(&descriptor);
        let mut rng = FailingTryRng;

        let err = mint_ticket(&params(), &binding, Duration::from_secs(30), &mut rng)
            .expect_err("failing RNG must abort ticket minting");

        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting PoW client nonce");
                assert!(
                    message.contains("failing PoW ticket RNG"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn fill_random_rejects_all_zero_nonce_material() {
        let mut rng = FixedTryRng { byte: 0 };
        let mut nonce = [0u8; 32];

        let err = fill_random(&mut rng, "minting PoW solution nonce", &mut nonce)
            .expect_err("all-zero PoW nonce material must fail");

        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting PoW solution nonce");
                assert!(message.contains("all-zero material"));
            }
            other => panic!("expected all-zero nonce RandomBytes error, got {other:?}"),
        }
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
    fn ticket_expiry_accessors_fail_closed_on_unrepresentable_timestamp() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: u64::MAX,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        assert!(ticket.checked_expires_at_time().is_none());
        assert_eq!(ticket.expires_at_time(), UNIX_EPOCH);

        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0xAA; MlDsaSuite::MlDsa44.signature_len()],
        };
        assert!(signed.checked_expires_at().is_none());
        assert_eq!(signed.expires_at(), UNIX_EPOCH);
    }

    #[test]
    fn mint_ticket_rejects_ttl_that_overflows_system_time() {
        let mut rng = rand::rngs::StdRng::from_seed([0x24; 32]);
        let params = Parameters::try_new(0, Duration::from_secs(u64::MAX), Duration::from_secs(1))
            .expect("huge bounds are structurally valid");
        let descriptor = [0xAA; 32];
        let binding = binding(&descriptor);

        let err = mint_ticket(&params, &binding, Duration::from_secs(u64::MAX), &mut rng)
            .expect_err("overflowing ttl should fail closed");
        assert!(matches!(
            err,
            MintError::ExpiryTimestampOverflow(ttl)
                if ttl == Duration::from_secs(u64::MAX)
        ));
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
        assert!(matches!(err, Error::Malformed(_)));
    }

    #[test]
    fn ticket_parse_rejects_truncated_prefixes_without_panic() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 1_700_000_120,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let bytes = ticket.to_bytes();

        for len in 0..TICKET_LEN {
            let err = Ticket::parse(&bytes[..len])
                .expect_err("truncated ticket prefix should fail closed");
            assert!(matches!(err, Error::Malformed(_)));
        }
    }

    #[test]
    fn ticket_field_readers_reject_overflowed_offsets_without_advancing() {
        let mut byte_cursor = usize::MAX;
        let err = read_ticket_byte(&[], &mut byte_cursor)
            .expect_err("overflowed byte cursor should fail closed");
        assert!(matches!(err, Error::Malformed(_)));
        assert_eq!(byte_cursor, usize::MAX);

        let mut field_cursor = usize::MAX;
        let err = read_ticket_field::<8>(&[], &mut field_cursor)
            .expect_err("overflowed field cursor should fail closed");
        assert!(matches!(err, Error::Malformed(_)));
        assert_eq!(field_cursor, usize::MAX);
    }

    #[test]
    fn verify_rejects_malformed_binding_lengths() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: 1_700_000_060,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let short_descriptor = [0x11; 31];
        let binding = ChallengeBinding::new(&short_descriptor, &RELAY_A, None);

        let err = verify_at(
            &ticket,
            &binding,
            &params,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("malformed descriptor binding must fail before challenge derivation");
        match err {
            Error::MalformedBinding(message) => {
                assert!(message.contains("descriptor_commit"));
            }
            other => panic!("expected malformed binding error, got {other:?}"),
        }

        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0xAA; MlDsaSuite::MlDsa44.signature_len()],
        };
        let err = verify_signed_ticket_at(
            &signed,
            &[],
            &binding,
            &params,
            None,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("malformed binding must fail before public-key validation");
        match err {
            Error::MalformedBinding(message) => {
                assert!(message.contains("descriptor_commit"));
            }
            other => panic!("expected malformed binding error, got {other:?}"),
        }
    }

    #[test]
    fn mint_ticket_rejects_malformed_binding_lengths() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let descriptor = [0x22; 32];
        let short_relay = [0x33; 31];
        let binding = ChallengeBinding::new(&descriptor, &short_relay, None);
        let mut rng = rand::rngs::StdRng::from_seed([0x44; 32]);

        let err = mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
            .expect_err("malformed relay binding must fail before minting");
        match err {
            MintError::MalformedBinding(message) => {
                assert!(message.contains("relay_id"));
            }
            other => panic!("expected malformed binding error, got {other:?}"),
        }
    }

    #[test]
    fn detects_invalid_solution() {
        let params = params();
        let descriptor = [0xAA; 32];
        let binding = binding(&descriptor);
        let expires_at = SystemTime::now()
            .checked_add(params.min_ticket_ttl())
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let client_nonce = [0x11; 32];
        let mut ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at,
            client_nonce,
            solution: invalid_solution_for(&binding, client_nonce, expires_at, params.difficulty()),
        };
        let err = verify(&ticket, &binding, &params).expect_err("should fail");
        assert!(matches!(err, Error::InvalidSolution));

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
        use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;
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
    fn signed_ticket_payload_matches_legacy_contiguous_layout() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_600,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let relay_id = [0x33; 32];
        let transcript = [0x44; 32];

        for transcript_hash in [None, Some(&transcript)] {
            let payload = SignedTicket::build_payload(&ticket, &relay_id, transcript_hash);
            let mut legacy =
                Vec::with_capacity(SIGNING_DOMAIN.len() + TICKET_LEN + relay_id.len() + 32);
            legacy.extend_from_slice(SIGNING_DOMAIN);
            legacy.extend_from_slice(&ticket.to_bytes());
            legacy.extend_from_slice(&relay_id);
            if let Some(hash) = transcript_hash {
                legacy.extend_from_slice(hash);
            }

            assert_eq!(payload.as_slice(), legacy.as_slice());
            assert_eq!(
                payload.as_slice().len(),
                if transcript_hash.is_some() {
                    SIGNED_TICKET_PAYLOAD_MAX_LEN
                } else {
                    SIGNED_TICKET_PAYLOAD_BASE_LEN
                }
            );
        }
    }

    #[test]
    fn signed_ticket_encode_decode_roundtrip() {
        use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;

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
    fn signed_ticket_decode_rejects_invalid_signature_length() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len() + 1],
        };
        let encoded = signed.encode();

        let err = SignedTicket::decode(&encoded).expect_err("invalid signature length should fail");
        match err {
            Error::Malformed(message) => {
                assert!(message.contains("signature"));
                assert!(message.contains("bytes"));
            }
            other => panic!("expected malformed signature length, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_decode_rejects_all_zero_signature_material() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0u8; MlDsaSuite::MlDsa44.signature_len()],
        };
        let encoded = signed.encode();

        let err = SignedTicket::decode(&encoded).expect_err("all-zero signature should fail");
        match err {
            Error::Malformed(message) => assert!(message.contains("all zero")),
            other => panic!("expected malformed all-zero signature, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_verify_rejects_invalid_signature_length_before_backend() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len() - 1],
        };

        let err = signed
            .verify(&[])
            .expect_err("invalid signature length should fail before key validation");
        match err {
            Error::Malformed(message) => {
                assert!(message.contains("signature"));
                assert!(message.contains("bytes"));
            }
            other => panic!("expected malformed signature length, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_verify_rejects_all_zero_signature_before_backend() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0u8; MlDsaSuite::MlDsa44.signature_len()],
        };

        let err = signed
            .verify(&[])
            .expect_err("all-zero signature should fail before key validation");
        match err {
            Error::Malformed(message) => assert!(message.contains("all zero")),
            other => panic!("expected malformed all-zero signature, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_verify_rejects_unsupported_version_before_signature_preflight() {
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
            signature: Vec::new(),
        };

        let err = signed
            .verify(&[])
            .expect_err("unsupported version must fail before signature checks");
        assert!(matches!(err, Error::UnsupportedVersion(_)));
    }

    #[test]
    fn signed_ticket_sign_rejects_invalid_secret_key_length_before_backend() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };

        let err = SignedTicket::sign(ticket, &RELAY_A, None, &[])
            .expect_err("invalid secret key length must fail before signing");
        match err {
            Error::Signing(message) => {
                assert!(
                    message.contains("secret key"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("expected signing key length error, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_verify_rejects_invalid_public_key_length_before_backend() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len()],
        };

        let err = signed
            .verify(&[])
            .expect_err("invalid public key length must fail before backend verification");
        match err {
            Error::PostQuantum(message) => {
                assert!(
                    message.contains("public key"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("expected public key length error, got {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_verifier_rejects_relay_mismatch_before_signature_preflight() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: 1_700_000_120,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let binding = other_binding(&descriptor);

        let err = verify_signed_ticket_at(
            &signed,
            &[],
            &binding,
            &params,
            None,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("relay mismatch must fail before signature or key preflight");
        assert!(matches!(err, Error::RelayMismatch));
    }

    #[test]
    fn signed_ticket_verifier_rejects_transcript_mismatch_before_signature_preflight() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: 1_700_000_120,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: Some([0x11; 32]),
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let expected_transcript = [0x22; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, Some(&expected_transcript));

        let err = verify_signed_ticket_at(
            &signed,
            &[],
            &binding,
            &params,
            None,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("transcript mismatch must fail before signature or key preflight");
        assert!(matches!(err, Error::TranscriptMismatch));
    }

    #[test]
    fn signed_ticket_verifier_rejects_policy_mismatch_before_signature_preflight() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty() + 1,
            expires_at: 1_700_000_120,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, None);

        let err = verify_signed_ticket_at(
            &signed,
            &[],
            &binding,
            &params,
            None,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("difficulty mismatch must fail before signature or key preflight");
        assert!(matches!(err, Error::DifficultyMismatch { .. }));
    }

    #[test]
    fn signed_ticket_verifier_rejects_expiry_before_signature_preflight() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: 1_699_999_999,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, None);

        let err = verify_signed_ticket_at(&signed, &[], &binding, &params, None, now)
            .expect_err("expired ticket must fail before signature or key preflight");
        assert!(matches!(err, Error::Expired(_, _)));
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
    fn revocation_store_load_enforces_capacity_by_evicting_oldest() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let old = [0x01; 32];
        let middle = [0x02; 32];
        let newest = [0x03; 32];
        write_revocation_snapshot(
            &path,
            vec![
                TicketRevocationSnapshotEntry {
                    fingerprint: old,
                    expires_at_secs: 2_120,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint: middle,
                    expires_at_secs: 2_160,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint: newest,
                    expires_at_secs: 2_220,
                },
            ],
        );

        let store = TicketRevocationStore::load(&path, limits, now).expect("load capped snapshot");
        let active = store.active_fingerprints(now);
        assert_eq!(active.len(), 2);
        assert!(!active.contains(&old));
        assert!(active.contains(&middle));
        assert!(active.contains(&newest));
    }

    #[test]
    fn revocation_store_load_rejects_duplicate_fingerprints() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let fingerprint = [0x44; 32];
        write_revocation_snapshot(
            &path,
            vec![
                TicketRevocationSnapshotEntry {
                    fingerprint,
                    expires_at_secs: 2_120,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint,
                    expires_at_secs: 2_160,
                },
            ],
        );

        let err =
            TicketRevocationStore::load(&path, limits, now).expect_err("duplicate should fail");
        match err {
            TicketRevocationStoreError::Parse(message) => {
                assert!(message.contains("duplicate"));
                assert!(message.contains("fingerprint"));
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn revocation_store_load_rejects_overflowing_expiry() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        write_revocation_snapshot(
            &path,
            vec![TicketRevocationSnapshotEntry {
                fingerprint: [0x55; 32],
                expires_at_secs: u64::MAX,
            }],
        );

        let err =
            TicketRevocationStore::load(&path, limits, now).expect_err("overflow should fail");
        match err {
            TicketRevocationStoreError::Parse(message) => {
                assert!(message.contains("expiry"));
                assert!(message.contains("overflows"));
            }
            other => panic!("expected parse error, got {other:?}"),
        }
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
    fn revocation_store_rejects_unrepresentable_ticket_expiry_without_panic() {
        let now = UNIX_EPOCH + Duration::from_secs(5_000);
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(60)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: u64::MAX,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0xAA; MlDsaSuite::MlDsa44.signature_len()],
        };

        let err = store
            .revoke_ticket_payload(&ticket, now)
            .expect_err("overflowed ticket payload expiry should fail closed");
        assert!(matches!(
            err,
            TicketRevocationStoreError::ExpiryTimestampOverflow(u64::MAX)
        ));

        let err = store
            .revoke_ticket_bytes(&ticket, now)
            .expect_err("overflowed ticket byte expiry should fail closed");
        assert!(matches!(
            err,
            TicketRevocationStoreError::ExpiryTimestampOverflow(u64::MAX)
        ));

        let err = store
            .revoke_ticket(&signed, now)
            .expect_err("overflowed signed-ticket expiry should fail closed");
        assert!(matches!(
            err,
            TicketRevocationStoreError::ExpiryTimestampOverflow(u64::MAX)
        ));
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
    fn revocation_store_rejects_malformed_raw_signature_material() {
        let now = UNIX_EPOCH + Duration::from_secs(11_000);
        let expires_at = now + Duration::from_secs(120);
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(600)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");
        let short = vec![0x11; MlDsaSuite::MlDsa44.signature_len() - 1];
        let all_zero = vec![0u8; MlDsaSuite::MlDsa44.signature_len()];

        let err = store
            .revoke_signature(&short, expires_at, now)
            .expect_err("short raw signature should fail");
        match err {
            TicketRevocationStoreError::MalformedSignature(message) => {
                assert!(message.contains("signature"));
                assert!(message.contains("bytes"));
            }
            other => panic!("expected malformed raw signature length, got {other:?}"),
        }

        let err = store
            .revoke_signature(&all_zero, expires_at, now)
            .expect_err("all-zero raw signature should fail");
        match err {
            TicketRevocationStoreError::MalformedSignature(message) => {
                assert!(message.contains("all zero"));
            }
            other => panic!("expected malformed all-zero raw signature, got {other:?}"),
        }

        assert_eq!(store.len(now), 0);
        assert!(!store.is_revoked_signature(&short, now));
        assert!(!store.is_revoked_signature(&all_zero, now));
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
        use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;

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
        use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;

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
