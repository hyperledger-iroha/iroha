//! `PoW` ticket helpers for the `SoraNet` admission protocol.
//!
//! Persistent ticket-consumption snapshots are hard-bounded, decoded under
//! explicit Norito limits, and loaded only from stable direct regular files.
use super::{
    replay_lock::ExclusiveLedgerLock,
    snapshot_file::{
        BoundedWriter, create_temporary_direct_regular_file, persist_temporary_snapshot,
        read_optional_bounded_regular_file,
    },
};
use blake3::Hasher;
#[cfg(test)]
use norito::to_bytes;
use norito::{
    DecodeLimits,
    codec::{decode_exact_from_slice_with_limits, encode_adaptive},
    decode_canonical_with_limits,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use rand_core::TryCryptoRng;
use soranet_pq::{MlDsaError, MlDsaSuite, sign_mldsa_from_os, verify_mldsa};
use std::{
    collections::HashMap,
    fmt,
    ops::Deref,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
use zeroize::{Zeroize as _, Zeroizing};
/// Domain separator used when deriving `PoW` challenges.
pub const CHALLENGE_DOMAIN: &[u8] = b"soranet.pow.challenge.v1";
/// Domain separator used when hashing `PoW` solutions.
pub const SOLUTION_DOMAIN: &[u8] = b"soranet.pow.solution.v1";
/// Domain separator used when signing `SignedTicket` payloads.
pub const SIGNING_DOMAIN: &[u8; 28] = b"soranet.pow.signed_ticket.v1";
/// Domain separator used when hashing revocation fingerprints.
pub const REVOCATION_DOMAIN: &[u8] = b"soranet.pow.revocation.v1";
/// Domain separator used to bind admission credentials to an exact client hello.
pub const ADMISSION_TRANSCRIPT_DOMAIN: &[u8] = b"soranet.pow.admission_transcript.v1";
/// Domain separator for the exact relay/transcript commitment carried by a ticket.
pub const TICKET_BINDING_DOMAIN: &[u8] = b"soranet.pow.ticket_binding.v1";
/// Length of the serialized `PoW` ticket payload.
pub const TICKET_LEN: usize = 74;

fn clear_sensitive_vec(value: &mut Vec<u8>) {
    value.resize(value.capacity(), 0);
    value.as_mut_slice().zeroize();
    value.clear();
}

/// Fixed-width ticket serialization whose bearer bytes are redacted and
/// scrubbed on drop.
pub struct TicketBytes([u8; TICKET_LEN]);
impl TicketBytes {
    fn clear(&mut self) {
        self.0.zeroize();
    }
}
impl AsRef<[u8]> for TicketBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl Deref for TicketBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl fmt::Debug for TicketBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TicketBytes(<redacted>)")
    }
}
impl Drop for TicketBytes {
    fn drop(&mut self) {
        self.clear();
    }
}
const SIGNED_TICKET_PAYLOAD_LEN: usize = SIGNING_DOMAIN.len() + TICKET_LEN + 32 + 32;
/// Maximum accepted bare Norito encoding for one signed ticket.
pub const SIGNED_TICKET_MAX_ENCODED_BYTES_V1: usize = 8 * 1024;
const SIGNED_TICKET_DECODE_MAX_NESTING_DEPTH_V1: usize = 8;
/// Slack tolerated when validating the remaining TTL to account for second-level truncation.
const TTL_GRACE: Duration = Duration::from_secs(1);
const BINDING_FIELD_LEN: usize = 32;
const REVOCATION_SNAPSHOT_BASE_LIMIT_BYTES: usize = 4 * 1024;
const REVOCATION_SNAPSHOT_ENTRY_LIMIT_BYTES: usize = 128;
const REVOCATION_SNAPSHOT_DECODE_MAX_NESTING_DEPTH_V1: usize = 8;
const REVOCATION_SNAPSHOT_VERSION_V1: u8 = 1;
/// First-release hard ceiling for persistent ticket-revocation entries.
pub const TICKET_REVOCATION_STORE_MAX_ENTRIES_V1: usize = 65_536;
/// Derive the mandatory admission transcript commitment for a serialized client hello.
///
/// The exact length and bytes are committed so an admission credential cannot
/// be moved to a different handshake, even when optional hello fields differ.
#[must_use]
pub fn derive_admission_transcript(client_hello: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ADMISSION_TRANSCRIPT_DOMAIN);
    let length = u64::try_from(client_hello.len()).unwrap_or(u64::MAX);
    hasher.update(&length.to_be_bytes());
    hasher.update(client_hello);
    *hasher.finalize().as_bytes()
}
pub(super) fn ticket_binding_commitment(
    descriptor_commit: &[u8],
    relay_id: &[u8],
    transcript_hash: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(TICKET_BINDING_DOMAIN);
    for field in [descriptor_commit, relay_id, transcript_hash.as_slice()] {
        let length = u64::try_from(field.len()).unwrap_or(u64::MAX);
        hasher.update(&length.to_be_bytes());
        hasher.update(field);
    }
    *hasher.finalize().as_bytes()
}
/// Hashcash-style ticket attached to `SoraNet` circuit establishment.
#[derive(PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct Ticket {
    /// Ticket format version (currently `1`).
    pub version: u8,
    /// Number of leading zero bits required in the solution.
    pub difficulty: u8,
    /// UNIX timestamp (seconds) when the ticket expires.
    pub expires_at: u64,
    /// Domain-separated commitment to the descriptor, relay, and admission transcript.
    ///
    /// Verifiers compare this field before performing proof work so a ticket
    /// cannot be moved to another admission attempt by chance.
    pub client_nonce: [u8; 32],
    /// Solution nonce satisfying the difficulty predicate.
    pub solution: [u8; 32],
}
impl fmt::Debug for Ticket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Ticket")
            .field("version", &self.version)
            .field("difficulty", &self.difficulty)
            .field("expires_at", &self.expires_at)
            .field("client_nonce", &"[REDACTED]")
            .field("solution", &"[REDACTED]")
            .finish()
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.zeroize_sensitive_fields();
    }
}
impl Ticket {
    /// Current ticket format version.
    pub const VERSION: u8 = 1;
    /// Serialize the ticket to a fixed-length zeroizing owner.
    #[must_use]
    pub fn to_bytes(&self) -> TicketBytes {
        let mut out = TicketBytes([0u8; TICKET_LEN]);
        out.0[0] = self.version;
        out.0[1] = self.difficulty;
        out.0[2..10].copy_from_slice(&self.expires_at.to_be_bytes());
        out.0[10..42].copy_from_slice(&self.client_nonce);
        out.0[42..74].copy_from_slice(&self.solution);
        out
    }
    /// Serialize the ticket to a `Vec<u8>`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.to_bytes().as_ref().to_vec()
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
        unix_time_from_secs(expires_at).ok_or(Error::ExpiryTimestampOverflow(expires_at))?;
        let client_nonce = Zeroizing::new(read_ticket_field::<32>(bytes, &mut cursor)?);
        let solution = Zeroizing::new(read_ticket_field::<32>(bytes, &mut cursor)?);
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
            client_nonce: *client_nonce,
            solution: *solution,
        })
    }
    /// Returns the ticket expiration timestamp as a `SystemTime`.
    #[must_use]
    pub fn expires_at_time(&self) -> SystemTime {
        self.checked_expires_at_time().unwrap_or(UNIX_EPOCH)
    }
    /// Returns the ticket expiration timestamp if it is representable by `SystemTime`.
    #[must_use]
    pub fn checked_expires_at_time(&self) -> Option<SystemTime> {
        unix_time_from_secs(self.expires_at)
    }
    /// Compute the canonical revocation fingerprint for this ticket payload.
    #[must_use]
    pub fn revocation_fingerprint(&self) -> [u8; 32] {
        compute_ticket_revocation_fingerprint(&self.to_bytes())
    }

    fn zeroize_sensitive_fields(&mut self) {
        self.version.zeroize();
        self.difficulty.zeroize();
        self.expires_at.zeroize();
        self.client_nonce.zeroize();
        self.solution.zeroize();
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
/// A canonical Argon2 admission ticket envelope signed using ML-DSA-44.
///
/// The signature covers the ticket bytes, relay ID, and mandatory transcript
/// hash. Production consumers must verify the enclosed ticket with
/// `soranet::puzzle`; a signature never downgrades admission to hashcash or a
/// difficulty-zero grant.
#[derive(PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
pub struct SignedTicket {
    /// The underlying Argon2 ticket wire structure.
    pub ticket: Ticket,
    /// The relay identifier (32 bytes) that signed this ticket.
    pub relay_id: [u8; 32],
    /// Transcript hash binding the ticket to a specific session.
    pub transcript_hash: [u8; 32],
    /// ML-DSA-44 signature over `(ticket || relay_id || transcript_hash)`.
    pub signature: Vec<u8>,
}
impl fmt::Debug for SignedTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignedTicket")
            .field("ticket", &self.ticket)
            .field("relay_id", &"[REDACTED]")
            .field("transcript_hash", &"[REDACTED]")
            .field("signature", &"[REDACTED]")
            .finish()
    }
}
impl SignedTicket {
    fn zeroize_sensitive_fields(&mut self) {
        self.ticket.zeroize_sensitive_fields();
        self.relay_id.zeroize();
        self.transcript_hash.zeroize();
        clear_sensitive_vec(&mut self.signature);
    }
}
impl Drop for SignedTicket {
    fn drop(&mut self) {
        self.zeroize_sensitive_fields();
    }
}
struct SignedTicketPayload {
    bytes: [u8; SIGNED_TICKET_PAYLOAD_LEN],
}
impl SignedTicketPayload {
    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn zeroize_sensitive_fields(&mut self) {
        self.bytes.zeroize();
    }
}
impl Drop for SignedTicketPayload {
    fn drop(&mut self) {
        self.zeroize_sensitive_fields();
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
        transcript_hash: &[u8; 32],
        secret_key: &[u8],
    ) -> Result<Self, Error> {
        Self::validate_ticket_format(&ticket)?;
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
            transcript_hash: *transcript_hash,
            signature: signature.into_bytes(),
        })
    }
    /// Decode a signed ticket from a Norito payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when the payload fails to parse.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > SIGNED_TICKET_MAX_ENCODED_BYTES_V1 {
            return Err(Error::Malformed(format!(
                "signed ticket length {} exceeds first-release maximum {SIGNED_TICKET_MAX_ENCODED_BYTES_V1}",
                bytes.len()
            )));
        }
        let decoded: Self =
            decode_exact_from_slice_with_limits(bytes, signed_ticket_decode_limits_v1())
                .map_err(|err| Error::Malformed(format!("signed ticket decode failed: {err}")))?;
        Self::validate_ticket_format(&decoded.ticket)?;
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
    /// Returns [`Error::Malformed`] if the signature length is invalid, [`Error::InvalidSignature`]
    /// if verification fails, or [`Error::PostQuantum`] if the key format is invalid.
    pub fn verify(&self, public_key: &[u8]) -> Result<(), Error> {
        Self::validate_ticket_format(&self.ticket)?;
        Self::validate_signature_material(&self.signature)?;
        MlDsaSuite::MlDsa44
            .validate_public_key(public_key)
            .map_err(|err| Error::PostQuantum(err.to_string()))?;
        let payload = Self::build_payload(&self.ticket, &self.relay_id, &self.transcript_hash);
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
    fn validate_ticket_format(ticket: &Ticket) -> Result<(), Error> {
        if ticket.version != Ticket::VERSION {
            return Err(Error::UnsupportedVersion(ticket.version));
        }
        ticket
            .checked_expires_at_time()
            .ok_or(Error::ExpiryTimestampOverflow(ticket.expires_at))?;
        Ok(())
    }
    fn build_payload(
        ticket: &Ticket,
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
    ) -> SignedTicketPayload {
        let mut payload = SignedTicketPayload {
            bytes: [0u8; SIGNED_TICKET_PAYLOAD_LEN],
        };
        let mut offset = 0;
        payload.bytes[offset..offset + SIGNING_DOMAIN.len()].copy_from_slice(SIGNING_DOMAIN);
        offset += SIGNING_DOMAIN.len();
        payload.bytes[offset..offset + TICKET_LEN].copy_from_slice(&ticket.to_bytes());
        offset += TICKET_LEN;
        payload.bytes[offset..offset + relay_id.len()].copy_from_slice(relay_id);
        offset += relay_id.len();
        payload.bytes[offset..offset + transcript_hash.len()].copy_from_slice(transcript_hash);
        debug_assert_eq!(offset + transcript_hash.len(), SIGNED_TICKET_PAYLOAD_LEN);
        payload
    }
    /// Returns the ticket expiration timestamp as a `SystemTime`.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        self.checked_expires_at().unwrap_or(UNIX_EPOCH)
    }
    /// Returns the signed ticket expiration timestamp if it is representable by `SystemTime`.
    #[must_use]
    pub fn checked_expires_at(&self) -> Option<SystemTime> {
        self.ticket.checked_expires_at_time()
    }
    /// Compute the canonical revocation fingerprint for the underlying ticket.
    ///
    /// Signed and unsigned presentations intentionally share one replay
    /// identity; randomized re-signing cannot mint a fresh revocation key.
    #[must_use]
    pub fn revocation_fingerprint(&self) -> [u8; 32] {
        self.ticket.revocation_fingerprint()
    }
}
fn signed_ticket_decode_limits_v1() -> DecodeLimits {
    DecodeLimits::new(
        SIGNED_TICKET_MAX_ENCODED_BYTES_V1,
        SIGNED_TICKET_MAX_ENCODED_BYTES_V1,
        SIGNED_TICKET_MAX_ENCODED_BYTES_V1,
        SIGNED_TICKET_MAX_ENCODED_BYTES_V1.saturating_mul(2),
        SIGNED_TICKET_DECODE_MAX_NESTING_DEPTH_V1,
    )
}
/// Limits applied to the revocation store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TicketRevocationStoreLimits {
    /// Maximum number of active fingerprints to retain before failing closed.
    pub max_entries: usize,
    /// Maximum TTL allowed for a revoked ticket relative to insertion time.
    pub max_ttl: Duration,
}
impl TicketRevocationStoreLimits {
    /// Create limits, rejecting zero capacity or zero TTL.
    ///
    /// # Errors
    ///
    /// Returns [`TicketRevocationStoreError`] when `max_entries` is zero or above
    /// the first-release ceiling, or when `max_ttl` is zero.
    pub fn new(max_entries: usize, max_ttl: Duration) -> Result<Self, TicketRevocationStoreError> {
        if max_entries == 0 {
            return Err(TicketRevocationStoreError::CapacityZero);
        }
        if max_entries > TICKET_REVOCATION_STORE_MAX_ENTRIES_V1 {
            return Err(TicketRevocationStoreError::CapacityTooLarge {
                requested: max_entries,
                limit: TICKET_REVOCATION_STORE_MAX_ENTRIES_V1,
            });
        }
        if max_ttl.is_zero() {
            return Err(TicketRevocationStoreError::TtlZero);
        }
        Ok(Self {
            max_entries,
            max_ttl,
        })
    }
    fn max_snapshot_bytes(self) -> usize {
        self.max_entries
            .checked_mul(REVOCATION_SNAPSHOT_ENTRY_LIMIT_BYTES)
            .and_then(|bytes| bytes.checked_add(REVOCATION_SNAPSHOT_BASE_LIMIT_BYTES))
            .expect("hard-bounded revocation capacity fits snapshot envelope")
    }
    fn decode_limits(self) -> DecodeLimits {
        let max_snapshot_bytes = self.max_snapshot_bytes();
        DecodeLimits::new(
            TICKET_REVOCATION_STORE_MAX_ENTRIES_V1,
            max_snapshot_bytes,
            TICKET_REVOCATION_STORE_MAX_ENTRIES_V1.saturating_add(4),
            max_snapshot_bytes.saturating_mul(2),
            REVOCATION_SNAPSHOT_DECODE_MAX_NESTING_DEPTH_V1,
        )
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
    /// Store is full of active entries and rejected the new entry.
    Capacity,
}
/// Outcome of an insertion attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TicketRevocationInsertOutcome {
    /// Final status.
    pub status: TicketRevocationInsertStatus,
}
impl TicketRevocationInsertOutcome {
    const fn accepted() -> Self {
        Self {
            status: TicketRevocationInsertStatus::Accepted,
        }
    }
    const fn rejected(status: TicketRevocationInsertStatus) -> Self {
        Self { status }
    }
}
/// Errors surfaced by the revocation store.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TicketRevocationStoreError {
    /// Store capacity cannot be zero.
    #[error("revocation store capacity must be greater than zero")]
    CapacityZero,
    /// Store capacity exceeds the first-release hard ceiling.
    #[error("revocation store capacity {requested} exceeds first-release limit {limit}")]
    CapacityTooLarge {
        /// Requested entry count.
        requested: usize,
        /// First-release entry ceiling.
        limit: usize,
    },
    /// TTL bound must be non-zero.
    #[error("revocation store max_ttl must be greater than zero")]
    TtlZero,
    /// A bounded collection could not reserve its configured capacity.
    #[error("revocation store allocation failed while reserving {entries} entries")]
    Allocation {
        /// Number of entries requested from the allocator.
        entries: usize,
    },
    /// Persistent revocation paths must identify a concrete file.
    #[error("revocation store path must not be empty")]
    PathEmpty,
    /// Filesystem error while reading or writing the store.
    #[error("revocation store io error: {0}")]
    Io(String),
    /// Persisted snapshot failed to parse.
    #[error("revocation store parse error: {0}")]
    Parse(String),
    /// Ticket expiry timestamp cannot be represented by `SystemTime`.
    #[error("revocation expiry timestamp {0} overflows system time")]
    ExpiryTimestampOverflow(u64),
    /// A signed ticket carries malformed signature material.
    #[error("signed-ticket revocation credential malformed: {0}")]
    MalformedSignature(String),
}
#[derive(Debug, Clone, Copy)]
struct RevokedTicketRecord {
    expires_at: SystemTime,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
struct TicketRevocationSnapshot {
    version: u8,
    high_watermark_secs: u64,
    high_watermark_nanos: u32,
    entries: Vec<TicketRevocationSnapshotEntry>,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct TicketRevocationSnapshotEntry {
    fingerprint: [u8; 32],
    expires_at_secs: u64,
    expires_at_nanos: u32,
}
/// Persistent store for revoked or consumed ticket fingerprints.
///
/// Active entries are never evicted to admit newer entries: once the store is
/// full, insertion fails closed until an entry expires. Persistent instances
/// hold an exclusive sidecar lock so two processes cannot fork replay history.
/// Mutating operations and rejected insertion decisions durably advance a
/// monotonic clock high-water mark. Read-only queries retain expired records
/// and do not write merely because wall time advanced; after clock rollback, a
/// retained record becomes active again and therefore fails closed.
#[derive(Debug)]
pub struct TicketRevocationStore {
    limits: TicketRevocationStoreLimits,
    high_watermark: SystemTime,
    records: HashMap<[u8; 32], RevokedTicketRecord>,
    dirty: bool,
    ledger_lock: Option<ExclusiveLedgerLock>,
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
            high_watermark: UNIX_EPOCH,
            records: HashMap::new(),
            dirty: false,
            ledger_lock: None,
        })
    }
    /// Load or create a persistent store at `path`.
    ///
    /// The path must be absolute and its parent chain must be custodied by the
    /// process owner or root; snapshots and sidecar locks are owner-private.
    /// Persistent custody is currently supported only on Unix; [`Self::in_memory`]
    /// remains available on other targets.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] if the limits are invalid or the on-disk
    /// snapshot cannot be safely custodied, read, or parsed.
    pub fn load(
        path: impl Into<PathBuf>,
        limits: TicketRevocationStoreLimits,
        now: SystemTime,
    ) -> Result<Self, TicketRevocationStoreError> {
        let limits = TicketRevocationStoreLimits::new(limits.max_entries, limits.max_ttl)?;
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(TicketRevocationStoreError::PathEmpty);
        }
        let ledger_lock = ExclusiveLedgerLock::acquire(&path)
            .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        let mut store = Self {
            limits,
            high_watermark: now.max(UNIX_EPOCH),
            records: HashMap::new(),
            dirty: true,
            ledger_lock: Some(ledger_lock),
        };
        store.load_from_disk(now)?;
        Ok(store)
    }
    /// Insert a `SignedTicket` using the canonical underlying-ticket identity.
    ///
    /// # Errors
    /// Propagates [`TicketRevocationStoreError`] when persistence fails while
    /// recording the revocation.
    pub fn revoke_ticket(
        &mut self,
        ticket: &SignedTicket,
        now: SystemTime,
    ) -> Result<TicketRevocationInsertOutcome, TicketRevocationStoreError> {
        validate_signed_ticket_signature_material(&ticket.signature)
            .map_err(TicketRevocationStoreError::MalformedSignature)?;
        let expires_at = ticket.checked_expires_at().ok_or(
            TicketRevocationStoreError::ExpiryTimestampOverflow(ticket.ticket.expires_at),
        )?;
        self.insert(ticket.revocation_fingerprint(), expires_at, now)
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
    /// Check if a ticket payload has been revoked and is still within its TTL.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] when a previously failed mutation
    /// cannot be made durable.
    pub fn is_ticket_payload_revoked(
        &mut self,
        ticket: &Ticket,
        now: SystemTime,
    ) -> Result<bool, TicketRevocationStoreError> {
        let fingerprint = ticket.revocation_fingerprint();
        self.is_revoked_fingerprint(&fingerprint, now)
    }
    /// Check if a ticket has been revoked.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] when a previously failed mutation
    /// cannot be made durable.
    pub fn is_ticket_revoked(
        &mut self,
        ticket: &SignedTicket,
        now: SystemTime,
    ) -> Result<bool, TicketRevocationStoreError> {
        validate_signed_ticket_signature_material(&ticket.signature)
            .map_err(TicketRevocationStoreError::MalformedSignature)?;
        self.is_revoked_fingerprint(&ticket.revocation_fingerprint(), now)
    }
    /// Number of active (non-expired) fingerprints.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] when a previously failed mutation
    /// cannot be made durable.
    pub fn len(&mut self, now: SystemTime) -> Result<usize, TicketRevocationStoreError> {
        let effective_now = self.effective_now_for_read(now)?;
        Ok(self
            .records
            .values()
            .filter(|record| !is_expired(record.expires_at, effective_now))
            .count())
    }
    /// Return the active fingerprints retained by the store.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError::Allocation`] if the bounded result
    /// cannot reserve memory, or an IO error if a previously failed mutation
    /// still cannot be made durable.
    pub fn active_fingerprints(
        &mut self,
        now: SystemTime,
    ) -> Result<Vec<[u8; 32]>, TicketRevocationStoreError> {
        let effective_now = self.effective_now_for_read(now)?;
        let active_len = self
            .records
            .values()
            .filter(|record| !is_expired(record.expires_at, effective_now))
            .count();
        let mut fingerprints = Vec::new();
        fingerprints.try_reserve_exact(active_len).map_err(|_| {
            TicketRevocationStoreError::Allocation {
                entries: active_len,
            }
        })?;
        fingerprints.extend(self.records.iter().filter_map(|(fingerprint, record)| {
            (!is_expired(record.expires_at, effective_now)).then_some(*fingerprint)
        }));
        Ok(fingerprints)
    }
    /// Remove expired entries and persist updates.
    ///
    /// # Errors
    /// Returns [`TicketRevocationStoreError`] when the updated snapshot cannot be written.
    pub fn purge_expired(&mut self, now: SystemTime) -> Result<usize, TicketRevocationStoreError> {
        let before = self.records.len();
        let effective_now = self.observe_now(now);
        self.prune_expired_in_memory(effective_now);
        let removed = before.saturating_sub(self.records.len());
        if self.dirty {
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
        let effective_now = self.observe_now(now);
        self.prune_expired_in_memory(effective_now);
        if is_expired(expires_at, effective_now) {
            if self.dirty {
                self.persist()?;
            }
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::Expired,
            ));
        }
        if exceeds_ttl(expires_at, effective_now, self.limits.max_ttl) {
            if self.dirty {
                self.persist()?;
            }
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::TtlExceeded,
            ));
        }
        if self.records.contains_key(&fingerprint) {
            if self.dirty {
                self.persist()?;
            }
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::Duplicate,
            ));
        }
        if self.records.len() >= self.limits.max_entries {
            // An unexpired consumption record is a security invariant, not a
            // cache entry. Forgetting one to admit a newer ticket would make
            // the forgotten ticket replayable for the rest of its lifetime.
            if self.dirty {
                self.persist()?;
            }
            return Ok(TicketRevocationInsertOutcome::rejected(
                TicketRevocationInsertStatus::Capacity,
            ));
        }
        if self.records.try_reserve(1).is_err() {
            if self.dirty {
                self.persist()?;
            }
            return Err(TicketRevocationStoreError::Allocation { entries: 1 });
        }
        self.records
            .insert(fingerprint, RevokedTicketRecord { expires_at });
        self.dirty = true;
        self.persist()?;
        Ok(TicketRevocationInsertOutcome::accepted())
    }
    fn is_revoked_fingerprint(
        &mut self,
        fingerprint: &[u8; 32],
        now: SystemTime,
    ) -> Result<bool, TicketRevocationStoreError> {
        let effective_now = self.effective_now_for_read(now)?;
        Ok(self
            .records
            .get(fingerprint)
            .is_some_and(|record| !is_expired(record.expires_at, effective_now)))
    }
    fn load_from_disk(&mut self, now: SystemTime) -> Result<(), TicketRevocationStoreError> {
        let Some(ledger_lock) = &self.ledger_lock else {
            return Ok(());
        };
        let bytes = match read_optional_bounded_regular_file(
            ledger_lock.custody(),
            self.limits.max_snapshot_bytes(),
            "ticket revocation snapshot",
        ) {
            Ok(Some(bytes)) => bytes,
            // Materialise the empty ledger immediately so startup validates
            // that replay state is actually durable before serving clients.
            Ok(None) => return self.persist(),
            Err(err) => return Err(TicketRevocationStoreError::Io(err.to_string())),
        };
        if bytes.is_empty() {
            return Err(TicketRevocationStoreError::Parse(
                "revocation snapshot is empty".to_owned(),
            ));
        }
        let snapshot: TicketRevocationSnapshot =
            decode_canonical_with_limits(&bytes, self.limits.decode_limits())
                .map_err(|err| TicketRevocationStoreError::Parse(err.to_string()))?;
        drop(bytes);
        if snapshot.version != REVOCATION_SNAPSHOT_VERSION_V1 {
            return Err(TicketRevocationStoreError::Parse(format!(
                "unsupported revocation snapshot version {}",
                snapshot.version
            )));
        }
        let persisted_high_watermark = decode_revocation_timestamp(
            snapshot.high_watermark_secs,
            snapshot.high_watermark_nanos,
            "revocation high-water timestamp",
        )?;
        self.high_watermark = self.high_watermark.max(persisted_high_watermark);
        let effective_now = self.observe_now(now);
        if snapshot.entries.len() > self.limits.max_entries {
            return Err(TicketRevocationStoreError::Parse(
                "revocation snapshot exceeds capacity".to_owned(),
            ));
        }
        if snapshot.entries.windows(2).any(|pair| {
            revocation_snapshot_entry_order(&pair[0], &pair[1]) != std::cmp::Ordering::Less
        }) {
            return Err(TicketRevocationStoreError::Parse(
                "revocation snapshot entries are not in strict canonical order".to_owned(),
            ));
        }
        self.records
            .try_reserve(snapshot.entries.len())
            .map_err(|_| TicketRevocationStoreError::Allocation {
                entries: snapshot.entries.len(),
            })?;
        for entry in snapshot.entries {
            let expires_at = decode_revocation_timestamp(
                entry.expires_at_secs,
                entry.expires_at_nanos,
                "revocation expiry timestamp",
            )?;
            if self
                .records
                .insert(entry.fingerprint, RevokedTicketRecord { expires_at })
                .is_some()
            {
                return Err(TicketRevocationStoreError::Parse(
                    "duplicate revocation fingerprint in snapshot".to_owned(),
                ));
            }
            if !is_expired(expires_at, effective_now)
                && exceeds_ttl(expires_at, effective_now, self.limits.max_ttl)
            {
                return Err(TicketRevocationStoreError::Parse(format!(
                    "active revocation expiry exceeds configured max_ttl of {:?}",
                    self.limits.max_ttl
                )));
            }
        }
        self.prune_expired_in_memory(effective_now);
        self.persist()
    }
    fn prune_expired_in_memory(&mut self, now: SystemTime) {
        let previous_len = self.records.len();
        self.records
            .retain(|_, record| !is_expired(record.expires_at, now));
        self.dirty |= self.records.len() != previous_len;
    }
    fn observe_now(&mut self, now: SystemTime) -> SystemTime {
        if now > self.high_watermark {
            self.high_watermark = now;
            self.dirty = true;
        }
        self.high_watermark
    }
    fn effective_now_for_read(
        &mut self,
        now: SystemTime,
    ) -> Result<SystemTime, TicketRevocationStoreError> {
        // Read-only observations never prune records or advance the durable
        // clock. Retaining an entry makes a later clock rollback stricter: the
        // entry becomes active again. A dirty mutation is different; no later
        // successful decision may escape until its full snapshot is durable.
        if self.dirty {
            self.persist()?;
        }
        Ok(self.high_watermark.max(now))
    }
    fn persist(&mut self) -> Result<(), TicketRevocationStoreError> {
        let Some(ledger_lock) = &self.ledger_lock else {
            self.dirty = false;
            return Ok(());
        };
        let mut entries = Vec::new();
        entries.try_reserve_exact(self.records.len()).map_err(|_| {
            TicketRevocationStoreError::Allocation {
                entries: self.records.len(),
            }
        })?;
        for (fingerprint, record) in &self.records {
            let (expires_at_secs, expires_at_nanos) =
                encode_revocation_timestamp(record.expires_at, "revocation expiry timestamp")?;
            entries.push(TicketRevocationSnapshotEntry {
                fingerprint: *fingerprint,
                expires_at_secs,
                expires_at_nanos,
            });
        }
        entries.sort_by(revocation_snapshot_entry_order);
        let (high_watermark_secs, high_watermark_nanos) =
            encode_revocation_timestamp(self.high_watermark, "revocation high-water timestamp")?;
        let snapshot = TicketRevocationSnapshot {
            version: REVOCATION_SNAPSHOT_VERSION_V1,
            high_watermark_secs,
            high_watermark_nanos,
            entries,
        };
        let tmp = create_temporary_direct_regular_file(
            ledger_lock.custody(),
            "temporary ticket revocation snapshot",
        )
        .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        let mut bounded = BoundedWriter::new(
            tmp,
            self.limits.max_snapshot_bytes(),
            "ticket revocation snapshot",
        );
        norito::core::write_canonical_to_writer(&snapshot, &mut bounded)
            .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        let tmp = bounded.into_inner();
        tmp.as_file()
            .sync_all()
            .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        persist_temporary_snapshot(tmp, ledger_lock.custody(), "ticket revocation snapshot")
            .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        #[cfg(unix)]
        ledger_lock
            .custody()
            .sync_parent()
            .map_err(|err| TicketRevocationStoreError::Io(err.to_string()))?;
        self.dirty = false;
        Ok(())
    }
}
fn revocation_snapshot_entry_order(
    left: &TicketRevocationSnapshotEntry,
    right: &TicketRevocationSnapshotEntry,
) -> std::cmp::Ordering {
    left.expires_at_secs
        .cmp(&right.expires_at_secs)
        .then_with(|| left.expires_at_nanos.cmp(&right.expires_at_nanos))
        .then_with(|| left.fingerprint.cmp(&right.fingerprint))
}
fn decode_revocation_timestamp(
    seconds: u64,
    nanos: u32,
    label: &str,
) -> Result<SystemTime, TicketRevocationStoreError> {
    if nanos >= 1_000_000_000 {
        return Err(TicketRevocationStoreError::Parse(format!(
            "{label} has noncanonical nanoseconds {nanos}"
        )));
    }
    UNIX_EPOCH
        .checked_add(Duration::new(seconds, nanos))
        .ok_or_else(|| TicketRevocationStoreError::Parse(format!("{label} overflows system time")))
}
fn encode_revocation_timestamp(
    timestamp: SystemTime,
    label: &str,
) -> Result<(u64, u32), TicketRevocationStoreError> {
    let duration = timestamp.duration_since(UNIX_EPOCH).map_err(|_| {
        TicketRevocationStoreError::Parse(format!("{label} predates the Unix epoch"))
    })?;
    Ok((duration.as_secs(), duration.subsec_nanos()))
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
    /// Transcript hash binding the ticket to this admission attempt.
    pub transcript_hash: &'a [u8; 32],
}
impl<'a> ChallengeBinding<'a> {
    /// Construct a new binding descriptor.
    #[must_use]
    pub fn new(
        descriptor_commit: &'a [u8],
        relay_id: &'a [u8],
        transcript_hash: &'a [u8; 32],
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
    /// Invalid bounds produce a fail-closed policy that rejects all minted and verified tickets.
    /// Runtime configuration loaders should prefer [`Parameters::try_new`] so invalid policy input
    /// can be surfaced as a configuration error.
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
    /// Ticket expiry timestamp cannot be represented by `SystemTime`.
    #[error("pow ticket expiry timestamp {0} overflows system time")]
    ExpiryTimestampOverflow(u64),
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
    /// The system clock moved backwards while a proof candidate was being
    /// evaluated, so the resulting expiry window cannot be trusted.
    #[error("system clock moved backwards while minting pow ticket")]
    ClockMovedBackwards,
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
    if dest.len() > 1 && dest.iter().all(|&byte| byte == dest[0]) {
        return Err(MintError::RandomBytes {
            operation,
            message: "rng returned all-identical-byte material".to_owned(),
        });
    }
    Ok(())
}
fn reject_repeated_nonce_material(
    operation: &'static str,
    candidate: &[u8; 32],
    prior: &[(&'static str, &[u8; 32])],
) -> Result<(), MintError> {
    if let Some((label, _)) = prior
        .iter()
        .find(|(_, bytes)| bool::from(candidate.ct_eq(*bytes)))
    {
        return Err(MintError::RandomBytes {
            operation,
            message: format!("rng repeated {label} material"),
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
/// Verify a hashcash-signed ticket at a fixed timestamp for legacy unit fixtures.
///
/// Production signed tickets are mandatory Argon2 envelopes and are verified by
/// [`crate::soranet::puzzle::verify_signed_ticket_at`]. Keeping this helper
/// test-only prevents a caller from silently selecting hashcash for a signed
/// credential.
///
/// # Errors
/// Returns [`Error`] when signature verification fails, relay/transcript bindings
/// mismatch, `PoW` validation fails, or the revocation store refuses the entry.
#[cfg(test)]
fn verify_signed_ticket_at(
    signed_ticket: &SignedTicket,
    public_key: &[u8],
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    mut revocations: Option<&mut TicketRevocationStore>,
    now: SystemTime,
) -> Result<(), Error> {
    validate_binding(binding).map_err(Error::MalformedBinding)?;
    if signed_ticket.relay_id != *binding.relay_id {
        return Err(Error::RelayMismatch);
    }
    if signed_ticket.transcript_hash != *binding.transcript_hash {
        return Err(Error::TranscriptMismatch);
    }
    validate_ticket_policy_at(&signed_ticket.ticket, params, now)?;
    // The canonical ticket identity is cheap to derive and does not depend on
    // ML-DSA's randomized signature. Reject a known replay before performing
    // public-key verification so repeated bearer use cannot amplify CPU work.
    SignedTicket::validate_signature_material(&signed_ticket.signature)?;
    if let Some(store) = revocations.as_deref_mut()
        && store
            .is_ticket_revoked(signed_ticket, now)
            .map_err(|err| Error::RevocationStore(err.to_string()))?
    {
        return Err(Error::Replay);
    }
    signed_ticket.verify(public_key)?;
    verify_ticket_solution(&signed_ticket.ticket, binding, params)?;
    if let Some(store) = revocations {
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
    unix_time_from_secs(ticket.expires_at)
        .ok_or(Error::ExpiryTimestampOverflow(ticket.expires_at))?;
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
    let expected_binding = ticket_binding_commitment(
        binding.descriptor_commit,
        binding.relay_id,
        binding.transcript_hash,
    );
    if !bool::from(ticket.client_nonce.ct_eq(&expected_binding)) {
        return Err(Error::InvalidSolution);
    }
    let challenge = derive_challenge(binding, &ticket.client_nonce, ticket.expires_at);
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
    if store
        .is_ticket_payload_revoked(ticket, now)
        .map_err(|err| Error::RevocationStore(err.to_string()))?
    {
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
/// random bytes cannot be generated, or the system clock is unavailable or regresses.
pub fn mint_ticket<R: TryCryptoRng>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
) -> Result<Ticket, MintError> {
    mint_ticket_with_clock(params, binding, ttl, rng, SystemTime::now)
}
fn mint_ticket_with_clock<R, F>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
    now: F,
) -> Result<Ticket, MintError>
where
    R: TryCryptoRng,
    F: FnMut() -> SystemTime,
{
    mint_ticket_with_clock_and_digest(params, binding, ttl, rng, now, derive_solution_digest)
}
fn mint_ticket_with_clock_and_digest<R, F, D>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
    mut now: F,
    mut derive_digest: D,
) -> Result<Ticket, MintError>
where
    R: TryCryptoRng,
    F: FnMut() -> SystemTime,
    D: FnMut(&blake3::Hash, &[u8; 32]) -> blake3::Hash,
{
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
    let client_nonce = Zeroizing::new(ticket_binding_commitment(
        binding.descriptor_commit,
        binding.relay_id,
        binding.transcript_hash,
    ));
    let mut previous_solution: Option<Zeroizing<[u8; 32]>> = None;
    loop {
        let minted_at = now();
        let expires_at = minted_at
            .checked_add(ttl)
            .ok_or(MintError::ExpiryTimestampOverflow(ttl))?;
        let expires_at_secs = expires_at.duration_since(UNIX_EPOCH)?.as_secs();
        let wire_expires_at =
            unix_time_from_secs(expires_at_secs).ok_or(MintError::ExpiryTimestampOverflow(ttl))?;
        let mut prior = Vec::with_capacity(2);
        prior.push(("ticket binding commitment", &*client_nonce));
        if let Some(previous) = previous_solution.as_ref() {
            prior.push(("previous solution nonce", &**previous));
        }
        let challenge = derive_challenge(binding, &client_nonce, expires_at_secs);
        let mut solution = Zeroizing::new([0u8; 32]);
        fill_random(rng, "minting PoW solution nonce", &mut solution[..])?;
        reject_repeated_nonce_material("minting PoW solution nonce", &solution, &prior)?;
        let digest = derive_digest(&challenge, &solution);
        let solved_at = now();
        if solved_at < minted_at {
            return Err(MintError::ClockMovedBackwards);
        }
        let Ok(remaining) = wire_expires_at.duration_since(solved_at) else {
            previous_solution = Some(solution);
            continue;
        };
        if params.min_ttl.saturating_sub(remaining) > TTL_GRACE {
            // Expiry is challenge-bound. Re-anchor the next candidate instead
            // of returning proof bytes that current verifier policy rejects.
            previous_solution = Some(solution);
            continue;
        }
        if leading_zero_bits_at_least(digest.as_bytes(), params.difficulty) {
            return Ok(Ticket {
                version: 1,
                difficulty: params.difficulty,
                expires_at: expires_at_secs,
                client_nonce: *client_nonce,
                solution: *solution,
            });
        }
        previous_solution = Some(solution);
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
    Ok(())
}
fn derive_challenge(
    binding: &ChallengeBinding<'_>,
    client_nonce: &[u8; 32],
    expires_at: u64,
) -> blake3::Hash {
    let mut hasher = Hasher::new();
    hasher.update(CHALLENGE_DOMAIN);
    hasher.update(binding.descriptor_commit);
    hasher.update(binding.relay_id);
    hasher.update(binding.transcript_hash);
    hasher.update(client_nonce);
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
fn compute_ticket_revocation_fingerprint(ticket: &TicketBytes) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(REVOCATION_DOMAIN);
    hasher.update(ticket.as_ref());
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
    use super::*;
    use rand::SeedableRng;
    use rand_core::{TryCryptoRng, TryRngCore};
    use tempfile::tempdir;
    const RELAY_A: [u8; 32] = [0xCC; 32];
    const RELAY_B: [u8; 32] = [0xDD; 32];
    const TRANSCRIPT: [u8; 32] = [0xEE; 32];
    fn params() -> Parameters {
        Parameters::new(5, Duration::from_secs(600), Duration::from_secs(30))
    }
    fn binding(descriptor: &[u8; 32]) -> ChallengeBinding<'_> {
        ChallengeBinding::new(descriptor, &RELAY_A, &TRANSCRIPT)
    }
    fn other_binding(descriptor: &[u8; 32]) -> ChallengeBinding<'_> {
        ChallengeBinding::new(descriptor, &RELAY_B, &TRANSCRIPT)
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
            transcript_hash: TRANSCRIPT,
            signature: vec![signature_byte; MlDsaSuite::MlDsa44.signature_len()],
        }
    }
    fn write_revocation_snapshot(
        path: &std::path::Path,
        mut entries: Vec<TicketRevocationSnapshotEntry>,
    ) {
        entries.sort_by(revocation_snapshot_entry_order);
        let snapshot = TicketRevocationSnapshot {
            version: REVOCATION_SNAPSHOT_VERSION_V1,
            high_watermark_secs: 0,
            high_watermark_nanos: 0,
            entries,
        };
        let bytes = to_bytes(&snapshot).expect("encode revocation snapshot");
        write_private_test_file(path, &bytes);
    }
    fn write_private_test_file(path: &std::path::Path, bytes: &[u8]) {
        use std::io::Write as _;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(path).expect("open private test file");
        file.write_all(bytes).expect("write private test file");
    }
    fn invalid_solution_for(
        binding: &ChallengeBinding<'_>,
        client_nonce: [u8; 32],
        expires_at: u64,
        difficulty: u8,
    ) -> [u8; 32] {
        let challenge = derive_challenge(binding, &client_nonce, expires_at);
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
    fn ticket_debug_redacts_bearer_proof_material() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_123,
            client_nonce: [17; 32],
            solution: [34; 32],
        };

        let rendered = format!("{ticket:?}");
        assert!(rendered.contains("version: 1"));
        assert!(rendered.contains("difficulty: 7"));
        assert!(rendered.contains("expires_at: 1700000123"));
        assert_eq!(rendered.matches("[REDACTED]").count(), 2);
        assert!(!rendered.contains("17, 17, 17"));
        assert!(!rendered.contains("34, 34, 34"));
    }
    #[test]
    fn ticket_drop_path_zeroizes_bearer_proof_material() {
        assert!(std::mem::needs_drop::<Ticket>());
        let mut ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_123,
            client_nonce: [17; 32],
            solution: [34; 32],
        };

        ticket.zeroize_sensitive_fields();

        assert_eq!(ticket.version, 0);
        assert_eq!(ticket.difficulty, 0);
        assert_eq!(ticket.expires_at, 0);
        assert_eq!(ticket.client_nonce, [0; 32]);
        assert_eq!(ticket.solution, [0; 32]);
    }
    #[test]
    fn serialized_ticket_owner_redacts_and_scrubs() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_123,
            client_nonce: [17; 32],
            solution: [34; 32],
        };
        let mut bytes = ticket.to_bytes();
        assert!(std::mem::needs_drop::<TicketBytes>());
        assert!(bytes.iter().any(|byte| *byte != 0));
        assert_eq!(format!("{bytes:?}"), "TicketBytes(<redacted>)");
        bytes.clear();
        assert!(bytes.iter().all(|byte| *byte == 0));
    }
    #[test]
    fn signed_ticket_debug_redacts_every_bearer_binding() {
        let signed = SignedTicket {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 7,
                expires_at: 1_700_000_123,
                client_nonce: [17; 32],
                solution: [34; 32],
            },
            relay_id: [51; 32],
            transcript_hash: [68; 32],
            signature: vec![85; MlDsaSuite::MlDsa44.signature_len()],
        };

        let rendered = format!("{signed:?}");
        assert!(rendered.contains("SignedTicket"));
        assert_eq!(rendered.matches("[REDACTED]").count(), 5);
        for raw_prefix in [
            "17, 17, 17",
            "34, 34, 34",
            "51, 51, 51",
            "68, 68, 68",
            "85, 85, 85",
        ] {
            assert!(
                !rendered.contains(raw_prefix),
                "debug output exposed bearer material: {rendered}"
            );
        }
    }
    #[test]
    #[allow(unsafe_code)]
    fn signed_ticket_drop_path_zeroizes_owned_bearer_material() {
        assert!(std::mem::needs_drop::<SignedTicket>());
        let signature_len = MlDsaSuite::MlDsa44.signature_len();
        let mut signature = Vec::with_capacity(signature_len + 32);
        let signature_capacity = signature.capacity();
        signature.resize(signature_capacity, 85);
        signature.truncate(signature_len);
        let mut signed = SignedTicket {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 7,
                expires_at: 1_700_000_123,
                client_nonce: [17; 32],
                solution: [34; 32],
            },
            relay_id: [51; 32],
            transcript_hash: [68; 32],
            signature,
        };

        signed.zeroize_sensitive_fields();

        assert_eq!(signed.ticket.version, 0);
        assert_eq!(signed.ticket.difficulty, 0);
        assert_eq!(signed.ticket.expires_at, 0);
        assert_eq!(signed.ticket.client_nonce, [0; 32]);
        assert_eq!(signed.ticket.solution, [0; 32]);
        assert_eq!(signed.relay_id, [0; 32]);
        assert_eq!(signed.transcript_hash, [0; 32]);
        assert!(signed.signature.is_empty());
        assert_eq!(signed.signature.capacity(), signature_capacity);
        // SAFETY: `zeroize_sensitive_fields` initializes and wipes every byte
        // through the vector's capacity immediately before clearing its length.
        unsafe { signed.signature.set_len(signature_capacity) };
        assert!(signed.signature.iter().all(|byte| *byte == 0));
        signed.signature.clear();
    }
    #[test]
    fn signed_ticket_signing_payload_drop_path_zeroizes_staging_bytes() {
        assert!(std::mem::needs_drop::<SignedTicketPayload>());
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_123,
            client_nonce: [17; 32],
            solution: [34; 32],
        };
        let mut payload = SignedTicket::build_payload(&ticket, &[51; 32], &[68; 32]);
        assert!(payload.bytes.iter().any(|byte| *byte != 0));

        payload.zeroize_sensitive_fields();

        assert!(payload.bytes.iter().all(|byte| *byte == 0));
    }
    #[test]
    fn admission_transcript_commits_to_exact_client_hello() {
        let client_hello = b"\x01soranet-client-hello\x01resume-binding-a";
        let transcript = derive_admission_transcript(client_hello);
        let mut expected = Hasher::new();
        expected.update(ADMISSION_TRANSCRIPT_DOMAIN);
        let length = u64::try_from(client_hello.len()).expect("test hello length fits in u64");
        expected.update(&length.to_be_bytes());
        expected.update(client_hello);
        assert_eq!(transcript, *expected.finalize().as_bytes());
        assert_eq!(transcript, derive_admission_transcript(client_hello));
        let mut substituted = client_hello.to_vec();
        *substituted.last_mut().expect("non-empty client hello") ^= 1;
        assert_ne!(transcript, derive_admission_transcript(&substituted));
        assert_ne!(transcript, derive_admission_transcript(&[]));
    }
    #[test]
    fn ticket_binding_commitment_is_domain_separated_and_binds_every_field() {
        let descriptor = [0x11; 32];
        let relay = [0x22; 32];
        let transcript = [0x33; 32];
        let binding = ChallengeBinding::new(&descriptor, &relay, &transcript);
        let commitment = ticket_binding_commitment(
            binding.descriptor_commit,
            binding.relay_id,
            binding.transcript_hash,
        );

        let mut expected = Hasher::new();
        expected.update(TICKET_BINDING_DOMAIN);
        for field in [
            descriptor.as_slice(),
            relay.as_slice(),
            transcript.as_slice(),
        ] {
            expected.update(&(field.len() as u64).to_be_bytes());
            expected.update(field);
        }
        assert_eq!(commitment, *expected.finalize().as_bytes());

        let changed_descriptor = [0x12; 32];
        let changed_relay = [0x23; 32];
        let changed_transcript = [0x34; 32];
        assert_ne!(
            commitment,
            ticket_binding_commitment(&changed_descriptor, &relay, &transcript)
        );
        assert_ne!(
            commitment,
            ticket_binding_commitment(&descriptor, &changed_relay, &transcript)
        );
        assert_ne!(
            commitment,
            ticket_binding_commitment(&descriptor, &relay, &changed_transcript)
        );
    }
    #[test]
    fn challenge_hashes_match_canonical_contiguous_layout() {
        let descriptor = [0x11; 32];
        let relay = [0x22; 32];
        let transcript = [0x33; 32];
        let client_nonce = [0x44; 32];
        let expires_at = 1_700_000_123_u64;
        let binding = ChallengeBinding::new(&descriptor, &relay, &transcript);
        let mut expected_challenge = Vec::with_capacity(
            CHALLENGE_DOMAIN.len()
                + descriptor.len()
                + relay.len()
                + transcript.len()
                + client_nonce.len()
                + 8,
        );
        expected_challenge.extend_from_slice(CHALLENGE_DOMAIN);
        expected_challenge.extend_from_slice(&descriptor);
        expected_challenge.extend_from_slice(&relay);
        expected_challenge.extend_from_slice(&transcript);
        expected_challenge.extend_from_slice(&client_nonce);
        expected_challenge.extend_from_slice(&expires_at.to_be_bytes());
        assert_eq!(
            derive_challenge(&binding, &client_nonce, expires_at),
            blake3::hash(&expected_challenge)
        );
        let challenge = blake3::hash(b"challenge");
        let solution = [0x55; 32];
        let mut expected_solution =
            Vec::with_capacity(SOLUTION_DOMAIN.len() + challenge.as_bytes().len() + solution.len());
        expected_solution.extend_from_slice(SOLUTION_DOMAIN);
        expected_solution.extend_from_slice(challenge.as_bytes());
        expected_solution.extend_from_slice(&solution);
        assert_eq!(
            derive_solution_digest(&challenge, &solution),
            blake3::hash(&expected_solution)
        );
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at,
            client_nonce,
            solution,
        };
        let ticket_bytes = ticket.to_bytes();
        let mut expected_revocation_input =
            Vec::with_capacity(REVOCATION_DOMAIN.len() + ticket_bytes.len());
        expected_revocation_input.extend_from_slice(REVOCATION_DOMAIN);
        expected_revocation_input.extend_from_slice(&ticket_bytes);
        let expected_revocation: [u8; 32] = blake3::hash(&expected_revocation_input).into();
        assert_eq!(
            compute_ticket_revocation_fingerprint(&ticket_bytes),
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
        assert!(
            TicketRevocationStoreLimits::new(
                TICKET_REVOCATION_STORE_MAX_ENTRIES_V1,
                Duration::from_secs(1),
            )
            .is_ok(),
            "the exact first-release ceiling must be accepted"
        );
        assert_eq!(
            TicketRevocationStoreLimits::new(
                TICKET_REVOCATION_STORE_MAX_ENTRIES_V1 + 1,
                Duration::from_secs(1),
            )
            .expect_err("capacity above the first-release ceiling"),
            TicketRevocationStoreError::CapacityTooLarge {
                requested: TICKET_REVOCATION_STORE_MAX_ENTRIES_V1 + 1,
                limit: TICKET_REVOCATION_STORE_MAX_ENTRIES_V1,
            }
        );
        assert_eq!(
            TicketRevocationStore::load(
                PathBuf::new(),
                TicketRevocationStoreLimits::new(1, Duration::from_secs(1)).expect("limits"),
                UNIX_EPOCH,
            )
            .expect_err("empty persistent path"),
            TicketRevocationStoreError::PathEmpty
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
                assert_eq!(operation, "minting PoW solution nonce");
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
    fn mint_ticket_rejects_repeated_nonzero_rng_material() {
        let descriptor = [0xAB; 32];
        let binding = binding(&descriptor);
        let mut rng = FixedTryRng { byte: 0xA5 };
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let error = mint_ticket_with_clock_and_digest(
            &params(),
            &binding,
            Duration::from_secs(30),
            &mut rng,
            || now,
            |_, _| blake3::Hash::from_bytes([0xFF; 32]),
        )
        .expect_err("a stuck nonzero RNG must fail before repeated proof work");
        match error {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting PoW solution nonce");
                assert!(message.contains("all-identical-byte material"));
            }
            other => panic!("expected repeated nonce failure, got {other:?}"),
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
    fn mint_reanchors_each_pow_candidate_across_long_search() {
        let mut rng = rand::rngs::StdRng::from_seed([0x91; 32]);
        let params = Parameters::new(1, Duration::from_secs(600), Duration::from_secs(30));
        let descriptor = [0xA7; 32];
        let binding = binding(&descriptor);
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut clock_reads = 0_u64;
        let mut digest_trials = 0_u64;
        let ticket = mint_ticket_with_clock_and_digest(
            &params,
            &binding,
            Duration::from_secs(60),
            &mut rng,
            || {
                let read = clock_reads;
                clock_reads += 1;
                let candidate = read / 2;
                let offset = candidate * 10 + u64::from(read % 2 == 1);
                base + Duration::from_secs(offset)
            },
            |challenge, solution| {
                digest_trials += 1;
                if digest_trials <= 7 {
                    blake3::Hash::from_bytes([0xFF; 32])
                } else {
                    derive_solution_digest(challenge, solution)
                }
            },
        )
        .expect("failed search history must not consume the successful candidate's ttl");
        assert!(digest_trials >= 8, "seven forced failures must be retried");
        let successful_candidate = clock_reads / 2 - 1;
        let solved_at = base + Duration::from_secs(successful_candidate * 10 + 1);
        assert!(solved_at.duration_since(base).expect("ordered clock") >= Duration::from_secs(71));
        assert_eq!(ticket.expires_at, 1_700_000_060 + successful_candidate * 10);
        verify_at(&ticket, &binding, &params, solved_at)
            .expect("fresh successful PoW candidate must satisfy remaining-ttl policy");
    }
    #[test]
    fn mint_rejects_clock_regression_during_candidate_evaluation() {
        let mut rng = rand::rngs::StdRng::from_seed([0x92; 32]);
        let params = params();
        let descriptor = [0xA8; 32];
        let binding = binding(&descriptor);
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut clock = [base + Duration::from_secs(2), base + Duration::from_secs(1)].into_iter();
        let error =
            mint_ticket_with_clock(&params, &binding, Duration::from_secs(60), &mut rng, || {
                clock.next().expect("two clock reads are sufficient")
            })
            .expect_err("clock regression must fail closed");
        assert!(matches!(error, MintError::ClockMovedBackwards));
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
            transcript_hash: TRANSCRIPT,
            signature: vec![0xAA; MlDsaSuite::MlDsa44.signature_len()],
        };
        assert!(signed.checked_expires_at().is_none());
        assert_eq!(signed.expires_at(), UNIX_EPOCH);
    }
    #[test]
    fn verify_rejects_unrepresentable_expiry_before_solution_work() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let descriptor = [0xAA; 32];
        let binding = binding(&descriptor);
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: u64::MAX,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let err = verify_at(
            &ticket,
            &binding,
            &params,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("unrepresentable expiry must fail before challenge verification");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
    }
    #[test]
    fn signed_ticket_verifier_rejects_unrepresentable_expiry_before_signature_preflight() {
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(30));
        let descriptor = [0xCC; 32];
        let binding = binding(&descriptor);
        let signed = SignedTicket {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: params.difficulty(),
                expires_at: u64::MAX,
                client_nonce: [0xAA; 32],
                solution: [0xBB; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let err = verify_signed_ticket_at(
            &signed,
            &[],
            &binding,
            &params,
            None,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("expiry overflow must fail before signature or key preflight");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
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
    fn ticket_parse_rejects_unrepresentable_expiry() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: u64::MAX,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let err = Ticket::parse(&ticket.to_bytes())
            .expect_err("unrepresentable expiry must fail at ticket parse time");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
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
        let binding = ChallengeBinding::new(&short_descriptor, &RELAY_A, &TRANSCRIPT);
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
            transcript_hash: TRANSCRIPT,
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
        let binding = ChallengeBinding::new(&descriptor, &short_relay, &TRANSCRIPT);
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
        let binding = binding(&descriptor);
        let ticket = Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: base + params.min_ticket_ttl().as_secs(),
            client_nonce: ticket_binding_commitment(
                binding.descriptor_commit,
                binding.relay_id,
                binding.transcript_hash,
            ),
            solution: [0x55; 32],
        };
        let now = UNIX_EPOCH + Duration::from_secs(base + 1);
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
        let signed =
            SignedTicket::sign(ticket, &relay_id, &transcript, kp.secret_key()).expect("sign");
        signed.verify(kp.public_key()).expect("verify");
        let encoded = signed.encode();
        // Tamper with ticket
        let mut tampered = SignedTicket::decode(&encoded).expect("decode tamper fixture");
        tampered.ticket.difficulty = 0;
        tampered
            .verify(kp.public_key())
            .expect_err("tampered ticket");
        // Tamper with relay_id
        let mut tampered_relay = SignedTicket::decode(&encoded).expect("decode relay fixture");
        tampered_relay.relay_id[0] ^= 0xFF;
        tampered_relay
            .verify(kp.public_key())
            .expect_err("tampered relay");
    }
    #[test]
    fn signed_and_unsigned_forms_share_one_replay_identity() {
        use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 10_120,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let fingerprint = ticket.revocation_fingerprint();
        let ticket_bytes = ticket.to_vec();
        let signed_a = SignedTicket::sign(
            Ticket::parse(&ticket_bytes).expect("decode first presentation"),
            &RELAY_A,
            &TRANSCRIPT,
            keypair.secret_key(),
        )
        .expect("sign first presentation");
        let signed_b = SignedTicket::sign(
            Ticket::parse(&ticket_bytes).expect("decode second presentation"),
            &RELAY_A,
            &TRANSCRIPT,
            keypair.secret_key(),
        )
        .expect("re-sign same ticket");
        assert_ne!(
            signed_a.signature, signed_b.signature,
            "ML-DSA re-signing fixture must exercise distinct randomized signatures"
        );

        assert_eq!(signed_a.revocation_fingerprint(), fingerprint);
        assert_eq!(signed_b.revocation_fingerprint(), fingerprint);
        assert_eq!(
            signed_a.revocation_fingerprint(),
            signed_b.revocation_fingerprint(),
            "randomized re-signing must not mint a fresh replay identity"
        );

        let distinct = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 10_120,
            client_nonce: [0x11; 32],
            solution: [0x23; 32],
        };
        assert_ne!(fingerprint, distinct.revocation_fingerprint());

        let now = UNIX_EPOCH + Duration::from_secs(10_000);
        let limits =
            TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("replay limits");
        let mut signed_first = TicketRevocationStore::in_memory(limits).expect("store");
        assert_eq!(
            signed_first
                .revoke_ticket(&signed_a, now)
                .expect("consume signed form")
                .status,
            TicketRevocationInsertStatus::Accepted
        );
        assert_eq!(
            signed_first
                .revoke_ticket_payload(&ticket, now)
                .expect("consume unsigned form")
                .status,
            TicketRevocationInsertStatus::Duplicate
        );

        let mut unsigned_first = TicketRevocationStore::in_memory(limits).expect("store");
        assert_eq!(
            unsigned_first
                .revoke_ticket_payload(&ticket, now)
                .expect("consume unsigned form")
                .status,
            TicketRevocationInsertStatus::Accepted
        );
        assert_eq!(
            unsigned_first
                .revoke_ticket(&signed_b, now)
                .expect("consume re-signed form")
                .status,
            TicketRevocationInsertStatus::Duplicate
        );
    }
    #[test]
    fn signed_ticket_payload_matches_canonical_contiguous_layout() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 7,
            expires_at: 1_700_000_600,
            client_nonce: [0x11; 32],
            solution: [0x22; 32],
        };
        let relay_id = [0x33; 32];
        let transcript = [0x44; 32];
        let payload = SignedTicket::build_payload(&ticket, &relay_id, &transcript);
        let mut expected =
            Vec::with_capacity(SIGNING_DOMAIN.len() + TICKET_LEN + relay_id.len() + 32);
        expected.extend_from_slice(SIGNING_DOMAIN);
        expected.extend_from_slice(&ticket.to_bytes());
        expected.extend_from_slice(&relay_id);
        expected.extend_from_slice(&transcript);
        assert_eq!(payload.as_slice(), expected.as_slice());
        assert_eq!(payload.as_slice().len(), SIGNED_TICKET_PAYLOAD_LEN);
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
        let signed = SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, kp.secret_key())
            .expect("sign ticket");
        let encoded = signed.encode();
        let decoded = SignedTicket::decode(&encoded).expect("decode");
        assert_eq!(decoded, signed);
        let err = SignedTicket::decode(&[]).expect_err("empty payload should fail");
        assert!(matches!(err, Error::Malformed(_)));
    }
    #[test]
    fn signed_ticket_decode_rejects_trailing_and_oversized_input() {
        let ticket = signed_ticket_with_expiry(1_700_000_600, 0xA5);
        let mut trailing = ticket.encode();
        trailing.push(0);
        let error = SignedTicket::decode(&trailing)
            .expect_err("bare signed-ticket decoding must consume every byte");
        assert!(matches!(error, Error::Malformed(_)));

        let oversized = vec![0_u8; SIGNED_TICKET_MAX_ENCODED_BYTES_V1 + 1];
        let error = SignedTicket::decode(&oversized)
            .expect_err("oversized input must fail before Norito decoding");
        assert!(
            matches!(&error, Error::Malformed(message) if message.contains("first-release maximum")),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn signed_ticket_decode_rejects_payload_without_transcript() {
        #[derive(NoritoSerialize)]
        struct SignedTicketWithoutTranscript {
            ticket: Ticket,
            relay_id: [u8; 32],
            transcript_hash: Option<[u8; 32]>,
            signature: Vec<u8>,
        }
        let payload = SignedTicketWithoutTranscript {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 1,
                expires_at: 1_700_000_600,
                client_nonce: [0x11; 32],
                solution: [0x22; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: None,
            signature: vec![0x33; MlDsaSuite::MlDsa44.signature_len()],
        };
        let encoded = encode_adaptive(&payload);
        let err = SignedTicket::decode(&encoded)
            .expect_err("first-release signed tickets require a transcript field");
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
            transcript_hash: TRANSCRIPT,
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
            transcript_hash: TRANSCRIPT,
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
            transcript_hash: TRANSCRIPT,
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
    fn signed_ticket_decode_rejects_unrepresentable_expiry_before_signature_preflight() {
        let signed = SignedTicket {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 0,
                expires_at: u64::MAX,
                client_nonce: [0xAA; 32],
                solution: [0xBB; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let encoded = signed.encode();
        let err = SignedTicket::decode(&encoded)
            .expect_err("unrepresentable expiry must fail before signature preflight");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
    }
    #[test]
    fn signed_ticket_verify_rejects_unrepresentable_expiry_before_signature_preflight() {
        let signed = SignedTicket {
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 0,
                expires_at: u64::MAX,
                client_nonce: [0xAA; 32],
                solution: [0xBB; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let err = signed
            .verify(&[])
            .expect_err("unrepresentable expiry must fail before signature preflight");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
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
            transcript_hash: TRANSCRIPT,
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
            transcript_hash: TRANSCRIPT,
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
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let err = signed
            .verify(&[])
            .expect_err("unsupported version must fail before signature checks");
        assert!(matches!(err, Error::UnsupportedVersion(_)));
    }
    #[test]
    fn signed_ticket_sign_rejects_unsupported_version_before_secret_key_preflight() {
        let ticket = Ticket {
            version: Ticket::VERSION + 1,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let err = SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, &[])
            .expect_err("unsupported version must fail before secret-key validation");
        assert!(matches!(err, Error::UnsupportedVersion(_)));
    }
    #[test]
    fn signed_ticket_sign_rejects_unrepresentable_expiry_before_secret_key_preflight() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: u64::MAX,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let err = SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, &[])
            .expect_err("unrepresentable expiry must fail before secret-key validation");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
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
        let err = SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, &[])
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
    fn signed_ticket_sign_rejects_all_zero_secret_key_material_before_backend() {
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: 0,
            expires_at: 123,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let secret_key = vec![0u8; MlDsaSuite::MlDsa44.secret_key_len()];
        let err = SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, &secret_key)
            .expect_err("all-zero secret key must fail before signing");
        match err {
            Error::Signing(message) => {
                assert!(message.contains("all zero"), "unexpected error: {message}");
            }
            other => panic!("expected all-zero secret key error, got {other:?}"),
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
            transcript_hash: TRANSCRIPT,
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
    fn signed_ticket_verify_rejects_all_zero_public_key_material_before_backend() {
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
            transcript_hash: TRANSCRIPT,
            signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len()],
        };
        let all_zero_public_key = vec![0u8; MlDsaSuite::MlDsa44.public_key_len()];
        let err = signed
            .verify(&all_zero_public_key)
            .expect_err("all-zero public key must fail before backend verification");
        match err {
            Error::PostQuantum(message) => {
                assert!(message.contains("all zero"), "unexpected error: {message}");
            }
            other => panic!("expected all-zero public key error, got {other:?}"),
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
            transcript_hash: TRANSCRIPT,
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
            transcript_hash: [0x11; 32],
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let expected_transcript = [0x22; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &expected_transcript);
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
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &TRANSCRIPT);
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
            transcript_hash: TRANSCRIPT,
            signature: Vec::new(),
        };
        let descriptor = [0xCC; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &TRANSCRIPT);
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
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng).expect("mint");
        verify(&ticket, &binding, &params).expect("verify with original binding");
        let error = verify(&ticket, &other, &params)
            .expect_err("an arbitrary mismatched relay must fail exactly");
        assert!(matches!(error, Error::InvalidSolution));
    }
    #[test]
    fn rejects_mismatched_transcript_hash() {
        let params = Parameters::new(8, Duration::from_secs(300), Duration::from_secs(45));
        let mut rng = rand::rngs::StdRng::from_seed([0x12; 32]);
        let descriptor = [0xAC; 32];
        let transcript_a = [0x01; 32];
        let transcript_b = [0x02; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &transcript_a);
        let mismatched = ChallengeBinding::new(&descriptor, &RELAY_A, &transcript_b);
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng).expect("mint");
        verify(&ticket, &binding, &params).expect("expected transcript to verify");
        let error = verify(&ticket, &mismatched, &params)
            .expect_err("an arbitrary mismatched transcript must fail exactly");
        assert!(matches!(error, Error::InvalidSolution));
    }
    #[test]
    fn revocation_store_materializes_empty_ledger_on_load() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nested/revocations.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("create ledger");
        assert_eq!(store.len(now).expect("len"), 0);
        assert!(
            std::fs::metadata(&path).expect("ledger metadata").len() > 0,
            "startup must materialize a parseable durable snapshot"
        );
        drop(store);
        let mut reloaded = TicketRevocationStore::load(&path, limits, now).expect("reload ledger");
        assert_eq!(reloaded.len(now).expect("len"), 0);
    }
    #[test]
    fn revocation_store_high_watermark_prevents_clock_rollback_replay() {
        let initial = UNIX_EPOCH + Duration::from_secs(1_000);
        let expiry = UNIX_EPOCH + Duration::from_secs(1_100);
        let rollback = UNIX_EPOCH + Duration::from_secs(1_050);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("clock-rollback.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let ticket = signed_ticket_with_expiry(1_100, 0xA5);
        {
            let mut store = TicketRevocationStore::load(&path, limits, initial).expect("load");
            assert_eq!(
                store
                    .revoke_ticket(&ticket, initial)
                    .expect("initial revocation")
                    .status,
                TicketRevocationInsertStatus::Accepted
            );
            assert_eq!(store.purge_expired(expiry).expect("purge at expiry"), 1);
            assert_eq!(
                store
                    .revoke_ticket(&ticket, rollback)
                    .expect("rollback insertion status")
                    .status,
                TicketRevocationInsertStatus::Expired,
                "a regressed clock must not reopen a consumed ticket"
            );
        }
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, rollback).expect("rollback reload");
        assert_eq!(
            reloaded
                .revoke_ticket(&ticket, rollback)
                .expect("restart rollback insertion status")
                .status,
            TicketRevocationInsertStatus::Expired,
            "the durable high-water mark must survive restart"
        );
    }
    #[test]
    fn revocation_queries_do_not_write_and_rollback_fails_closed() {
        let initial = UNIX_EPOCH + Duration::from_secs(1_000);
        let expiry = UNIX_EPOCH + Duration::from_secs(1_100);
        let rollback = UNIX_EPOCH + Duration::from_secs(1_050);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("query-clock-high-water.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let ticket = signed_ticket_with_expiry(1_100, 0xA5);
        {
            let mut store = TicketRevocationStore::load(&path, limits, initial).expect("load");
            store
                .revoke_ticket(&ticket, initial)
                .expect("initial revocation");
            let snapshot_before_query = std::fs::read(&path).expect("read snapshot before query");
            assert!(
                !store
                    .is_ticket_revoked(&ticket, expiry)
                    .expect("expiry query")
            );
            assert_eq!(store.len(expiry).expect("active length at expiry"), 0);
            assert!(
                store
                    .active_fingerprints(expiry)
                    .expect("active fingerprints at expiry")
                    .is_empty()
            );
            assert_eq!(
                std::fs::read(&path).expect("read snapshot after query"),
                snapshot_before_query,
                "read-only queries must not force an atomic snapshot rewrite"
            );
        }
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, rollback).expect("rollback reload");
        assert!(
            reloaded
                .is_ticket_revoked(&ticket, rollback)
                .expect("rollback query"),
            "the retained revocation must become active again after rollback"
        );
        assert_eq!(
            reloaded
                .revoke_ticket(&ticket, rollback)
                .expect("rollback revocation")
                .status,
            TicketRevocationInsertStatus::Duplicate
        );
    }
    #[test]
    fn revocation_store_durably_observes_rejected_insert_time() {
        let initial = UNIX_EPOCH + Duration::from_secs(1_000);
        let observed = UNIX_EPOCH + Duration::from_secs(1_200);
        let rollback = UNIX_EPOCH + Duration::from_secs(1_050);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("rejected-clock-high-water.norito");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let ticket = signed_ticket_with_expiry(1_200, 0xA5);
        {
            let mut store = TicketRevocationStore::load(&path, limits, initial).expect("load");
            assert_eq!(
                store
                    .revoke_ticket(&ticket, observed)
                    .expect("expired revocation")
                    .status,
                TicketRevocationInsertStatus::Expired
            );
        }
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, rollback).expect("rollback reload");
        assert_eq!(
            reloaded
                .revoke_ticket(&ticket, rollback)
                .expect("rollback revocation")
                .status,
            TicketRevocationInsertStatus::Expired
        );
    }
    #[test]
    fn revocation_store_retries_dirty_snapshot_after_failed_write() {
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let ticket = signed_ticket_with_expiry(1_120, 0xA5);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("dirty-retry.norito");
        let mut store = TicketRevocationStore::load(&path, limits, now).expect("load");
        std::fs::remove_file(&path).expect("remove initial snapshot");
        std::fs::create_dir(&path).expect("block snapshot destination");
        let error = store
            .revoke_ticket(&ticket, now)
            .expect_err("the first snapshot write must fail");
        assert!(matches!(error, TicketRevocationStoreError::Io(_)));
        std::fs::remove_dir(&path).expect("remove snapshot blocker");

        assert!(
            store
                .is_ticket_revoked(&ticket, now)
                .expect("a later query must retry the dirty snapshot")
        );
        drop(store);

        let mut reloaded = TicketRevocationStore::load(&path, limits, now).expect("reload");
        assert!(
            reloaded
                .is_ticket_revoked(&ticket, now)
                .expect("failed-write revocation must survive restart after retry")
        );
    }
    #[test]
    fn revocation_store_preserves_subsecond_expiry_and_high_watermark() {
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let initial = UNIX_EPOCH + Duration::new(1_000, 100_000_000);
        let expiry = UNIX_EPOCH + Duration::new(1_100, 500_000_000);
        let before_expiry = UNIX_EPOCH + Duration::new(1_100, 250_000_000);
        let ticket = signed_ticket_with_expiry(1_101, 0xA5);
        let fingerprint = ticket.revocation_fingerprint();
        let dir = tempdir().expect("tempdir");
        let expiry_path = dir.path().join("subsecond-expiry.norito");
        {
            let mut store =
                TicketRevocationStore::load(&expiry_path, limits, initial).expect("load");
            assert_eq!(
                store
                    .insert(fingerprint, expiry, initial)
                    .expect("revoke ticket fingerprint")
                    .status,
                TicketRevocationInsertStatus::Accepted
            );
        }
        let mut reloaded = TicketRevocationStore::load(&expiry_path, limits, before_expiry)
            .expect("reload before subsecond expiry");
        assert!(
            reloaded
                .is_revoked_fingerprint(&fingerprint, before_expiry)
                .expect("ticket remains revoked")
        );
        assert_eq!(
            reloaded
                .insert(fingerprint, expiry, before_expiry)
                .expect("duplicate revocation")
                .status,
            TicketRevocationInsertStatus::Duplicate
        );
        drop(reloaded);

        let high_water_path = dir.path().join("subsecond-high-water.norito");
        let observed = UNIX_EPOCH + Duration::new(1_200, 500_000_000);
        let rollback = UNIX_EPOCH + Duration::new(1_200, 250_000_000);
        let rollback_expiry = UNIX_EPOCH + Duration::new(1_200, 400_000_000);
        let rollback_ticket = signed_ticket_with_expiry(1_201, 0x5A);
        let rollback_fingerprint = rollback_ticket.revocation_fingerprint();
        {
            let mut store =
                TicketRevocationStore::load(&high_water_path, limits, initial).expect("load");
            assert_eq!(
                store
                    .insert(fingerprint, observed, observed)
                    .expect("expired observation")
                    .status,
                TicketRevocationInsertStatus::Expired
            );
        }
        let mut reloaded = TicketRevocationStore::load(&high_water_path, limits, rollback)
            .expect("reload after subsecond rollback");
        assert_eq!(
            reloaded
                .insert(rollback_fingerprint, rollback_expiry, rollback)
                .expect("rollback revocation")
                .status,
            TicketRevocationInsertStatus::Expired
        );
    }
    #[test]
    fn revocation_store_fails_closed_without_evicting_active_records() {
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
        assert_eq!(
            outcome_c.status,
            TicketRevocationInsertStatus::Capacity,
            "a full store must reject new consumption records"
        );
        assert!(store.is_ticket_revoked(&ticket_a, now).expect("ticket a"));
        assert!(store.is_ticket_revoked(&ticket_b, now).expect("ticket b"));
        assert!(!store.is_ticket_revoked(&ticket_c, now).expect("ticket c"));
        let mut active = store
            .active_fingerprints(now)
            .expect("collect bounded active fingerprints");
        active.sort_unstable();
        let mut expected = vec![
            ticket_a.revocation_fingerprint(),
            ticket_b.revocation_fingerprint(),
        ];
        expected.sort_unstable();
        assert_eq!(active, expected);
        let reload_now = UNIX_EPOCH + Duration::from_secs(1_250);
        drop(store);
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, reload_now).expect("reload from disk");
        assert_eq!(
            reloaded.len(reload_now).expect("reloaded len"),
            0,
            "expired entries must be pruned on load"
        );
    }
    #[test]
    fn revocation_store_load_rejects_over_capacity_snapshot() {
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
                    expires_at_nanos: 0,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint: middle,
                    expires_at_secs: 2_160,
                    expires_at_nanos: 0,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint: newest,
                    expires_at_secs: 2_220,
                    expires_at_nanos: 0,
                },
            ],
        );
        let err =
            TicketRevocationStore::load(&path, limits, now).expect_err("snapshot must fail closed");
        assert_eq!(
            err,
            TicketRevocationStoreError::Parse("revocation snapshot exceeds capacity".to_owned())
        );
    }
    #[test]
    fn revocation_store_load_rejects_empty_snapshot() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        write_private_test_file(&path, b"");
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let err =
            TicketRevocationStore::load(&path, limits, now).expect_err("snapshot must fail closed");
        assert_eq!(
            err,
            TicketRevocationStoreError::Parse("revocation snapshot is empty".to_owned())
        );
    }
    #[test]
    fn revocation_store_rejects_noncanonical_snapshot_nanoseconds() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("noncanonical-nanoseconds.norito");
        let snapshot = TicketRevocationSnapshot {
            version: REVOCATION_SNAPSHOT_VERSION_V1,
            high_watermark_secs: 2_000,
            high_watermark_nanos: 1_000_000_000,
            entries: Vec::new(),
        };
        write_private_test_file(&path, &to_bytes(&snapshot).expect("encode snapshot"));
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(300)).expect("limits");
        let error = TicketRevocationStore::load(&path, limits, now)
            .expect_err("noncanonical nanoseconds must fail closed");
        assert!(matches!(
            error,
            TicketRevocationStoreError::Parse(message)
                if message.contains("noncanonical nanoseconds")
        ));
    }
    #[test]
    fn revocation_store_load_rejects_active_entry_beyond_ttl() {
        let now = UNIX_EPOCH + Duration::from_secs(2_000);
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        write_revocation_snapshot(
            &path,
            vec![TicketRevocationSnapshotEntry {
                fingerprint: [0x31; 32],
                expires_at_secs: 2_120,
                expires_at_nanos: 0,
            }],
        );
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(60)).expect("limits");
        let err =
            TicketRevocationStore::load(&path, limits, now).expect_err("snapshot must fail closed");
        assert!(
            matches!(err, TicketRevocationStoreError::Parse(message) if message.contains("max_ttl"))
        );
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
                    expires_at_nanos: 0,
                },
                TicketRevocationSnapshotEntry {
                    fingerprint,
                    expires_at_secs: 2_160,
                    expires_at_nanos: 0,
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
                expires_at_nanos: 0,
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
        assert_eq!(store.len(now).expect("len"), 0);
        let expired = store.revoke_ticket(&overdue, now).expect("insert overdue");
        assert_eq!(expired.status, TicketRevocationInsertStatus::Expired);
        assert_eq!(store.len(now).expect("len"), 0);
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
            ticket: Ticket {
                version: Ticket::VERSION,
                difficulty: 0,
                expires_at: u64::MAX,
                client_nonce: [0u8; 32],
                solution: [0u8; 32],
            },
            relay_id: RELAY_A,
            transcript_hash: TRANSCRIPT,
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
        assert_eq!(store.len(now).expect("len"), 0);
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
        assert!(store.is_ticket_revoked(&long, later).expect("long ticket"));
        assert!(
            !store
                .is_ticket_revoked(&short, later)
                .expect("short ticket")
        );
        drop(store);
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, later).expect("reload after purge");
        assert_eq!(reloaded.len(later).expect("reloaded len"), 1);
        assert!(
            reloaded
                .is_ticket_revoked(&long, later)
                .expect("reloaded long ticket")
        );
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
            client_nonce: ticket_binding_commitment(
                binding.descriptor_commit,
                binding.relay_id,
                binding.transcript_hash,
            ),
            solution: [0x20; 32],
        };
        verify_with_revocations_at(&ticket, &binding, &params, Some(&mut store), now)
            .expect("first verification");
        let err = verify_with_revocations_at(&ticket, &binding, &params, Some(&mut store), now)
            .expect_err("replay should be rejected");
        matches!(err, Error::Replay);
        let later = now + Duration::from_secs(30);
        drop(store);
        let mut reloaded =
            TicketRevocationStore::load(&path, limits, later).expect("reload revocations");
        let err =
            verify_with_revocations_at(&ticket, &binding, &params, Some(&mut reloaded), later)
                .expect_err("replay should persist");
        matches!(err, Error::Replay);
    }
    #[test]
    fn revocation_store_capacity_never_makes_consumed_ticket_replayable() {
        let now = UNIX_EPOCH + Duration::from_secs(8_000);
        let limits = TicketRevocationStoreLimits::new(1, Duration::from_secs(300)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");
        let params = Parameters::new(0, Duration::from_secs(600), Duration::from_secs(60));
        let descriptor = [0xBC; 32];
        let binding = binding(&descriptor);
        let client_nonce = ticket_binding_commitment(
            binding.descriptor_commit,
            binding.relay_id,
            binding.transcript_hash,
        );
        let mk_ticket = |expires_at_secs: u64, nonce: u8| Ticket {
            version: 1,
            difficulty: params.difficulty(),
            expires_at: expires_at_secs,
            client_nonce,
            solution: [nonce.wrapping_add(1); 32],
        };
        let ticket_a = mk_ticket(8_120, 0x01);
        let ticket_b = mk_ticket(8_140, 0x02);
        verify_with_revocations_at(&ticket_a, &binding, &params, Some(&mut store), now)
            .expect("accept ticket a");
        let capacity_err =
            verify_with_revocations_at(&ticket_b, &binding, &params, Some(&mut store), now)
                .expect_err("full replay store must reject ticket b");
        assert!(matches!(capacity_err, Error::RevocationStore(_)));
        assert!(
            store
                .is_ticket_payload_revoked(&ticket_a, now)
                .expect("ticket a query"),
            "accepted ticket must remain consumed"
        );
        assert!(
            !store
                .is_ticket_payload_revoked(&ticket_b, now)
                .expect("ticket b query"),
            "rejected ticket must not displace active consumption state"
        );
        let replay_err =
            verify_with_revocations_at(&ticket_a, &binding, &params, Some(&mut store), now)
                .expect_err("ticket a must remain a replay");
        assert!(matches!(replay_err, Error::Replay));
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
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &TRANSCRIPT);
        let expires_at = now.duration_since(UNIX_EPOCH).expect("epoch").as_secs() + 120;
        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at,
            client_nonce: ticket_binding_commitment(
                binding.descriptor_commit,
                binding.relay_id,
                binding.transcript_hash,
            ),
            solution: [0u8; 32],
        };
        let signed =
            SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, keypair.secret_key()).expect("sign");
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
        drop(store);
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
    fn known_signed_ticket_replay_is_rejected_before_public_key_crypto() {
        let now = UNIX_EPOCH + Duration::from_secs(43_000);
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(600)).expect("limits");
        let mut store = TicketRevocationStore::in_memory(limits).expect("store");
        let params = Parameters::new(0, Duration::from_secs(900), Duration::from_secs(60));
        let descriptor = [0xAB; 32];
        let binding = ChallengeBinding::new(&descriptor, &RELAY_A, &TRANSCRIPT);
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: now.duration_since(UNIX_EPOCH).expect("epoch").as_secs() + 120,
            client_nonce: ticket_binding_commitment(
                binding.descriptor_commit,
                binding.relay_id,
                binding.transcript_hash,
            ),
            solution: [0; 32],
        };
        assert_eq!(
            store
                .revoke_ticket_payload(&ticket, now)
                .expect("preload canonical replay")
                .status,
            TicketRevocationInsertStatus::Accepted
        );
        let replay = SignedTicket {
            ticket,
            relay_id: RELAY_A,
            transcript_hash: TRANSCRIPT,
            // Structurally valid, deliberately unauthenticated material. If
            // ML-DSA/key validation runs first, the empty public key below
            // produces a different error and this regression fails.
            signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len()],
        };

        let error = verify_signed_ticket_at(&replay, &[], &binding, &params, Some(&mut store), now)
            .expect_err("known replay must be rejected before public-key work");
        assert!(matches!(error, Error::Replay));
    }
    #[test]
    fn revocation_store_rejects_concurrent_ledger_owner() {
        let now = UNIX_EPOCH + Duration::from_secs(60_000);
        let limits = TicketRevocationStoreLimits::new(2, Duration::from_secs(120)).expect("limits");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("single_owner.norito");
        let owner = TicketRevocationStore::load(&path, limits, now).expect("first owner");
        let error = TicketRevocationStore::load(&path, limits, now)
            .expect_err("a second ledger owner must fail closed");
        assert!(
            matches!(&error, TicketRevocationStoreError::Io(message) if message.contains("exclusive replay-ledger lock")),
            "unexpected concurrent-owner error: {error:?}"
        );
        drop(owner);
        TicketRevocationStore::load(&path, limits, now).expect("lock released with owner");
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
            SignedTicket::sign(ticket, &RELAY_A, &TRANSCRIPT, keypair.secret_key()).expect("sign");
        let mismatched = ChallengeBinding::new(&descriptor, &RELAY_B, &TRANSCRIPT);
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
