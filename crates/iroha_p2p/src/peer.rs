//! Tokio actor Peer

use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

use bytes::{Buf, BufMut, BytesMut};
#[cfg(not(feature = "noise_handshake"))]
use iroha_crypto::SessionKey;
#[cfg(feature = "noise_handshake")]
use iroha_crypto::blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_crypto::soranet::pow::TicketRevocationStoreLimits;
use iroha_crypto::soranet::{
    handshake::{
        HarnessError, RuntimeParams, build_client_hello, client_handle_relay_hello,
        process_client_hello, relay_finalize_handshake,
    },
    pow::{
        self, ChallengeBinding as PowBinding, Parameters as PowParameters, SignedTicket,
        Ticket as PowTicket, TicketRevocationStore,
    },
    puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
};
use message::*;
use norito::{
    codec::{Decode, DecodeAll, Encode},
    core as ncore,
};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
#[cfg(feature = "noise_handshake")]
use snow::{Builder, params::NoiseParams};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::Duration,
};

use crate::{ConsensusConfigCaps, ConsensusHandshakeCaps, Error, RelayRole, boilerplate::*};
// (keep fully-qualified uses inline; avoid unused import warnings)

/// Max length of a handshake message in bytes excluding the length prefix.
///
/// Previously this value was limited to `u8::MAX` which proved insufficient once
/// additional metadata (such as the peer's public address) was included in the
/// payload, causing handshake messages to exceed 255 bytes and therefore
/// failing to decrypt on the receiving side.  The length prefix is now encoded
/// as a `u16`, allowing messages up to `u16::MAX` bytes.
pub const MAX_HANDSHAKE_LENGTH: u16 = u16::MAX;
/// Default associated data for AEAD
/// [`Authenticated encryption`](https://en.wikipedia.org/wiki/Authenticated_encryption)
pub const DEFAULT_AAD: &[u8; 10] = b"Iroha2 AAD";

/// Default capacity for peer I/O buffers.
///
/// A small benchmarking utility compared buffer sizes from 256 bytes to
/// 8 KiB and measured how many messages could be cycled through per
/// second. A 1 KiB buffer reached ≈25 million messages per second while
/// larger capacities didn't improve throughput but doubled memory usage.
/// Therefore 1 KiB is chosen as a balanced default.
pub const DEFAULT_BUFFER_CAPACITY: usize = 1024;
/// Upper bound for preallocating per-connection message buffers to reduce growth.
const DEFAULT_MESSAGE_PREALLOC_CAP: usize = 512 * 1024;
/// Prefix byte used to indicate a versioned handshake hello payload.
const HANDSHAKE_HELLO_VERSION_PREFIX: u8 = 0xFF;
/// Single supported handshake hello payload version.
const HANDSHAKE_HELLO_VERSION: u8 = 1;

/// Count of handshake failures (timeout or verification error).
static HANDSHAKE_FAILURES: AtomicU64 = AtomicU64::new(0);
// Handshake error taxonomy counters
static HSE_TIMEOUT: AtomicU64 = AtomicU64::new(0);
static HSE_PREFACE: AtomicU64 = AtomicU64::new(0);
static HSE_VERIFY: AtomicU64 = AtomicU64::new(0);
static HSE_DECRYPT: AtomicU64 = AtomicU64::new(0);
static HSE_CODEC: AtomicU64 = AtomicU64::new(0);
static HSE_IO: AtomicU64 = AtomicU64::new(0);
static HSE_OTHER: AtomicU64 = AtomicU64::new(0);
static MALFORMED_PAYLOAD_FRAMES: AtomicU64 = AtomicU64::new(0);

// Handshake latency histogram buckets (ms)
const HN: usize = 12;
static HANDSHAKE_BUCKETS_MS: [u64; HN] = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2000, 5000];
static HANDSHAKE_BUCKET_COUNTS: [AtomicU64; HN] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static HANDSHAKE_MS_SUM: AtomicU64 = AtomicU64::new(0);
static HANDSHAKE_MS_COUNT: AtomicU64 = AtomicU64::new(0);

/// Runtime configuration shared across `SoraNet` handshake attempts.
#[derive(Debug, Clone)]
pub struct SoranetHandshakeConfig {
    descriptor_commit: Arc<Vec<u8>>,
    relay_id: Arc<Vec<u8>>,
    client_capabilities: Arc<Vec<u8>>,
    relay_capabilities: Arc<Vec<u8>>,
    trust_gossip: bool,
    kem_id: u8,
    sig_id: u8,
    resume_hash: Option<Arc<Vec<u8>>>,
    pow_required: bool,
    pow_params: Arc<PowParameters>,
    pow_ticket_ttl: Duration,
    puzzle_params: Option<Arc<PuzzleParameters>>,
    signed_ticket_public_key: Option<Arc<Vec<u8>>>,
    admission_token: Option<Arc<Vec<u8>>>,
    revocation_store: Option<Arc<Mutex<TicketRevocationStore>>>,
    revocation_store_error: Option<Arc<str>>,
}

impl SoranetHandshakeConfig {
    pub(crate) fn new(
        descriptor_commit: Vec<u8>,
        client_capabilities: Vec<u8>,
        relay_capabilities: Vec<u8>,
        trust_gossip: bool,
        kem_id: u8,
        sig_id: u8,
        resume_hash: Option<Vec<u8>>,
        pow_required: bool,
        pow_params: PowParameters,
        puzzle_params: Option<PuzzleParameters>,
        pow_ticket_ttl: Duration,
        signed_ticket_public_key: Option<Vec<u8>>,
        revocation_store: Option<Arc<Mutex<TicketRevocationStore>>>,
        revocation_store_error: Option<String>,
    ) -> Self {
        let kem_id = match kem_id {
            1 | 2 => kem_id,
            other => {
                iroha_logger::warn!(
                    kem_id = other,
                    "unsupported ML-KEM identifier; defaulting to ML-KEM-768 (1)"
                );
                1
            }
        };
        let sig_id = match sig_id {
            1 => sig_id,
            other => {
                iroha_logger::warn!(
                    sig_id = other,
                    "unsupported signature suite; defaulting to Dilithium3 (1)"
                );
                1
            }
        };
        Self {
            relay_id: Arc::new(descriptor_commit.clone()),
            descriptor_commit: Arc::new(descriptor_commit),
            client_capabilities: Arc::new(client_capabilities),
            relay_capabilities: Arc::new(relay_capabilities),
            trust_gossip,
            kem_id,
            sig_id,
            resume_hash: resume_hash.map(Arc::new),
            pow_required,
            pow_params: Arc::new(pow_params),
            pow_ticket_ttl,
            puzzle_params: puzzle_params.map(Arc::new),
            signed_ticket_public_key: signed_ticket_public_key.map(Arc::new),
            admission_token: None,
            revocation_store,
            revocation_store_error: revocation_store_error.map(Arc::from),
        }
    }

    fn effective_ticket_ttl(&self) -> Duration {
        self.pow_ticket_ttl
            .min(self.pow_params.max_future_skew())
            .max(self.pow_params.min_ticket_ttl())
    }

    fn pow_binding(&self) -> PowBinding<'_> {
        PowBinding::new(
            self.descriptor_commit.as_slice(),
            self.relay_id.as_slice(),
            self.resume_hash
                .as_ref()
                .map(|value| value.as_ref().as_slice()),
        )
    }

    fn puzzle_binding(&self) -> PuzzleBinding<'_> {
        PuzzleBinding::new(
            self.descriptor_commit.as_slice(),
            self.relay_id.as_slice(),
            self.resume_hash
                .as_ref()
                .map(|value| value.as_ref().as_slice()),
        )
    }

    fn enforce_revocation(&self, ticket: &PowTicket) -> Result<(), ChallengeVerifyError> {
        let inc_revocation_metric = |reason: &str| {
            if let Some(metrics) = iroha_telemetry::metrics::global() {
                metrics.inc_soranet_pow_revocation_store(reason);
            }
        };
        if let Some(error) = self.revocation_store_error.as_ref() {
            iroha_logger::error!(
                error = %error,
                "soranet pow revocation store unavailable; rejecting ticket"
            );
            inc_revocation_metric("unavailable");
            return Err(ChallengeVerifyError::RevocationStore(error.to_string()));
        }
        let Some(store) = self.revocation_store.as_ref() else {
            return Ok(());
        };
        let now = SystemTime::now();
        let mut guard = store.lock().map_err(|_| {
            iroha_logger::error!("soranet pow revocation store lock poisoned");
            inc_revocation_metric("lock_poisoned");
            ChallengeVerifyError::RevocationStore("lock_poisoned".to_string())
        })?;
        pow::record_revocation(ticket, Some(&mut guard), now).map_err(|err| match err {
            pow::Error::Replay => ChallengeVerifyError::Replay,
            pow::Error::RevocationStore(message) => {
                iroha_logger::warn!(error = %message, "soranet pow revocation store error");
                inc_revocation_metric(&message);
                ChallengeVerifyError::RevocationStore(message)
            }
            other => ChallengeVerifyError::Pow(other),
        })
    }

    fn admission_for_difficulty(&self, difficulty: u8) -> ChallengeAdmission {
        let pow = self.pow_params.with_difficulty(difficulty);
        let puzzle = self
            .puzzle_params
            .as_ref()
            .map(|params| params.with_difficulty(difficulty));
        ChallengeAdmission {
            pow,
            ticket_ttl: self.effective_ticket_ttl(),
            puzzle,
        }
    }

    /// Whether this peer participates in trust gossip exchange.
    pub fn trust_gossip(&self) -> bool {
        self.trust_gossip
    }

    #[cfg(test)]
    pub(crate) fn defaults() -> Self {
        Self::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            false,
            PowParameters::new(0, Duration::from_secs(300), Duration::from_secs(30)),
            None,
            Duration::from_secs(60),
            None,
            None,
            None,
        )
    }

    pub(crate) fn runtime_params(&self) -> RuntimeParams<'_> {
        RuntimeParams {
            descriptor_commit: self.descriptor_commit.as_slice(),
            client_capabilities: self.client_capabilities.as_slice(),
            relay_capabilities: self.relay_capabilities.as_slice(),
            kem_id: self.kem_id,
            sig_id: self.sig_id,
            resume_hash: self
                .resume_hash
                .as_ref()
                .map(|value| value.as_ref().as_slice()),
        }
    }

    pub(crate) fn pow_required(&self) -> bool {
        if self.admission_token.is_some() {
            return false;
        }
        self.pow_required && (self.pow_params.difficulty() > 0 || self.puzzle_params.is_some())
    }

    /// Removes expired revocations from the backing store and returns the number of entries purged.
    #[allow(dead_code)]
    pub(crate) fn purge_expired_revocations(&self) -> Result<usize, ChallengeVerifyError> {
        if let Some(error) = self.revocation_store_error.as_ref() {
            iroha_logger::error!(
                error = %error,
                "soranet pow revocation store unavailable; purge skipped"
            );
            return Err(ChallengeVerifyError::RevocationStore(error.to_string()));
        }
        let Some(store) = self.revocation_store.as_ref() else {
            return Ok(0);
        };
        let mut guard = store.lock().map_err(|_| {
            iroha_logger::error!("soranet pow revocation store lock poisoned");
            ChallengeVerifyError::RevocationStore("lock_poisoned".to_string())
        })?;
        guard.purge_expired(SystemTime::now()).map_err(|err| {
            iroha_logger::error!(
                error = %err,
                "failed to purge soranet pow revocation store"
            );
            ChallengeVerifyError::RevocationStore(err.to_string())
        })
    }

    /// Returns the number of active revocation fingerprints currently tracked.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn active_revocations(&self) -> usize {
        if self.revocation_store_error.is_some() {
            return 0;
        }
        let Some(store) = self.revocation_store.as_ref() else {
            return 0;
        };
        let Ok(guard) = store.lock() else {
            return 0;
        };
        guard.len(SystemTime::now())
    }

    /// Attach an admission token to the handshake configuration.
    pub fn set_admission_token(&mut self, token: Vec<u8>) {
        self.admission_token = Some(Arc::new(token));
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn pow_parameters(&self) -> PowParameters {
        *self.pow_params
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn pow_ticket_ttl(&self) -> Duration {
        self.effective_ticket_ttl()
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn puzzle_parameters(&self) -> Option<PuzzleParameters> {
        self.puzzle_params.as_ref().map(|params| **params)
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn admission_summary(&self) -> Option<ChallengeAdmission> {
        if !self.pow_required() {
            return None;
        }
        Some(self.admission_for_difficulty(self.pow_params.difficulty()))
    }

    pub(crate) fn mint_challenge_ticket<R: RngCore + CryptoRng>(
        &self,
        rng: &mut R,
    ) -> Result<Option<MintedChallenge>, ChallengeMintError> {
        if let Some(token) = self.admission_token.as_ref() {
            let mut frames = Vec::with_capacity(1);
            frames.push(token.as_ref().clone());
            return Ok(Some(MintedChallenge {
                frames,
                ticket: None,
                admission: None,
            }));
        }
        if !self.pow_required() {
            return Ok(None);
        }
        let ttl = self.effective_ticket_ttl();
        let ticket = if let Some(params) = self.puzzle_params.as_ref() {
            let binding = self.puzzle_binding();
            puzzle::mint_ticket(params.as_ref(), &binding, ttl, rng)
                .map_err(ChallengeMintError::Puzzle)?
        } else {
            let binding = self.pow_binding();
            pow::mint_ticket(self.pow_params.as_ref(), &binding, ttl, rng)
                .map_err(ChallengeMintError::Pow)?
        };
        let admission = self.admission_for_difficulty(ticket.difficulty);
        let ticket_bytes = ticket.to_vec();
        Ok(Some(MintedChallenge {
            frames: vec![ticket_bytes.clone()],
            ticket: Some(ticket_bytes),
            admission: Some(admission),
        }))
    }

    pub(crate) fn verify_challenge_ticket(
        &self,
        bytes: &[u8],
    ) -> Result<Option<ChallengeAdmission>, ChallengeVerifyError> {
        if !self.pow_required() {
            return Ok(None);
        }
        if let Some(public_key) = self.signed_ticket_public_key.as_deref() {
            match SignedTicket::decode(bytes) {
                Ok(signed) => {
                    return self.verify_signed_ticket_decoded(&signed, public_key);
                }
                Err(pow::Error::Malformed(_)) if bytes.len() == pow::TICKET_LEN => {
                    // Fallback to unsigned tickets when the payload matches the raw PoW layout.
                }
                Err(err) => return Err(ChallengeVerifyError::Pow(err)),
            }
        }

        self.verify_unsigned_ticket_bytes(bytes)
    }

    /// Verify a signed ticket using the configured binding and revocation store.
    ///
    /// The caller must supply the relay's ML-DSA public key used to issue the ticket.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn verify_signed_ticket(
        &self,
        bytes: &[u8],
        public_key: &[u8],
    ) -> Result<Option<ChallengeAdmission>, ChallengeVerifyError> {
        if !self.pow_required() {
            return Ok(None);
        }
        let signed = SignedTicket::decode(bytes).map_err(ChallengeVerifyError::Pow)?;
        self.verify_signed_ticket_decoded(&signed, public_key)
    }

    fn verify_signed_ticket_decoded(
        &self,
        signed: &SignedTicket,
        public_key: &[u8],
    ) -> Result<Option<ChallengeAdmission>, ChallengeVerifyError> {
        if let Some(error) = self.revocation_store_error.as_ref() {
            return Err(ChallengeVerifyError::RevocationStore(error.to_string()));
        }
        let mut store = self
            .revocation_store
            .as_ref()
            .map(|store| store.lock())
            .transpose()
            .map_err(|_| {
                iroha_logger::error!("soranet pow revocation store lock poisoned");
                ChallengeVerifyError::RevocationStore("lock_poisoned".to_string())
            })?;
        let binding = self.pow_binding();
        let admission = self.admission_for_difficulty(signed.ticket.difficulty);
        pow::verify_signed_ticket(
            signed,
            public_key,
            &binding,
            self.pow_params.as_ref(),
            store.as_deref_mut(),
        )
        .map_err(|err| match err {
            pow::Error::Replay => ChallengeVerifyError::Replay,
            pow::Error::RevocationStore(message) => {
                iroha_logger::warn!(error = %message, "soranet pow revocation store error");
                ChallengeVerifyError::RevocationStore(message)
            }
            other => ChallengeVerifyError::Pow(other),
        })?;
        Ok(Some(admission))
    }

    fn verify_unsigned_ticket_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<Option<ChallengeAdmission>, ChallengeVerifyError> {
        let ticket = PowTicket::parse(bytes).map_err(ChallengeVerifyError::Pow)?;
        let admission = self
            .puzzle_params
            .as_ref()
            .map_or_else(
                || {
                    let binding = self.pow_binding();
                    pow::verify(&ticket, &binding, self.pow_params.as_ref())
                        .map_err(ChallengeVerifyError::Pow)
                },
                |params| {
                    let binding = self.puzzle_binding();
                    puzzle::verify(&ticket, &binding, params.as_ref())
                        .map_err(ChallengeVerifyError::Puzzle)
                },
            )
            .map(|()| self.admission_for_difficulty(ticket.difficulty))?;

        self.enforce_revocation(&ticket)?;

        Ok(Some(admission))
    }
}

/// Errors encountered while minting `SoraNet` handshake challenges.
#[derive(Debug, Error)]
pub enum ChallengeMintError {
    /// Underlying `PoW` ticket minting failure.
    #[error("pow ticket mint failed: {0}")]
    Pow(#[from] pow::MintError),
    /// Argon2 puzzle minting failure.
    #[error("puzzle ticket mint failed: {0}")]
    Puzzle(#[from] puzzle::MintError),
}

/// Errors encountered while verifying `SoraNet` handshake challenges.
#[derive(Debug, Error)]
pub enum ChallengeVerifyError {
    /// Underlying `PoW` ticket verification failure.
    #[error("pow ticket verification failed: {0}")]
    Pow(#[from] pow::Error),
    /// Argon2 puzzle verification failure.
    #[error("puzzle ticket verification failed: {0}")]
    Puzzle(#[from] puzzle::Error),
    /// Ticket replay detected by the revocation store.
    #[error("replay")]
    Replay,
    /// Revocation store failed to accept or load the entry.
    #[error("store_error")]
    RevocationStore(String),
}

/// Admission policy snapshot returned alongside minted or verified tickets.
#[derive(Debug, Clone, Copy)]
pub struct ChallengeAdmission {
    /// Effective `PoW` parameters (including adaptive difficulty).
    pub pow: PowParameters,
    /// Ticket TTL after applying policy clamps.
    pub ticket_ttl: Duration,
    /// Optional puzzle parameters when Argon2 gating is enabled.
    pub puzzle: Option<PuzzleParameters>,
}

/// Minted ticket bytes alongside the admission policy summary.
#[derive(Debug, Clone)]
pub struct MintedChallenge {
    /// Handshake frames (token, puzzle) to send before the client hello.
    pub frames: Vec<Vec<u8>>,
    /// Serialized puzzle ticket if one was minted.
    pub ticket: Option<Vec<u8>>,
    /// Admission policy applied when minting the ticket.
    pub admission: Option<ChallengeAdmission>,
}

#[cfg(test)]
mod handshake_config_tests {
    use std::num::NonZeroU32;

    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn sanitises_invalid_kem_and_signature_ids() {
        let params = PowParameters::new(0, Duration::from_secs(300), Duration::from_secs(30));
        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            42,
            99,
            None,
            false,
            params,
            None,
            Duration::from_secs(60),
            None,
            None,
            None,
        );
        let runtime = config.runtime_params();
        assert_eq!(runtime.kem_id, 1);
        assert_eq!(runtime.sig_id, 1);
    }

    #[test]
    fn puzzle_ticket_mints_and_verifies() {
        let pow_params = PowParameters::new(5, Duration::from_secs(900), Duration::from_secs(120));
        let puzzle_params = puzzle::Parameters::new(
            NonZeroU32::new(64 * 1024).expect("memory"),
            NonZeroU32::new(2).expect("time"),
            NonZeroU32::new(1).expect("lanes"),
            2,
            Duration::from_secs(900),
            Duration::from_secs(120),
        );
        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            Some(puzzle_params),
            Duration::from_secs(240),
            None,
            None,
            None,
        );
        assert_eq!(config.pow_parameters().difficulty(), 5);
        assert_eq!(config.pow_ticket_ttl(), Duration::from_secs(240));
        let configured_puzzle = config
            .puzzle_parameters()
            .expect("puzzle parameters available");
        assert_eq!(configured_puzzle.memory_kib().get(), 64 * 1024);
        let admission = config
            .admission_summary()
            .expect("admission summary present");
        assert_eq!(admission.pow.difficulty(), 5);
        assert_eq!(admission.ticket_ttl, Duration::from_secs(240));

        let mut rng = StdRng::from_seed([7u8; 32]);
        let minted = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint ticket")
            .expect("ticket bytes present");
        assert_eq!(
            minted
                .admission
                .expect("admission present")
                .pow
                .difficulty(),
            puzzle_params.difficulty()
        );

        let verification = config
            .verify_challenge_ticket(
                minted
                    .ticket
                    .as_ref()
                    .expect("ticket bytes present")
                    .as_slice(),
            )
            .expect("verify ticket");
        assert_eq!(
            verification.expect("verification summary").pow.difficulty(),
            puzzle_params.difficulty()
        );

        let mut corrupted = minted.ticket.expect("ticket bytes present");
        // Corrupt the version byte to guarantee a parse/verify failure.
        // Flipping solution bytes is probabilistic for low difficulties (it may still satisfy
        // the leading-zero predicate), so do not rely on it in tests.
        corrupted[0] ^= 0xFF;
        assert!(config.verify_challenge_ticket(&corrupted).is_err());
    }

    #[test]
    fn token_frame_emitted_when_configured() {
        let pow_params = PowParameters::new(5, Duration::from_secs(900), Duration::from_secs(120));
        let mut config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(240),
            None,
            None,
            None,
        );

        let mut encoded = b"SNTK\x01".to_vec();
        encoded.extend_from_slice(&[0xAA; 64]);
        config.set_admission_token(encoded.clone());

        let mut rng = StdRng::from_seed([0x99; 32]);
        let minted = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint token challenge")
            .expect("token frame present");

        assert!(minted.ticket.is_none());
        assert!(minted.admission.is_none());
        assert_eq!(minted.frames.len(), 1);
        assert_eq!(minted.frames[0], encoded);
    }

    #[test]
    fn pow_ticket_replay_rejected_and_persisted() {
        let pow_params = PowParameters::new(1, Duration::from_secs(900), Duration::from_secs(120));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(240),
            None,
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0x21; 32]);
        let minted = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint")
            .expect("ticket present");
        let ticket = minted.ticket.expect("ticket bytes");

        config
            .verify_challenge_ticket(&ticket)
            .expect("first verify");
        let err = config
            .verify_challenge_ticket(&ticket)
            .expect_err("replay must fail");
        assert!(matches!(err, ChallengeVerifyError::Replay));

        let reloaded =
            TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
        let config_reloaded = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(240),
            None,
            Some(Arc::new(Mutex::new(reloaded))),
            None,
        );
        let err = config_reloaded
            .verify_challenge_ticket(&ticket)
            .expect_err("replay after reload must fail");
        assert!(matches!(err, ChallengeVerifyError::Replay));
    }

    #[test]
    fn signed_ticket_replay_persists_across_reload() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("signed_revocations.norito");
        let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(900)).expect("limits");
        let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");

        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(180),
            Some(keypair.public_key().to_vec()),
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0x27; 32]);
        let ticket = pow::mint_ticket(
            config.pow_params.as_ref(),
            &config.pow_binding(),
            config.pow_ticket_ttl(),
            &mut rng,
        )
        .expect("mint pow ticket");
        let signed = SignedTicket::sign(
            ticket,
            &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
            None,
            keypair.secret_key(),
        )
        .expect("sign ticket");
        let signed_bytes = signed.encode();

        config
            .verify_challenge_ticket(&signed_bytes)
            .expect("first verify signed ticket");

        let reloaded =
            TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
        let config_reloaded = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(180),
            Some(keypair.public_key().to_vec()),
            Some(Arc::new(Mutex::new(reloaded))),
            None,
        );
        let err = config_reloaded
            .verify_challenge_ticket(&signed_bytes)
            .expect_err("signed ticket replay after reload must fail");
        assert!(matches!(err, ChallengeVerifyError::Replay));
    }

    #[test]
    fn revocation_store_eviction_and_counts_surface() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(1, Duration::from_secs(900)).expect("limits");
        let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(120),
            None,
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0x31; 32]);
        let first = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint")
            .expect("ticket");
        let second = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint second")
            .expect("ticket");

        config
            .verify_challenge_ticket(first.ticket.as_ref().expect("ticket bytes"))
            .expect("first verify");
        assert_eq!(config.active_revocations(), 1);

        config
            .verify_challenge_ticket(second.ticket.as_ref().expect("ticket bytes"))
            .expect("second verify");
        assert_eq!(
            config.active_revocations(),
            1,
            "capacity-one store should evict oldest entry"
        );

        config.purge_expired_revocations().expect("purge succeeds");
        assert_eq!(
            config.active_revocations(),
            1,
            "purge should not drop non-expired entries"
        );
    }

    #[test]
    fn revocation_store_ttl_overflow_surfaces_store_error() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revocations.norito");
        let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(10)).expect("limits");
        let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(120),
            None,
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0x41; 32]);
        let minted = config
            .mint_challenge_ticket(&mut rng)
            .expect("mint")
            .expect("ticket");
        let err = config
            .verify_challenge_ticket(minted.ticket.as_ref().expect("ticket bytes"))
            .expect_err("revocation store ttl cap should reject ticket");
        assert!(matches!(err, ChallengeVerifyError::RevocationStore(_)));
    }

    #[test]
    fn signed_ticket_invalid_signature_rejected() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(600)).expect("limits");
        let store =
            TicketRevocationStore::in_memory(limits).expect("revocation store should be available");

        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(120),
            None,
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let ticket = PowTicket {
            version: 1,
            difficulty: 1,
            expires_at: 1_000,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let signed = SignedTicket {
            ticket,
            relay_id: config.relay_id.as_slice().try_into().unwrap(),
            transcript_hash: None,
            signature: vec![0u8; 48],
        };
        let signed_bytes = signed.encode();

        let err = config
            .verify_signed_ticket(&signed_bytes, b"invalid-public-key")
            .expect_err("invalid signature must fail");
        match err {
            ChallengeVerifyError::Pow(pow_err) => assert!(matches!(
                pow_err,
                pow::Error::InvalidSignature | pow::Error::PostQuantum(_)
            )),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn signed_ticket_with_config_key_accepts_once() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(900)).expect("limits");
        let store =
            TicketRevocationStore::in_memory(limits).expect("revocation store should be available");
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");

        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(120),
            Some(keypair.public_key().to_vec()),
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0x55; 32]);
        let ticket = pow::mint_ticket(
            config.pow_params.as_ref(),
            &config.pow_binding(),
            config.pow_ticket_ttl(),
            &mut rng,
        )
        .expect("mint pow ticket");
        let signed = SignedTicket::sign(
            ticket,
            &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
            None,
            keypair.secret_key(),
        )
        .expect("sign ticket");
        let signed_bytes = signed.encode();

        let admission = config
            .verify_challenge_ticket(&signed_bytes)
            .expect("verify signed ticket")
            .expect("admission");
        assert_eq!(admission.pow.difficulty(), pow_params.difficulty());

        let err = config
            .verify_challenge_ticket(&signed_bytes)
            .expect_err("replay should be rejected");
        assert!(matches!(err, ChallengeVerifyError::Replay));
    }

    #[test]
    fn raw_ticket_accepted_with_signed_key_present() {
        let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
        let store =
            TicketRevocationStore::in_memory(limits).expect("revocation store should be available");
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");

        let config = SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            None,
            Duration::from_secs(120),
            Some(keypair.public_key().to_vec()),
            Some(Arc::new(Mutex::new(store))),
            None,
        );

        let mut rng = StdRng::from_seed([0xA5; 32]);
        let ticket = pow::mint_ticket(
            config.pow_params.as_ref(),
            &config.pow_binding(),
            config.pow_ticket_ttl(),
            &mut rng,
        )
        .expect("mint pow ticket");
        let ticket_bytes = ticket.to_vec();

        let admission = config
            .verify_challenge_ticket(&ticket_bytes)
            .expect("verify ticket")
            .expect("admission");
        assert_eq!(admission.pow.difficulty(), pow_params.difficulty());
    }
}

/// Returns the number of handshake failures observed in this process.
pub fn handshake_failure_count() -> u64 {
    HANDSHAKE_FAILURES.load(Ordering::Relaxed)
}
/// Returns the number of handshake timeouts observed.
pub fn handshake_error_timeout() -> u64 {
    HSE_TIMEOUT.load(Ordering::Relaxed)
}
/// Returns the number of preface (magic/version) errors observed.
pub fn handshake_error_preface() -> u64 {
    HSE_PREFACE.load(Ordering::Relaxed)
}
/// Returns the number of signature/verification errors observed.
pub fn handshake_error_verify() -> u64 {
    HSE_VERIFY.load(Ordering::Relaxed)
}
/// Returns the number of decryption errors observed.
pub fn handshake_error_decrypt() -> u64 {
    HSE_DECRYPT.load(Ordering::Relaxed)
}
/// Returns the number of Norito codec errors observed during handshake.
pub fn handshake_error_codec() -> u64 {
    HSE_CODEC.load(Ordering::Relaxed)
}
/// Returns the number of I/O errors observed during handshake.
pub fn handshake_error_io() -> u64 {
    HSE_IO.load(Ordering::Relaxed)
}
/// Returns the number of miscellaneous handshake errors observed.
pub fn handshake_error_other() -> u64 {
    HSE_OTHER.load(Ordering::Relaxed)
}
/// Returns the histogram bucket upper bounds (milliseconds) for handshake latency.
pub fn handshake_bucket_bounds_ms() -> &'static [u64] {
    &HANDSHAKE_BUCKETS_MS
}
/// Returns the current handshake latency histogram counts per bucket.
pub fn handshake_bucket_counts() -> Vec<u64> {
    HANDSHAKE_BUCKET_COUNTS
        .iter()
        .map(|c| c.load(Ordering::Relaxed))
        .collect()
}
/// Returns the total sum (milliseconds) of observed handshake latencies.
pub fn handshake_ms_sum() -> u64 {
    HANDSHAKE_MS_SUM.load(Ordering::Relaxed)
}
/// Returns the total count of observed handshakes.
pub fn handshake_ms_count() -> u64 {
    HANDSHAKE_MS_COUNT.load(Ordering::Relaxed)
}

/// Returns the number of decrypted peer frames dropped due to malformed inner payloads.
pub fn malformed_payload_frame_count() -> u64 {
    MALFORMED_PAYLOAD_FRAMES.load(Ordering::Relaxed)
}

fn record_malformed_payload_frame() {
    MALFORMED_PAYLOAD_FRAMES.fetch_add(1, Ordering::Relaxed);
}

fn observe_handshake_ms(ms: u64) {
    HANDSHAKE_MS_SUM.fetch_add(ms, Ordering::Relaxed);
    HANDSHAKE_MS_COUNT.fetch_add(1, Ordering::Relaxed);
    for (i, b) in HANDSHAKE_BUCKETS_MS.iter().enumerate() {
        if ms <= *b {
            HANDSHAKE_BUCKET_COUNTS[i].fetch_add(1, Ordering::Relaxed);
            break;
        }
    }
}

// Pre-handshake magic/version used to quickly reject garbage before
// entering the cryptographic handshake. Outbound writes first, inbound
// reads first, to avoid deadlock.
const PRE_MAGIC: &[u8; 4] = b"I2P2";
const PRE_VERSION: u8 = 1;

async fn write_pre_handshake_header<W>(write: &mut W) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    write.write_all(PRE_MAGIC).await?;
    write.write_all(&[PRE_VERSION]).await?;
    write.flush().await?;
    Ok(())
}

async fn read_and_verify_pre_handshake_header<R>(read: &mut R) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut magic = [0u8; 4];
    let mut ver = [0u8; 1];
    read.read_exact(&mut magic).await?;
    read.read_exact(&mut ver).await?;
    if &magic != PRE_MAGIC || ver[0] != PRE_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad pre-handshake header",
        ));
    }
    Ok(())
}

async fn write_handshake_frame<W>(write: &mut W, payload: &[u8]) -> Result<(), crate::Error>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_HANDSHAKE_LENGTH as usize {
        return Err(crate::Error::HandshakeMessageTooLarge);
    }
    let len = u16::try_from(payload.len()).map_err(|_| crate::Error::HandshakeMessageTooLarge)?;
    write.write_all(&len.to_be_bytes()).await?;
    write.write_all(payload).await?;
    write.flush().await?;
    Ok(())
}

async fn read_handshake_frame<R>(read: &mut R) -> Result<Vec<u8>, crate::Error>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 2];
    read.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf);
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut payload = vec![0u8; len as usize];
    read.read_exact(&mut payload).await?;
    Ok(payload)
}

#[cfg(feature = "noise_handshake")]
fn map_noise_error(err: snow::Error) -> crate::Error {
    crate::Error::HandshakeNoise(err.to_string())
}

#[cfg(feature = "noise_handshake")]
fn derive_noise_key(handshake_hash: &[u8]) -> [u8; 32] {
    let hash = Blake2bVar::new(32)
        .expect("blake2b-256 output length must be valid")
        .chain(handshake_hash)
        .finalize_boxed();
    let mut out = [0u8; 32];
    out.copy_from_slice(&hash);
    out
}

#[cfg(feature = "noise_handshake")]
async fn noise_handshake_initiator<R, W>(read: &mut R, write: &mut W) -> Result<Vec<u8>, Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let params: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2b"
        .parse()
        .expect("noise params must be valid");
    let builder = Builder::new(params);
    let keypair = builder.generate_keypair().map_err(map_noise_error)?;
    let mut initiator = builder
        .local_private_key(&keypair.private)
        .map_err(map_noise_error)?
        .build_initiator()
        .map_err(map_noise_error)?;

    let mut out = vec![0u8; MAX_HANDSHAKE_LENGTH as usize];
    let mut payload = vec![0u8; MAX_HANDSHAKE_LENGTH as usize];

    let len = initiator
        .write_message(&[], &mut out)
        .map_err(map_noise_error)?;
    write_handshake_frame(write, &out[..len]).await?;

    let msg = read_handshake_frame(read).await?;
    initiator
        .read_message(&msg, &mut payload)
        .map_err(map_noise_error)?;

    let len = initiator
        .write_message(&[], &mut out)
        .map_err(map_noise_error)?;
    write_handshake_frame(write, &out[..len]).await?;

    let key = derive_noise_key(initiator.get_handshake_hash());
    initiator.into_transport_mode().map_err(map_noise_error)?;
    Ok(key.to_vec())
}

#[cfg(feature = "noise_handshake")]
async fn noise_handshake_responder<R, W>(read: &mut R, write: &mut W) -> Result<Vec<u8>, Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let params: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2b"
        .parse()
        .expect("noise params must be valid");
    let builder = Builder::new(params);
    let keypair = builder.generate_keypair().map_err(map_noise_error)?;
    let mut responder = builder
        .local_private_key(&keypair.private)
        .map_err(map_noise_error)?
        .build_responder()
        .map_err(map_noise_error)?;

    let mut out = vec![0u8; MAX_HANDSHAKE_LENGTH as usize];
    let mut payload = vec![0u8; MAX_HANDSHAKE_LENGTH as usize];

    let msg = read_handshake_frame(read).await?;
    responder
        .read_message(&msg, &mut payload)
        .map_err(map_noise_error)?;

    let len = responder
        .write_message(&[], &mut out)
        .map_err(map_noise_error)?;
    write_handshake_frame(write, &out[..len]).await?;

    let msg = read_handshake_frame(read).await?;
    responder
        .read_message(&msg, &mut payload)
        .map_err(map_noise_error)?;

    let key = derive_noise_key(responder.get_handshake_hash());
    responder.into_transport_mode().map_err(map_noise_error)?;
    Ok(key.to_vec())
}

mod post_channel {
    use tokio::sync::mpsc;

    pub type Sender<T> = mpsc::Sender<T>;
    pub type Receiver<T> = mpsc::Receiver<T>;

    pub fn channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
        mpsc::channel(cap)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, OnceLock};

    /// Origin for a peer task spawn observed in tests.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpawnPath {
        /// Outbound dialer (`connecting`).
        Connecting,
        /// Inbound accept (`connected_from`).
        ConnectedFrom,
    }

    static RECORDS: OnceLock<Mutex<Vec<(SpawnPath, usize)>>> = OnceLock::new();

    fn records() -> &'static Mutex<Vec<(SpawnPath, usize)>> {
        RECORDS.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Record a spawn observation for later assertions.
    pub fn record(path: SpawnPath, value: usize) {
        let mut guard = records().lock().expect("spawn record mutex poisoned");
        guard.push((path, value));
    }

    /// Snapshot the spawn observations accumulated so far.
    pub fn snapshot() -> Vec<(SpawnPath, usize)> {
        records()
            .lock()
            .expect("spawn record mutex poisoned")
            .clone()
    }
}

pub mod handles {
    //! Module with functions to start peer actor and handle to interact with it.

    use iroha_crypto::KeyPair;
    use iroha_logger::Instrument;
    use iroha_primitives::addr::SocketAddr;

    use super::{run::RunPeerArgs, *};

    /// Start Peer in `state::Connecting` state
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub(crate) fn connecting<T: Pload + crate::network::message::ClassifyTopic, K: Kex, E: Enc>(
        peer_addr: SocketAddr,
        peer_id: iroha_data_model::prelude::PeerId,
        our_public_address: SocketAddr,
        key_pair: KeyPair,
        connection_id: ConnectionId,
        service_message_sender: mpsc::Sender<ServiceMessage<T>>,
        idle_timeout: Duration,
        dial_timeout: Duration,
        chain_id: Option<iroha_data_model::ChainId>,
        consensus_caps: Option<crate::ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        soranet_handshake: Arc<SoranetHandshakeConfig>,
        post_capacity: usize,
        quic_enabled: bool,
        tls_enabled: bool,
        tls_fallback_to_plain: bool,
        prefer_scion: bool,
        local_scion_supported: bool,
        prefer_ws_fallback: bool,
        trust_gossip: bool,
        max_frame_bytes: usize,
        relay_role: RelayRole,
        happy_eyeballs_stagger: Duration,
        tcp_nodelay: bool,
        tcp_keepalive: Option<Duration>,
        proxy_tls_verify: bool,
        proxy_tls_pinned_cert_der: Option<std::sync::Arc<[u8]>>,
        proxy_policy: crate::transport::ProxyPolicy,
        quic_dialer: Option<crate::transport::QuicDialer>,
        quic_datagrams_enabled: bool,
        quic_datagram_max_payload_bytes: usize,
    ) {
        #[cfg(test)]
        crate::peer::test_support::record(
            crate::peer::test_support::SpawnPath::Connecting,
            max_frame_bytes,
        );
        let peer = state::Connecting {
            peer_addr,
            peer_id,
            our_public_address,
            key_pair,
            connection_id,
            chain_id,
            consensus_caps,
            confidential_caps,
            crypto_caps,
            soranet_handshake,
            quic_enabled,
            tls_enabled,
            tls_fallback_to_plain,
            prefer_scion,
            local_scion_supported,
            prefer_ws_fallback,
            trust_gossip,
            relay_role,
            dial_timeout,
            happy_eyeballs_stagger,
            tcp_nodelay,
            tcp_keepalive,
            proxy_tls_verify,
            proxy_tls_pinned_cert_der,
            proxy_policy,
            quic_dialer,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Start Peer in `state::ConnectedFrom` state
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connected_from<
        T: Pload + crate::network::message::ClassifyTopic,
        K: Kex,
        E: Enc,
    >(
        our_public_address: SocketAddr,
        key_pair: KeyPair,
        connection: Connection,
        service_message_sender: mpsc::Sender<ServiceMessage<T>>,
        idle_timeout: Duration,
        chain_id: Option<iroha_data_model::ChainId>,
        consensus_caps: Option<crate::ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        soranet_handshake: Arc<SoranetHandshakeConfig>,
        local_scion_supported: bool,
        post_capacity: usize,
        relay_role: RelayRole,
        trust_gossip: bool,
        max_frame_bytes: usize,
        quic_datagrams_enabled: bool,
        quic_datagram_max_payload_bytes: usize,
    ) {
        #[cfg(test)]
        crate::peer::test_support::record(
            crate::peer::test_support::SpawnPath::ConnectedFrom,
            max_frame_bytes,
        );
        let peer = state::ConnectedFrom {
            our_public_address,
            key_pair,
            connection,
            chain_id,
            consensus_caps,
            confidential_caps,
            crypto_caps,
            soranet_handshake,
            local_scion_supported,
            trust_gossip,
            relay_role,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Per-topic senders for peer substreams.
    pub(super) struct TopicSenders<T> {
        pub(super) hi_consensus: post_channel::Sender<T>,
        pub(super) hi_consensus_payload: post_channel::Sender<T>,
        pub(super) hi_consensus_chunk: post_channel::Sender<T>,
        pub(super) hi_control: post_channel::Sender<T>,
        pub(super) lo_block_sync: post_channel::Sender<T>,
        pub(super) lo_tx_gossip: post_channel::Sender<T>,
        pub(super) lo_peer_gossip: post_channel::Sender<T>,
        pub(super) lo_health: post_channel::Sender<T>,
        pub(super) lo_other: post_channel::Sender<T>,
    }

    /// Post error reason for bounded per‑peer channels.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PostError {
        /// Per-topic bounded channel is full.
        Full,
        /// Peer task/channel closed.
        Closed,
    }

    /// Peer actor handle.
    pub struct PeerHandle<T: Pload> {
        // NOTE: it's ok for these channels to be unbounded in default mode.
        // Because post messages originate inside the system and their rate is configurable.
        pub(super) senders: TopicSenders<T>,
    }

    impl<T: Pload> PeerHandle<T> {
        /// Post message `T` on Peer
        ///
        /// # Errors
        /// Fail if peer terminated
        pub fn post(&self, msg: T) -> Result<(), PostError>
        where
            T: crate::network::message::ClassifyTopic,
        {
            use tokio::sync::mpsc::error::TrySendError;

            let topic = msg.topic();
            let sender = match topic {
                crate::network::message::Topic::Consensus => &self.senders.hi_consensus,
                crate::network::message::Topic::ConsensusPayload => {
                    &self.senders.hi_consensus_payload
                }
                crate::network::message::Topic::ConsensusChunk => &self.senders.hi_consensus_chunk,
                crate::network::message::Topic::Control => &self.senders.hi_control,
                crate::network::message::Topic::BlockSync => &self.senders.lo_block_sync,
                crate::network::message::Topic::TxGossip
                | crate::network::message::Topic::TxGossipRestricted => &self.senders.lo_tx_gossip,
                crate::network::message::Topic::PeerGossip
                | crate::network::message::Topic::TrustGossip => &self.senders.lo_peer_gossip,
                crate::network::message::Topic::Health => &self.senders.lo_health,
                crate::network::message::Topic::Other => &self.senders.lo_other,
            };

            sender.try_send(msg).map_err(|e| match e {
                TrySendError::Full(_) => PostError::Full,
                TrySendError::Closed(_) => PostError::Closed,
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use norito::codec::{Decode, Encode};
        use tokio::sync::mpsc::error::TryRecvError;

        use super::*;
        use crate::network::message::{ClassifyTopic, Topic};

        #[derive(Clone, Debug, Decode, Encode)]
        struct ConsensusChunkMsg;

        impl<'a> norito::core::DecodeFromSlice<'a> for ConsensusChunkMsg {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                norito::core::decode_field_canonical::<Self>(bytes)
            }
        }

        impl ClassifyTopic for ConsensusChunkMsg {
            fn topic(&self) -> Topic {
                Topic::ConsensusChunk
            }
        }

        #[derive(Clone, Debug, Decode, Encode)]
        struct ConsensusPayloadMsg;

        impl<'a> norito::core::DecodeFromSlice<'a> for ConsensusPayloadMsg {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                norito::core::decode_field_canonical::<Self>(bytes)
            }
        }

        impl ClassifyTopic for ConsensusPayloadMsg {
            fn topic(&self) -> Topic {
                Topic::ConsensusPayload
            }
        }

        #[test]
        fn consensus_chunk_routes_to_high_queue() {
            let (hi_consensus_tx, mut hi_consensus_rx) = post_channel::channel(1);
            let (hi_consensus_payload_tx, mut hi_consensus_payload_rx) = post_channel::channel(1);
            let (hi_consensus_chunk_tx, mut hi_consensus_chunk_rx) = post_channel::channel(1);
            let (hi_control_tx, mut hi_control_rx) = post_channel::channel(1);
            let (lo_block_sync_tx, mut lo_block_sync_rx) = post_channel::channel(1);
            let (lo_tx_gossip_tx, mut lo_tx_gossip_rx) = post_channel::channel(1);
            let (lo_peer_gossip_tx, mut lo_peer_gossip_rx) = post_channel::channel(1);
            let (lo_health_tx, mut lo_health_rx) = post_channel::channel(1);
            let (lo_other_tx, mut lo_other_rx) = post_channel::channel(1);

            let handle = PeerHandle {
                senders: TopicSenders {
                    hi_consensus: hi_consensus_tx,
                    hi_consensus_payload: hi_consensus_payload_tx,
                    hi_consensus_chunk: hi_consensus_chunk_tx,
                    hi_control: hi_control_tx,
                    lo_block_sync: lo_block_sync_tx,
                    lo_tx_gossip: lo_tx_gossip_tx,
                    lo_peer_gossip: lo_peer_gossip_tx,
                    lo_health: lo_health_tx,
                    lo_other: lo_other_tx,
                },
            };

            handle
                .post(ConsensusChunkMsg)
                .expect("consensus chunk post should succeed");

            assert!(matches!(
                hi_consensus_chunk_rx.try_recv(),
                Ok(ConsensusChunkMsg)
            ));
            assert!(matches!(
                hi_consensus_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                hi_consensus_payload_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(hi_control_rx.try_recv(), Err(TryRecvError::Empty)));
            assert!(matches!(
                lo_block_sync_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                lo_tx_gossip_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                lo_peer_gossip_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(lo_health_rx.try_recv(), Err(TryRecvError::Empty)));
            assert!(matches!(lo_other_rx.try_recv(), Err(TryRecvError::Empty)));
        }

        #[test]
        fn consensus_payload_routes_to_dedicated_high_queue() {
            let (hi_consensus_tx, mut hi_consensus_rx) = post_channel::channel(1);
            let (hi_consensus_payload_tx, mut hi_consensus_payload_rx) = post_channel::channel(1);
            let (hi_consensus_chunk_tx, mut hi_consensus_chunk_rx) = post_channel::channel(1);
            let (hi_control_tx, mut hi_control_rx) = post_channel::channel(1);
            let (lo_block_sync_tx, mut lo_block_sync_rx) = post_channel::channel(1);
            let (lo_tx_gossip_tx, mut lo_tx_gossip_rx) = post_channel::channel(1);
            let (lo_peer_gossip_tx, mut lo_peer_gossip_rx) = post_channel::channel(1);
            let (lo_health_tx, mut lo_health_rx) = post_channel::channel(1);
            let (lo_other_tx, mut lo_other_rx) = post_channel::channel(1);

            let handle = PeerHandle {
                senders: TopicSenders {
                    hi_consensus: hi_consensus_tx,
                    hi_consensus_payload: hi_consensus_payload_tx,
                    hi_consensus_chunk: hi_consensus_chunk_tx,
                    hi_control: hi_control_tx,
                    lo_block_sync: lo_block_sync_tx,
                    lo_tx_gossip: lo_tx_gossip_tx,
                    lo_peer_gossip: lo_peer_gossip_tx,
                    lo_health: lo_health_tx,
                    lo_other: lo_other_tx,
                },
            };

            handle
                .post(ConsensusPayloadMsg)
                .expect("consensus payload post should succeed");

            assert!(matches!(
                hi_consensus_payload_rx.try_recv(),
                Ok(ConsensusPayloadMsg)
            ));
            assert!(matches!(
                hi_consensus_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                hi_consensus_chunk_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(hi_control_rx.try_recv(), Err(TryRecvError::Empty)));
            assert!(matches!(
                lo_block_sync_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                lo_tx_gossip_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(
                lo_peer_gossip_rx.try_recv(),
                Err(TryRecvError::Empty)
            ));
            assert!(matches!(lo_health_rx.try_recv(), Err(TryRecvError::Empty)));
            assert!(matches!(lo_other_rx.try_recv(), Err(TryRecvError::Empty)));
        }
    }
}

mod run {
    //! Module with peer [`run`] function.

    use std::task::Poll;

    #[cfg(feature = "quic")]
    use bytes::Bytes;
    use futures::future::poll_fn;
    use iroha_logger::prelude::*;
    use norito::codec::Decode;
    use tokio::time::Instant;
    use tracing;

    use crate::network::message::{ClassifyTopic, Topic};
    use crate::{Priority, sampler::LogSampler};

    use super::{
        cryptographer::Cryptographer,
        handshake_flow::Handshake,
        state::{ConnectedFrom, Connecting, Ready},
        *,
    };

    #[cfg(feature = "quic")]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum DatagramSend {
        Sent { bytes: usize },
        TooLarge,
        Unsupported,
        Disabled,
    }

    #[cfg(feature = "quic")]
    struct QuicDatagramSender<E: Enc> {
        connection: quinn::Connection,
        cryptographer: Cryptographer<E>,
        buffer: Vec<u8>,
        encrypted: Vec<u8>,
        max_frame_bytes: usize,
        max_payload_bytes: usize,
    }

    #[cfg(feature = "quic")]
    impl<E: Enc> QuicDatagramSender<E> {
        fn new(
            connection: quinn::Connection,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
            max_payload_bytes: usize,
        ) -> Self {
            Self {
                connection,
                cryptographer,
                buffer: Vec::new(),
                encrypted: Vec::new(),
                max_frame_bytes,
                max_payload_bytes,
            }
        }

        fn try_send<T: Pload + ClassifyTopic>(&mut self, msg: &T) -> Result<DatagramSend, Error> {
            // Encode a single Norito-framed payload and encrypt it with the negotiated session key.
            ncore::to_bytes_in(msg, &mut self.buffer)?;
            let max_plaintext = crate::frame_plaintext_cap(self.max_frame_bytes);
            if self.buffer.len() > max_plaintext {
                return Err(Error::FrameTooLarge);
            }
            let encrypted = self
                .cryptographer
                .encrypt_into(&self.buffer, &mut self.encrypted)?;

            let Some(mut max_datagram) = self.connection.max_datagram_size() else {
                return Ok(DatagramSend::Unsupported);
            };
            max_datagram = max_datagram.min(self.max_payload_bytes);
            if max_datagram == 0 || self.max_payload_bytes == 0 {
                return Ok(DatagramSend::Disabled);
            }
            if encrypted.len() > max_datagram {
                return Ok(DatagramSend::TooLarge);
            }

            match self
                .connection
                .send_datagram(Bytes::copy_from_slice(encrypted))
            {
                Ok(()) => Ok(DatagramSend::Sent {
                    bytes: encrypted.len(),
                }),
                Err(quinn::SendDatagramError::UnsupportedByPeer) => Ok(DatagramSend::Unsupported),
                Err(quinn::SendDatagramError::Disabled) => Ok(DatagramSend::Disabled),
                Err(quinn::SendDatagramError::TooLarge) => Ok(DatagramSend::TooLarge),
                Err(quinn::SendDatagramError::ConnectionLost(e)) => {
                    Err(std::io::Error::other(format!("quic datagram send failed: {e}")).into())
                }
            }
        }
    }

    #[cfg(feature = "quic")]
    struct QuicDatagramReceiver<E: Enc, T: Pload> {
        connection: quinn::Connection,
        cryptographer: Cryptographer<E>,
        decrypted: Vec<u8>,
        framed_schema: [u8; 16],
        framed_padding: usize,
        max_frame_bytes: usize,
        _payload: std::marker::PhantomData<T>,
    }

    #[cfg(feature = "quic")]
    impl<E: Enc, T: Pload> QuicDatagramReceiver<E, T> {
        fn new(
            connection: quinn::Connection,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
        ) -> Self {
            let framed_schema = <T as ncore::NoritoSerialize>::schema_hash();
            let align = core::mem::align_of::<ncore::Archived<T>>();
            let framed_padding = if align <= 1 {
                0
            } else {
                let rem = ncore::Header::SIZE % align;
                if rem == 0 { 0 } else { align - rem }
            };
            Self {
                connection,
                cryptographer,
                decrypted: Vec::new(),
                framed_schema,
                framed_padding,
                max_frame_bytes,
                _payload: std::marker::PhantomData,
            }
        }

        async fn recv(&mut self) -> Result<(T, usize), Error> {
            let datagram =
                self.connection.read_datagram().await.map_err(|e| {
                    std::io::Error::other(format!("quic datagram recv failed: {e}"))
                })?;
            if datagram.len() > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            let plaintext = self
                .cryptographer
                .decrypt_into(datagram.as_ref(), &mut self.decrypted)?;
            let frame_len =
                framed_message_len::<T>(plaintext, self.framed_schema, self.framed_padding)?;
            if frame_len != plaintext.len() {
                return Err(Error::Format);
            }
            let decoded = ncore::decode_from_bytes::<T>(plaintext).map_err(|err| {
                iroha_logger::warn!(error = ?err, "Failed to decode peer datagram payload");
                Error::Format
            })?;
            Ok((decoded, frame_len))
        }
    }

    #[cfg(feature = "quic")]
    type DatagramSender<E> = QuicDatagramSender<E>;
    #[cfg(not(feature = "quic"))]
    type DatagramSender<E> = std::marker::PhantomData<E>;

    #[cfg(feature = "quic")]
    type DatagramReceiver<E, T> = QuicDatagramReceiver<E, T>;
    #[cfg(not(feature = "quic"))]
    type DatagramReceiver<E, T> = std::marker::PhantomData<(E, T)>;

    #[cfg(feature = "quic")]
    async fn recv_best_effort_datagram<E: Enc, T: Pload>(
        receiver: &mut Option<DatagramReceiver<E, T>>,
    ) -> Result<(T, usize), Error> {
        let receiver = receiver.as_mut().expect("guarded by is_some");
        receiver.recv().await
    }

    #[cfg(not(feature = "quic"))]
    async fn recv_best_effort_datagram<E: Enc, T: Pload>(
        _receiver: &mut Option<DatagramReceiver<E, T>>,
    ) -> Result<(T, usize), Error> {
        // No QUIC support in this build: the branch is always disabled by the guard in `select!`,
        // so this future is never polled.
        std::future::pending::<Result<(T, usize), Error>>().await
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LowTopic {
        BlockSync,
        TxGossip,
        PeerGossip,
        Health,
        Other,
    }

    const LOW_TOPIC_COUNT: usize = 5;
    const HI_BUDGET_RESET: u8 = 32;
    const HI_BUDGET_FALLBACK: u8 = 1;
    const HI_CONTROL_BURST_MAX: u8 = 4;
    const HI_CONSENSUS_BURST_MAX: u8 = 4;
    const HI_PAYLOAD_BURST_MAX: u8 = 2;
    // Drain a few queued outbound posts per loop iteration to allow `MessageSender` to
    // batch multiple logical messages into fewer encrypted frames.
    const OUTBOUND_DRAIN_HI_MAX: usize = 8;
    const OUTBOUND_DRAIN_LO_MAX: usize = 32;
    const INBOUND_SEND_WARN_MS: u64 = 250;
    const MALFORMED_PAYLOAD_FRAME_THRESHOLD: u32 = 3;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum HighTopic {
        Control,
        Consensus,
        ConsensusPayload,
        ConsensusChunk,
    }

    fn note_malformed_payload_frame(streak: &mut u32) -> bool {
        record_malformed_payload_frame();
        *streak = streak.saturating_add(1);
        *streak >= MALFORMED_PAYLOAD_FRAME_THRESHOLD
    }

    fn low_topic_label(topic: LowTopic) -> &'static str {
        match topic {
            LowTopic::BlockSync => "low:block_sync",
            LowTopic::TxGossip => "low:tx_gossip",
            LowTopic::PeerGossip => "low:peer_gossip",
            LowTopic::Health => "low:health",
            LowTopic::Other => "low:other",
        }
    }

    fn high_topic_label(topic: HighTopic) -> &'static str {
        match topic {
            HighTopic::Control => "hi:control",
            HighTopic::Consensus => "hi:consensus",
            HighTopic::ConsensusPayload => "hi:consensus_payload",
            HighTopic::ConsensusChunk => "hi:consensus_chunk",
        }
    }

    fn note_high_topic_served(
        control_burst: &mut u8,
        consensus_burst: &mut u8,
        payload_burst: &mut u8,
        topic: HighTopic,
    ) {
        match topic {
            HighTopic::Control => {
                *control_burst = control_burst.saturating_add(1).min(HI_CONTROL_BURST_MAX);
            }
            HighTopic::Consensus => {
                *control_burst = 0;
                *consensus_burst = consensus_burst
                    .saturating_add(1)
                    .min(HI_CONSENSUS_BURST_MAX);
            }
            HighTopic::ConsensusPayload => {
                *control_burst = 0;
                *consensus_burst = 0;
                *payload_burst = payload_burst.saturating_add(1).min(HI_PAYLOAD_BURST_MAX);
            }
            HighTopic::ConsensusChunk => {
                *control_burst = 0;
                *consensus_burst = 0;
                *payload_burst = 0;
            }
        }
    }

    fn try_recv_high_data_fair<T>(
        payload_burst: u8,
        hi_consensus_payload_rx: &mut post_channel::Receiver<T>,
        hi_consensus_chunk_rx: &mut post_channel::Receiver<T>,
    ) -> Option<(HighTopic, T)> {
        if payload_burst >= HI_PAYLOAD_BURST_MAX {
            if let Some(m) = hi_consensus_chunk_rx.try_recv_now() {
                return Some((HighTopic::ConsensusChunk, m));
            }
        }
        if let Some(m) = hi_consensus_payload_rx.try_recv_now() {
            return Some((HighTopic::ConsensusPayload, m));
        }
        hi_consensus_chunk_rx
            .try_recv_now()
            .map(|m| (HighTopic::ConsensusChunk, m))
    }

    fn try_recv_high_fair<T>(
        control_burst: &mut u8,
        consensus_burst: &mut u8,
        payload_burst: &mut u8,
        hi_control_rx: &mut post_channel::Receiver<T>,
        hi_consensus_rx: &mut post_channel::Receiver<T>,
        hi_consensus_payload_rx: &mut post_channel::Receiver<T>,
        hi_consensus_chunk_rx: &mut post_channel::Receiver<T>,
    ) -> Option<(HighTopic, T)> {
        if *control_burst >= HI_CONTROL_BURST_MAX {
            if let Some(m) = hi_consensus_rx.try_recv_now() {
                note_high_topic_served(
                    control_burst,
                    consensus_burst,
                    payload_burst,
                    HighTopic::Consensus,
                );
                return Some((HighTopic::Consensus, m));
            }
            if let Some((topic, msg)) = try_recv_high_data_fair(
                *payload_burst,
                hi_consensus_payload_rx,
                hi_consensus_chunk_rx,
            ) {
                note_high_topic_served(control_burst, consensus_burst, payload_burst, topic);
                return Some((topic, msg));
            }
        }
        if let Some(m) = hi_control_rx.try_recv_now() {
            note_high_topic_served(
                control_burst,
                consensus_burst,
                payload_burst,
                HighTopic::Control,
            );
            return Some((HighTopic::Control, m));
        }
        if *consensus_burst >= HI_CONSENSUS_BURST_MAX {
            if let Some((topic, msg)) = try_recv_high_data_fair(
                *payload_burst,
                hi_consensus_payload_rx,
                hi_consensus_chunk_rx,
            ) {
                note_high_topic_served(control_burst, consensus_burst, payload_burst, topic);
                return Some((topic, msg));
            }
        }
        if let Some(m) = hi_consensus_rx.try_recv_now() {
            note_high_topic_served(
                control_burst,
                consensus_burst,
                payload_burst,
                HighTopic::Consensus,
            );
            return Some((HighTopic::Consensus, m));
        }
        let next = try_recv_high_data_fair(
            *payload_burst,
            hi_consensus_payload_rx,
            hi_consensus_chunk_rx,
        )?;
        note_high_topic_served(control_burst, consensus_burst, payload_burst, next.0);
        Some(next)
    }

    fn bump_low_rr(low_rr: &mut u8, served_idx: usize) {
        *low_rr = u8::try_from((served_idx + 1) % LOW_TOPIC_COUNT)
            .expect("LOW_TOPIC_COUNT must fit in u8");
    }

    fn inbound_priority_from_topic(topic: Topic) -> Priority {
        match topic {
            Topic::Consensus | Topic::ConsensusPayload | Topic::ConsensusChunk | Topic::Control => {
                Priority::High
            }
            Topic::BlockSync
            | Topic::TxGossip
            | Topic::TxGossipRestricted
            | Topic::PeerGossip
            | Topic::TrustGossip
            | Topic::Health
            | Topic::Other => Priority::Low,
        }
    }

    fn try_recv_low_rr<T>(
        low_rr: &mut u8,
        lo_block_sync_rx: &mut post_channel::Receiver<T>,
        lo_tx_gossip_rx: &mut post_channel::Receiver<T>,
        lo_peer_gossip_rx: &mut post_channel::Receiver<T>,
        lo_health_rx: &mut post_channel::Receiver<T>,
        lo_other_rx: &mut post_channel::Receiver<T>,
    ) -> Option<(LowTopic, T)> {
        for offset in 0..LOW_TOPIC_COUNT {
            let idx = ((*low_rr as usize) + offset) % LOW_TOPIC_COUNT;
            let msg = match idx {
                0 => lo_block_sync_rx.try_recv_now(),
                1 => lo_tx_gossip_rx.try_recv_now(),
                2 => lo_peer_gossip_rx.try_recv_now(),
                3 => lo_health_rx.try_recv_now(),
                _ => lo_other_rx.try_recv_now(),
            };
            if let Some(msg) = msg {
                let topic = match idx {
                    0 => LowTopic::BlockSync,
                    1 => LowTopic::TxGossip,
                    2 => LowTopic::PeerGossip,
                    3 => LowTopic::Health,
                    _ => LowTopic::Other,
                };
                bump_low_rr(low_rr, idx);
                return Some((topic, msg));
            }
        }
        None
    }

    fn maybe_take_low_after_hi<T>(
        hi_budget: &mut u8,
        low_rr: &mut u8,
        lo_block_sync_rx: &mut post_channel::Receiver<T>,
        lo_tx_gossip_rx: &mut post_channel::Receiver<T>,
        lo_peer_gossip_rx: &mut post_channel::Receiver<T>,
        lo_health_rx: &mut post_channel::Receiver<T>,
        lo_other_rx: &mut post_channel::Receiver<T>,
    ) -> Option<(LowTopic, T)> {
        if *hi_budget != 0 {
            return None;
        }
        if let Some(msg) = try_recv_low_rr(
            low_rr,
            lo_block_sync_rx,
            lo_tx_gossip_rx,
            lo_peer_gossip_rx,
            lo_health_rx,
            lo_other_rx,
        ) {
            *hi_budget = HI_BUDGET_RESET;
            return Some(msg);
        }
        *hi_budget = HI_BUDGET_FALLBACK;
        None
    }

    async fn recv_low_rr<T>(
        low_rr: &mut u8,
        lo_block_sync_rx: &mut post_channel::Receiver<T>,
        lo_tx_gossip_rx: &mut post_channel::Receiver<T>,
        lo_peer_gossip_rx: &mut post_channel::Receiver<T>,
        lo_health_rx: &mut post_channel::Receiver<T>,
        lo_other_rx: &mut post_channel::Receiver<T>,
    ) -> Option<(LowTopic, T)> {
        poll_fn(|cx| {
            let mut closed = 0;
            for offset in 0..LOW_TOPIC_COUNT {
                let idx = ((*low_rr as usize) + offset) % LOW_TOPIC_COUNT;
                let poll = match idx {
                    0 => lo_block_sync_rx.poll_recv(cx),
                    1 => lo_tx_gossip_rx.poll_recv(cx),
                    2 => lo_peer_gossip_rx.poll_recv(cx),
                    3 => lo_health_rx.poll_recv(cx),
                    _ => lo_other_rx.poll_recv(cx),
                };
                match poll {
                    Poll::Ready(Some(msg)) => {
                        let topic = match idx {
                            0 => LowTopic::BlockSync,
                            1 => LowTopic::TxGossip,
                            2 => LowTopic::PeerGossip,
                            3 => LowTopic::Health,
                            _ => LowTopic::Other,
                        };
                        bump_low_rr(low_rr, idx);
                        return Poll::Ready(Some((topic, msg)));
                    }
                    Poll::Ready(None) => closed += 1,
                    Poll::Pending => {}
                }
            }
            if closed == LOW_TOPIC_COUNT {
                Poll::Ready(None)
            } else {
                Poll::Pending
            }
        })
        .await
    }

    /// Peer task.
    #[allow(clippy::too_many_lines)]
    #[log(skip_all, fields(connection = &peer.log_description(), conn_id = peer.connection_id(), peer, disambiguator))]
    pub(super) async fn run<T: Pload + ClassifyTopic, K: Kex, E: Enc, P: Entrypoint<K, E>>(
        RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
        }: RunPeerArgs<T, P>,
    ) {
        let conn_id = peer.connection_id();
        let mut peer_id = None;

        iroha_logger::trace!("Peer created");

        // Insure proper termination from every execution path.
        async {
            // Try to do handshake process
            #[cfg(feature = "noise_handshake")]
            iroha_logger::debug!("noise_handshake feature enabled: deriving session key via Noise XX");
            let hs_start = Instant::now();
            let ready_peer = match tokio::time::timeout(idle_timeout, peer.handshake()).await {
                Ok(Ok(ready)) => {
                    let ms = u64::try_from(hs_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    observe_handshake_ms(ms);
                    ready
                }
                Ok(Err(error)) => {
                    iroha_logger::warn!(?error, "Failure during handshake.");
                    HANDSHAKE_FAILURES.fetch_add(1, Ordering::Relaxed);
                    match error {
                        Error::HandshakeBadPreface => { HSE_PREFACE.fetch_add(1, Ordering::Relaxed); },
                        Error::SymmetricEncryption(_) => { HSE_DECRYPT.fetch_add(1, Ordering::Relaxed); },
                        Error::NoritoCodec(_) => { HSE_CODEC.fetch_add(1, Ordering::Relaxed); },
                        Error::Io(_) => { HSE_IO.fetch_add(1, Ordering::Relaxed); },
                        _ => { HSE_OTHER.fetch_add(1, Ordering::Relaxed); },
                    }
                    return;
                },
                Err(_) => {
                    iroha_logger::warn!(timeout=?idle_timeout, "Other peer has been idle during handshake");
                    HANDSHAKE_FAILURES.fetch_add(1, Ordering::Relaxed);
                    HSE_TIMEOUT.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let Ready {
                peer: new_peer_id,
                connection:
                    Connection {
                        read,
                        write,
                        read_low,
                        write_low,
                        quic,
                        id: connection_id,
                        ..
                    },
                cryptographer,
                relay_role,
                scion_supported,
                trust_gossip,
            } = ready_peer;
            let peer_id = peer_id.insert(new_peer_id);

            let disambiguator = cryptographer.disambiguator;

            tracing::Span::current().record("peer", peer_id.to_string());
            tracing::Span::current().record("disambiguator", disambiguator);

            // Create per-topic substreams (bounded or unbounded depending on feature).
            let (hi_consensus_tx, mut hi_consensus_rx) = post_channel::channel(post_capacity);
            let (hi_consensus_payload_tx, mut hi_consensus_payload_rx) =
                post_channel::channel(post_capacity);
            let (hi_consensus_chunk_tx, mut hi_consensus_chunk_rx) =
                post_channel::channel(post_capacity);
            let (hi_control_tx, mut hi_control_rx) = post_channel::channel(post_capacity);
            let (lo_block_sync_tx, mut lo_block_sync_rx) = post_channel::channel(post_capacity);
            let (lo_tx_gossip_tx, mut lo_tx_gossip_rx) = post_channel::channel(post_capacity);
            let (lo_peer_gossip_tx, mut lo_peer_gossip_rx) = post_channel::channel(post_capacity);
            let (lo_health_tx, mut lo_health_rx) = post_channel::channel(post_capacity);
            let (lo_other_tx, mut lo_other_rx) = post_channel::channel(post_capacity);
            let (peer_message_sender, peer_message_receiver) = oneshot::channel();
            let ready_peer_handle = handles::PeerHandle {
                senders: handles::TopicSenders {
                    hi_consensus: hi_consensus_tx,
                    hi_consensus_payload: hi_consensus_payload_tx,
                    hi_consensus_chunk: hi_consensus_chunk_tx,
                    hi_control: hi_control_tx,
                    lo_block_sync: lo_block_sync_tx,
                    lo_tx_gossip: lo_tx_gossip_tx,
                    lo_peer_gossip: lo_peer_gossip_tx,
                    lo_health: lo_health_tx,
                    lo_other: lo_other_tx,
                },
            };
            if service_message_sender
                .send(ServiceMessage::Connected(Connected {
                    connection_id,
                    peer: peer_id.clone(),
                    ready_peer_handle,
                    peer_message_sender,
                    disambiguator,
                    relay_role,
                    scion_supported,
                    trust_gossip,
                }))
                .await
                .is_err()
            {
                iroha_logger::error!(
                    "Peer is ready, but network dropped connection sender."
                );
                return;
            }
            let Ok(peer_message_senders) = peer_message_receiver.await else {
                // NOTE: this is not considered as error, because network might decide not to connect peer.
                iroha_logger::debug!(
                    "Network decide not to connect peer."
                );
                return;
            };

            iroha_logger::trace!("Peer connected");

            let mut message_reader = MessageReader::new(read, cryptographer.clone(), max_frame_bytes);
            let mut message_reader_low =
                read_low.map(|read| MessageReader::new(read, cryptographer.clone(), max_frame_bytes));
            // Sampler for repeated read/parse errors to avoid log floods from malformed peers
            let mut read_err_sampler = LogSampler::new();
            let mut malformed_payload_sampler = LogSampler::new();
            let mut recv_backpressure_sampler = LogSampler::new();
            let mut message_sender_hi = MessageSender::new(write, cryptographer.clone(), max_frame_bytes);
            let mut message_sender_low =
                write_low.map(|write| MessageSender::new(write, cryptographer.clone(), max_frame_bytes));

            #[cfg(feature = "quic")]
            let mut datagram_sender: Option<DatagramSender<E>> = None;
            #[cfg(not(feature = "quic"))]
            let datagram_sender: Option<DatagramSender<E>> = None;
            let mut datagram_receiver: Option<DatagramReceiver<E, T>> = None;
            #[cfg(feature = "quic")]
            if quic_datagrams_enabled {
                    if let Some(conn) = quic.clone() {
                        // Receiver is always safe to enable when datagrams are configured locally.
                        datagram_receiver = Some(QuicDatagramReceiver::<E, T>::new(
                            conn.clone(),
                            cryptographer.clone(),
                            max_frame_bytes,
                        ));
                    // Sender requires that the peer negotiated datagram support.
                    if conn.max_datagram_size().is_some() && quic_datagram_max_payload_bytes > 0 {
                        datagram_sender = Some(QuicDatagramSender::new(
                            conn,
                            cryptographer.clone(),
                            max_frame_bytes,
                            quic_datagram_max_payload_bytes,
                        ));
                    }
                }
            }
            #[cfg(not(feature = "quic"))]
            let _ = (
                &datagram_sender,
                quic_datagrams_enabled,
                quic_datagram_max_payload_bytes,
                quic,
            );

            let mut idle_interval = tokio::time::interval_at(Instant::now() + idle_timeout, idle_timeout);
            let mut ping_interval = tokio::time::interval_at(Instant::now() + idle_timeout / 2, idle_timeout / 2);

            // Fairness scheduler: opportunistically service one low-priority topic
            // after processing a burst of high-priority posts. This avoids starving
            // low topics during sustained consensus traffic.
            let mut hi_budget: u8 = HI_BUDGET_RESET;
            let mut low_rr: u8 = 0;
            let mut hi_control_burst: u8 = 0;
            let mut hi_consensus_burst: u8 = 0;
            let mut hi_payload_burst: u8 = 0;
            let mut malformed_payload_streak_hi: u32 = 0;
            let mut malformed_payload_streak_low: u32 = 0;

            loop {
                if let Some((topic, msg)) = maybe_take_low_after_hi(
                    &mut hi_budget,
                    &mut low_rr,
                    &mut lo_block_sync_rx,
                    &mut lo_tx_gossip_rx,
                    &mut lo_peer_gossip_rx,
                    &mut lo_health_rx,
                    &mut lo_other_rx,
                ) {
                    iroha_logger::trace!("Post message ({})", low_topic_label(topic));
                    #[cfg(feature = "quic")]
                    let sent_datagram = {
                        let net_topic = msg.topic();
                        if net_topic.is_best_effort() {
                            if let Some(sender) = datagram_sender.as_mut() {
                                match sender.try_send(&msg) {
                                    Ok(DatagramSend::Sent { .. }) => true,
                                    Ok(DatagramSend::Unsupported | DatagramSend::Disabled) => {
                                        datagram_sender = None;
                                        false
                                    }
                                    Ok(DatagramSend::TooLarge) => false,
                                    Err(error) => {
                                        iroha_logger::error!(
                                            %error,
                                            "Failed to send peer datagram."
                                        );
                                        break;
                                    }
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };
                    #[cfg(not(feature = "quic"))]
                    let sent_datagram = false;
                    if !sent_datagram {
                        let prepared = if let Some(sender) = message_sender_low.as_mut() {
                            sender.prepare_message(&Message::Data(msg), Priority::Low)
                        } else {
                            message_sender_hi.prepare_message(&Message::Data(msg), Priority::Low)
                        };
                        if let Err(error) = prepared {
                            iroha_logger::error!(%error, "Failed to encrypt message.");
                            break;
                        }
                    }
                    continue;
                }

                // Drain additional ready outbound posts without awaiting, so that a burst of
                // queued messages can be coalesced into fewer encrypted frames before the next
                // `message_sender.send()` step. This reduces per-connection frame rate and tokio
                // I/O driver churn under load.
                let mut drained_hi = 0usize;
                while drained_hi < OUTBOUND_DRAIN_HI_MAX && hi_budget > 0 {
                    let Some((topic, msg)) = try_recv_high_fair(
                        &mut hi_control_burst,
                        &mut hi_consensus_burst,
                        &mut hi_payload_burst,
                        &mut hi_control_rx,
                        &mut hi_consensus_rx,
                        &mut hi_consensus_payload_rx,
                        &mut hi_consensus_chunk_rx,
                    ) else {
                        break;
                    };
                    iroha_logger::trace!("Post message ({}/drain)", high_topic_label(topic));
                    if let Err(error) =
                        message_sender_hi.prepare_message(&Message::Data(msg), Priority::High)
                    {
                        iroha_logger::error!(%error, "Failed to encrypt message.");
                        break;
                    }
                    hi_budget = hi_budget.saturating_sub(1);
                    drained_hi = drained_hi.saturating_add(1);
                }

                let mut drained_lo = 0usize;
                while drained_lo < OUTBOUND_DRAIN_LO_MAX {
                    let Some((topic, m)) = try_recv_low_rr(
                        &mut low_rr,
                        &mut lo_block_sync_rx,
                        &mut lo_tx_gossip_rx,
                        &mut lo_peer_gossip_rx,
                        &mut lo_health_rx,
                        &mut lo_other_rx,
                    ) else {
                        break;
                    };
                    iroha_logger::trace!("Post message ({}/drain)", low_topic_label(topic));
                    #[cfg(feature = "quic")]
                    let sent_datagram = {
                        let net_topic = m.topic();
                        if net_topic.is_best_effort() {
                            if let Some(sender) = datagram_sender.as_mut() {
                                match sender.try_send(&m) {
                                    Ok(DatagramSend::Sent { .. }) => true,
                                    Ok(DatagramSend::Unsupported | DatagramSend::Disabled) => {
                                        datagram_sender = None;
                                        false
                                    }
                                    Ok(DatagramSend::TooLarge) => false,
                                    Err(error) => {
                                        iroha_logger::error!(
                                            %error,
                                            "Failed to send peer datagram."
                                        );
                                        break;
                                    }
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };
                    #[cfg(not(feature = "quic"))]
                    let sent_datagram = false;
                    if !sent_datagram {
                        let prepared = if let Some(sender) = message_sender_low.as_mut() {
                            sender.prepare_message(&Message::Data(m), Priority::Low)
                        } else {
                            message_sender_hi.prepare_message(&Message::Data(m), Priority::Low)
                        };
                        if let Err(error) = prepared {
                            iroha_logger::error!(%error, "Failed to encrypt message.");
                            break;
                        }
                    }
                    hi_budget = HI_BUDGET_RESET;
                    drained_lo = drained_lo.saturating_add(1);
                }

                tokio::select! {
                    // High-priority topics first (budgeted to avoid starvation).
                    _ = ping_interval.tick() => {
                        iroha_logger::trace!(
                            ping_period=?ping_interval.period(),
                            "The connection has been idle, pinging to check if it's alive"
                        );
                        if let Err(error) =
                            message_sender_hi.prepare_message(&Message::<T>::Ping, Priority::High)
                        {
                            iroha_logger::error!(%error, "Failed to encrypt message.");
                            break;
                        }
                    }
                    _ = idle_interval.tick() => {
                        iroha_logger::error!(
                            timeout=?idle_interval.period(),
                            "Didn't receive anything from the peer within given timeout, abandoning this connection"
                        );
                        break;
                    }
                    msg = hi_control_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg {
                            note_high_topic_served(
                                &mut hi_control_burst,
                                &mut hi_consensus_burst,
                                &mut hi_payload_burst,
                                HighTopic::Control,
                            );
                            iroha_logger::trace!("Post message ({})", high_topic_label(HighTopic::Control));
                            if let Err(error) = message_sender_hi.prepare_message(&Message::Data(m), Priority::High) {
                                iroha_logger::error!(%error, "Failed to encrypt message.");
                                break;
                            }
                            hi_budget = hi_budget.saturating_sub(1);
                        }
                    }
                    msg = hi_consensus_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg {
                            note_high_topic_served(
                                &mut hi_control_burst,
                                &mut hi_consensus_burst,
                                &mut hi_payload_burst,
                                HighTopic::Consensus,
                            );
                            iroha_logger::trace!("Post message ({})", high_topic_label(HighTopic::Consensus));
                            if let Err(error) = message_sender_hi.prepare_message(&Message::Data(m), Priority::High) {
                                iroha_logger::error!(%error, "Failed to encrypt message.");
                                break;
                            }
                            hi_budget = hi_budget.saturating_sub(1);
                        }
                    }
                    msg = hi_consensus_payload_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg {
                            note_high_topic_served(
                                &mut hi_control_burst,
                                &mut hi_consensus_burst,
                                &mut hi_payload_burst,
                                HighTopic::ConsensusPayload,
                            );
                            iroha_logger::trace!("Post message ({})", high_topic_label(HighTopic::ConsensusPayload));
                            if let Err(error) = message_sender_hi.prepare_message(&Message::Data(m), Priority::High) {
                                iroha_logger::error!(%error, "Failed to encrypt message.");
                                break;
                            }
                            hi_budget = hi_budget.saturating_sub(1);
                        }
                    }
                    msg = hi_consensus_chunk_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg {
                            note_high_topic_served(
                                &mut hi_control_burst,
                                &mut hi_consensus_burst,
                                &mut hi_payload_burst,
                                HighTopic::ConsensusChunk,
                            );
                            iroha_logger::trace!("Post message ({})", high_topic_label(HighTopic::ConsensusChunk));
                            if let Err(error) = message_sender_hi.prepare_message(&Message::Data(m), Priority::High) {
                                iroha_logger::error!(%error, "Failed to encrypt message.");
                                break;
                            }
                            hi_budget = hi_budget.saturating_sub(1);
                        }
                    }
                    // Low-priority topics
                    low = recv_low_rr(
                        &mut low_rr,
                        &mut lo_block_sync_rx,
                        &mut lo_tx_gossip_rx,
                        &mut lo_peer_gossip_rx,
                        &mut lo_health_rx,
                        &mut lo_other_rx,
	                    ) => {
	                        if let Some((topic, msg)) = low {
	                            iroha_logger::trace!("Post message ({})", low_topic_label(topic));
	                            #[cfg(feature = "quic")]
	                            let sent_datagram = {
	                                let net_topic = msg.topic();
	                                if net_topic.is_best_effort() {
	                                    if let Some(sender) = datagram_sender.as_mut() {
	                                        match sender.try_send(&msg) {
	                                            Ok(DatagramSend::Sent { .. }) => true,
	                                            Ok(DatagramSend::Unsupported | DatagramSend::Disabled) => {
	                                                datagram_sender = None;
	                                                false
	                                            }
	                                            Ok(DatagramSend::TooLarge) => false,
	                                            Err(error) => {
	                                                iroha_logger::error!(
	                                                    %error,
	                                                    "Failed to send peer datagram."
	                                                );
	                                                break;
	                                            }
	                                        }
	                                    } else {
	                                        false
	                                    }
	                                } else {
	                                    false
	                                }
	                            };
	                            #[cfg(not(feature = "quic"))]
	                            let sent_datagram = false;
	                            if !sent_datagram {
	                                let prepared = if let Some(sender) = message_sender_low.as_mut() {
	                                    sender.prepare_message(&Message::Data(msg), Priority::Low)
	                                } else {
                                    message_sender_hi.prepare_message(&Message::Data(msg), Priority::Low)
                                };
                                if let Err(error) = prepared {
                                    iroha_logger::error!(%error, "Failed to encrypt message.");
                                    break;
                                }
                            }
                            hi_budget = HI_BUDGET_RESET;
                        }
                    }
                    datagram = recv_best_effort_datagram::<E, T>(&mut datagram_receiver), if datagram_receiver.is_some() => {
                        match datagram {
                            Ok((payload, encoded_len)) => {
                                let topic = payload.topic();
                                if !topic.is_best_effort() {
                                    iroha_logger::debug!(
                                        conn_id,
                                        ?topic,
                                        "Dropping non-best-effort payload received via QUIC datagram"
                                    );
                                    continue;
                                }
                                let peer_message = PeerMessage {
                                    peer: peer_id.clone(),
                                    payload,
                                    payload_bytes: encoded_len,
                                };
                                match peer_message_senders.low.try_send(peer_message) {
                                    Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                        // Best-effort delivery: drop when the network can't keep up.
                                    }
                                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                        iroha_logger::error!(
                                            "Network dropped peer message channel (datagram)."
                                        );
                                        break;
                                    }
                                }
                                idle_interval.reset();
                                ping_interval.reset();
                            }
                            Err(Error::Io(_)) => {
                                iroha_logger::debug!(
                                    conn_id,
                                    "QUIC datagram receive failed; disabling datagram receiver"
                                );
                                datagram_receiver = None;
                            }
                            Err(error) => {
                                iroha_logger::debug!(
                                    conn_id,
                                    %error,
                                    "Dropping malformed QUIC datagram payload"
                                );
                            }
                        }
                    }
                    msg = message_reader.read_message() => {
                        let (message, encoded_len): (Message<T>, usize) = match msg {
                            Ok(Some((msg, encoded_len))) => {
                                malformed_payload_streak_hi = 0;
                                (msg, encoded_len)
                            }
                            Ok(None) => {
                                iroha_logger::debug!("Peer send whole message and close connection");
                                break;
                            }
                            Err(Error::MalformedPayloadFrame) => {
                                let disconnect =
                                    note_malformed_payload_frame(&mut malformed_payload_streak_hi);
                                if let Some(suppressed) = malformed_payload_sampler
                                    .should_log(tokio::time::Duration::from_millis(500))
                                {
                                    iroha_logger::warn!(
                                        peer = %peer_id,
                                        conn_id,
                                        stream = "high",
                                        malformed_payload_streak = malformed_payload_streak_hi,
                                        threshold = MALFORMED_PAYLOAD_FRAME_THRESHOLD,
                                        suppressed,
                                        "Dropped malformed decrypted peer payload frame"
                                    );
                                }
                                if disconnect {
                                    iroha_logger::error!(
                                        peer = %peer_id,
                                        conn_id,
                                        stream = "high",
                                        malformed_payload_streak = malformed_payload_streak_hi,
                                        "Disconnecting peer after consecutive malformed decrypted payload frames"
                                    );
                                    break;
                                }
                                idle_interval.reset();
                                ping_interval.reset();
                                continue;
                            }
                            Err(error) => {
                                if let Some(supp) = read_err_sampler.should_log(tokio::time::Duration::from_millis(500)) {
                                    iroha_logger::error!(
                                        ?error,
                                        suppressed=supp,
                                        "Error while reading message from peer."
                                    );
                                }
                                break;
                            }
                        };
                        match message {
                            Message::Ping => {
                                iroha_logger::trace!("Received peer ping");
                                if let Err(error) =
                                    message_sender_hi.prepare_message(&Message::<T>::Pong, Priority::High)
                                {
                                    iroha_logger::error!(%error, "Failed to encrypt message.");
                                    break;
                                }
                            },
                            Message::Pong => {
                                iroha_logger::trace!("Received peer pong");
                            }
                            Message::Data(payload) => {
                                iroha_logger::trace!("Received peer message");
                                let topic = payload.topic();
                                let inbound_priority = inbound_priority_from_topic(topic);
                                let peer_message = PeerMessage {
                                    peer: peer_id.clone(),
                                    payload,
                                    payload_bytes: encoded_len,
                                };
                                let send_start = Instant::now();
                                let send_result = match inbound_priority {
                                    Priority::High => peer_message_senders.high.send(peer_message).await,
                                    Priority::Low => peer_message_senders.low.send(peer_message).await,
                                };
                                let send_wait_ms =
                                    u64::try_from(send_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                                if matches!(inbound_priority, Priority::High)
                                    && send_wait_ms >= INBOUND_SEND_WARN_MS
                                {
                                    if let Some(suppressed) = recv_backpressure_sampler
                                        .should_log(tokio::time::Duration::from_millis(500))
                                    {
                                        iroha_logger::warn!(
                                            peer = %peer_id,
                                            conn_id,
                                            ?topic,
                                            wait_ms = send_wait_ms,
                                            payload_bytes = encoded_len,
                                            suppressed,
                                            "Inbound high-priority frame waited on dispatch channel"
                                        );
                                    }
                                }
                                if send_result.is_err() {
                                    iroha_logger::error!("Network dropped peer message channel.");
                                    break;
                                }
                            }
                        }
                        // Reset idle and ping timeout as peer received message from another peer
                        idle_interval.reset();
                        ping_interval.reset();
                    }
                    msg = async {
                        let reader = message_reader_low.as_mut().expect("guarded by is_some");
                        reader.read_message().await
                    }, if message_reader_low.is_some() => {
                        let (message, encoded_len): (Message<T>, usize) = match msg {
                            Ok(Some((msg, encoded_len))) => {
                                malformed_payload_streak_low = 0;
                                (msg, encoded_len)
                            }
                            Ok(None) => {
                                iroha_logger::debug!("Peer closed low-priority stream");
                                message_reader_low = None;
                                continue;
                            }
                            Err(Error::MalformedPayloadFrame) => {
                                let disconnect =
                                    note_malformed_payload_frame(&mut malformed_payload_streak_low);
                                if let Some(suppressed) = malformed_payload_sampler
                                    .should_log(tokio::time::Duration::from_millis(500))
                                {
                                    iroha_logger::warn!(
                                        peer = %peer_id,
                                        conn_id,
                                        stream = "low",
                                        malformed_payload_streak = malformed_payload_streak_low,
                                        threshold = MALFORMED_PAYLOAD_FRAME_THRESHOLD,
                                        suppressed,
                                        "Dropped malformed decrypted peer payload frame"
                                    );
                                }
                                if disconnect {
                                    iroha_logger::error!(
                                        peer = %peer_id,
                                        conn_id,
                                        stream = "low",
                                        malformed_payload_streak = malformed_payload_streak_low,
                                        "Disconnecting peer after consecutive malformed decrypted payload frames"
                                    );
                                    break;
                                }
                                idle_interval.reset();
                                ping_interval.reset();
                                continue;
                            }
                            Err(error) => {
                                if let Some(supp) = read_err_sampler.should_log(tokio::time::Duration::from_millis(500)) {
                                    iroha_logger::debug!(
                                        ?error,
                                        suppressed=supp,
                                        "Error while reading message from peer (low stream)."
                                    );
                                }
                                message_reader_low = None;
                                continue;
                            }
                        };
                        match message {
                            Message::Ping => {
                                iroha_logger::trace!("Received peer ping (low stream)");
                                if let Err(error) =
                                    message_sender_hi.prepare_message(&Message::<T>::Pong, Priority::High)
                                {
                                    iroha_logger::error!(%error, "Failed to encrypt message.");
                                    break;
                                }
                            },
                            Message::Pong => {
                                iroha_logger::trace!("Received peer pong (low stream)");
                            }
                            Message::Data(payload) => {
                                iroha_logger::trace!("Received peer message (low stream)");
                                let topic = payload.topic();
                                let inbound_priority = inbound_priority_from_topic(topic);
                                let peer_message = PeerMessage {
                                    peer: peer_id.clone(),
                                    payload,
                                    payload_bytes: encoded_len,
                                };
                                let send_start = Instant::now();
                                let send_result = match inbound_priority {
                                    Priority::High => peer_message_senders.high.send(peer_message).await,
                                    Priority::Low => peer_message_senders.low.send(peer_message).await,
                                };
                                let send_wait_ms =
                                    u64::try_from(send_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                                if matches!(inbound_priority, Priority::High)
                                    && send_wait_ms >= INBOUND_SEND_WARN_MS
                                {
                                    if let Some(suppressed) = recv_backpressure_sampler
                                        .should_log(tokio::time::Duration::from_millis(500))
                                    {
                                        iroha_logger::warn!(
                                            peer = %peer_id,
                                            conn_id,
                                            ?topic,
                                            wait_ms = send_wait_ms,
                                            payload_bytes = encoded_len,
                                            suppressed,
                                            "Inbound high-priority frame waited on dispatch channel"
                                        );
                                    }
                                }
                                if send_result.is_err() {
                                    iroha_logger::error!("Network dropped peer message channel.");
                                    break;
                                }
                            }
                        }
                        // Reset idle and ping timeout as peer received message from another peer
                        idle_interval.reset();
                        ping_interval.reset();
                    }
                    // `send()` is safe to be cancelled: it won't advance the queue or write
                    // anything if another branch completes first.
                    result = message_sender_hi.send(), if message_sender_hi.ready() => {
                        if let Err(error) = result {
                            iroha_logger::error!(%error, "Failed to send message to peer (hi stream).");
                            break;
                        }
                    }
                    result = async {
                        let sender = message_sender_low.as_mut().expect("ready implies sender present");
                        sender.send().await
                    }, if message_sender_low.as_ref().is_some_and(MessageSender::ready) => {
                        if let Err(error) = result {
                            iroha_logger::warn!(%error, "Failed to send message to peer (low stream); falling back to hi stream");
                            message_sender_low = None;
                        }
                    }
                    else => break,
                }

                // Opportunistically allow a low-priority message through after bursts of high-priority posts.
                if hi_budget == 0 {
                    if let Some((topic, m)) = try_recv_low_rr(
                        &mut low_rr,
                        &mut lo_block_sync_rx,
                        &mut lo_tx_gossip_rx,
                        &mut lo_peer_gossip_rx,
                        &mut lo_health_rx,
                        &mut lo_other_rx,
                    ) {
                        #[cfg(feature = "quic")]
                        let sent_datagram = {
                            let net_topic = m.topic();
                            if net_topic.is_best_effort() {
                                if let Some(sender) = datagram_sender.as_mut() {
                                    match sender.try_send(&m) {
                                        Ok(DatagramSend::Sent { .. }) => true,
                                        Ok(DatagramSend::Unsupported | DatagramSend::Disabled) => {
                                            datagram_sender = None;
                                            false
                                        }
                                        Ok(DatagramSend::TooLarge) => false,
                                        Err(error) => {
                                            iroha_logger::error!(
                                                %error,
                                                "Failed to send peer datagram."
                                            );
                                            break;
                                        }
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        };
                        #[cfg(not(feature = "quic"))]
                        let sent_datagram = false;
                        if !sent_datagram {
                            let prepared = if let Some(sender) = message_sender_low.as_mut() {
                                sender.prepare_message(&Message::Data(m), Priority::Low)
                            } else {
                                message_sender_hi.prepare_message(&Message::Data(m), Priority::Low)
                            };
                            if let Err(error) = prepared {
                                iroha_logger::error!(%error, "Failed to encrypt message.");
                                break;
                            }
                        }
                        hi_budget = HI_BUDGET_RESET;
                        iroha_logger::trace!("Post message ({})", low_topic_label(topic));
                    }
                }
                tokio::task::yield_now().await;
            }
        }.await;

        iroha_logger::debug!("Peer is terminated.");
        let _ = service_message_sender
            .send(ServiceMessage::Terminated(Terminated {
                peer: peer_id,
                conn_id,
            }))
            .await;
    }

    // Traits to unify bounded/unbounded try_recv across feature flags at module scope
    pub(super) trait TryRecvExt<T> {
        fn try_recv_now(&mut self) -> Option<T>;
    }
    impl<T> TryRecvExt<T> for tokio::sync::mpsc::Receiver<T> {
        fn try_recv_now(&mut self) -> Option<T> {
            self.try_recv().ok()
        }
    }

    /// Args to pass inside [`run`] function.
    pub(super) struct RunPeerArgs<T: Pload, P> {
        pub peer: P,
        pub service_message_sender: mpsc::Sender<ServiceMessage<T>>,
        pub idle_timeout: Duration,
        pub post_capacity: usize,
        #[allow(dead_code)]
        pub max_frame_bytes: usize,
        pub quic_datagrams_enabled: bool,
        pub quic_datagram_max_payload_bytes: usize,
    }

    /// Trait for peer stages that might be used as starting point for peer's [`run`] function.
    pub(super) trait Entrypoint<K: Kex, E: Enc>: Handshake<K, E> + Send + 'static {
        fn connection_id(&self) -> ConnectionId;

        /// Debug description, used for logging
        fn log_description(&self) -> String;
    }

    impl<K: Kex, E: Enc> Entrypoint<K, E> for Connecting {
        fn connection_id(&self) -> ConnectionId {
            self.connection_id
        }

        fn log_description(&self) -> String {
            format!("outgoing to {}", self.peer_addr)
        }
    }

    impl<K: Kex, E: Enc> Entrypoint<K, E> for ConnectedFrom {
        fn connection_id(&self) -> ConnectionId {
            self.connection.id
        }

        fn log_description(&self) -> String {
            #[allow(clippy::option_if_let_else)]
            match self.connection.remote_addr {
                None => "incoming".to_owned(),
                Some(remote_addr) => {
                    // In case of incoming connection,
                    // only host will have some meaningful value.
                    // Port will have some random value chosen only for this connection.
                    format!("incoming from {}", remote_addr.ip())
                }
            }
        }
    }

    /// Cancellation-safe way to read messages from tcp stream.
    ///
    /// This reader supports "batched frames": a single encrypted frame may
    /// contain multiple Norito-framed messages concatenated back-to-back.
    /// This reduces the encrypted frame rate and therefore lowers Tokio IO
    /// driver overhead under high message volumes (e.g. `NPoS` consensus).
    struct MessageReader<E: Enc, M: Pload> {
        read: Box<dyn AsyncRead + Send + Unpin>,
        buffer: bytes::BytesMut,
        decrypted: Vec<u8>,
        decode_scratch: Vec<u8>,
        cryptographer: Cryptographer<E>,
        pending: VecDeque<(M, usize)>,
        framed_schema: [u8; 16],
        framed_padding: usize,
        max_frame_bytes: usize,
    }

    impl<E: Enc, M: Pload> MessageReader<E, M> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();

        fn new(
            read: Box<dyn AsyncRead + Send + Unpin>,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
        ) -> Self {
            let prealloc = max_frame_bytes.min(DEFAULT_MESSAGE_PREALLOC_CAP);
            let capacity = DEFAULT_BUFFER_CAPACITY.max(prealloc.saturating_add(Self::U32_SIZE));
            let decrypt_capacity = DEFAULT_BUFFER_CAPACITY.max(prealloc);
            let align = core::mem::align_of::<ncore::Archived<M>>();
            let framed_padding = if align <= 1 {
                0
            } else {
                let rem = ncore::Header::SIZE % align;
                if rem == 0 { 0 } else { align - rem }
            };
            Self {
                read,
                cryptographer,
                buffer: BytesMut::with_capacity(capacity),
                decrypted: Vec::with_capacity(decrypt_capacity),
                decode_scratch: Vec::new(),
                pending: VecDeque::new(),
                framed_schema: <M as ncore::NoritoSerialize>::schema_hash(),
                framed_padding,
                max_frame_bytes,
            }
        }

        fn copy_to_aligned_scratch<'a>(
            scratch: &'a mut Vec<u8>,
            src: &[u8],
            align: usize,
        ) -> &'a [u8] {
            debug_assert!(align.is_power_of_two());
            let len = src.len();
            if len == 0 || align <= 1 {
                scratch.clear();
                scratch.extend_from_slice(src);
                return scratch.as_slice();
            }
            let extra = align.saturating_sub(1);
            let needed = len.saturating_add(extra);
            if scratch.len() < needed {
                scratch.resize(needed, 0);
            }
            let base = scratch.as_ptr() as usize;
            let misalignment = base % align;
            let offset = if misalignment == 0 {
                0
            } else {
                align - misalignment
            };
            let end = offset.saturating_add(len);
            scratch[offset..end].copy_from_slice(src);
            &scratch[offset..end]
        }

        fn reserve_for_frame(&mut self) -> Result<(), Error> {
            if self.buffer.len() < Self::U32_SIZE {
                return Ok(());
            }
            let mut prefix = &self.buffer[..];
            let size = prefix.get_u32() as usize;
            if size > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            let needed = size.saturating_add(Self::U32_SIZE);
            if self.buffer.capacity() < needed {
                self.buffer
                    .reserve(needed.saturating_sub(self.buffer.len()));
            }
            Ok(())
        }

        /// Read message by first reading it's size as u32 and then rest of the message
        ///
        /// # Errors
        /// - Fail in case reading from stream fails
        /// - Connection is closed by there is still unfinished message in buffer
        /// - Forward errors from [`Self::parse_message`]
        async fn read_message(&mut self) -> Result<Option<(M, usize)>, Error> {
            if let Some(msg) = self.pending.pop_front() {
                return Ok(Some(msg));
            }
            loop {
                // Try to get full message
                if self.parse_next_encrypted_frame()? {
                    if let Some(msg) = self.pending.pop_front() {
                        return Ok(Some(msg));
                    }
                }
                self.reserve_for_frame()?;
                if 0 == self.read.read_buf(&mut self.buffer).await? {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    return Err(Error::ConnectionResetByPeer);
                }
            }
        }

        /// Parse the next encrypted frame from `self.buffer` and enqueue decoded messages.
        ///
        /// # Errors
        /// - Fail to decrypt message
        /// - Fail to decode the encrypted envelope
        fn parse_next_encrypted_frame(&mut self) -> Result<bool, Error> {
            let mut buf = &self.buffer[..];
            if buf.remaining() < Self::U32_SIZE {
                // Not enough data to read u32
                return Ok(false);
            }
            let size = buf.get_u32() as usize;
            if size > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            if buf.remaining() < size {
                // Not enough data to read the whole data
                return Ok(false);
            }

            let data = &buf[..size];
            let parsed = (|| -> Result<VecDeque<(M, usize)>, Error> {
                let decrypted = self.cryptographer.decrypt_into(data, &mut self.decrypted)?;
                let decrypted_len = decrypted.len();
                if decrypted_len == 0 {
                    return Err(Error::MalformedPayloadFrame);
                } else {
                    // Decrypted payload may contain multiple Norito-framed messages.
                    let align = core::mem::align_of::<ncore::Archived<M>>();
                    let mut offset = 0usize;
                    let mut frame_messages = VecDeque::new();
                    while offset < decrypted_len {
                        let remaining = decrypted
                            .get(offset..)
                            .ok_or(Error::MalformedPayloadFrame)?;
                        let frame_len = framed_message_len::<M>(
                            remaining,
                            self.framed_schema,
                            self.framed_padding,
                        )
                        .map_err(|_| Error::MalformedPayloadFrame)?;
                        let frame = remaining
                            .get(..frame_len)
                            .ok_or(Error::MalformedPayloadFrame)?;
                        let misaligned = align > 1
                            && !frame.is_empty()
                            && !((frame.as_ptr() as usize).is_multiple_of(align));
                        let decoded = if misaligned {
                            let aligned = Self::copy_to_aligned_scratch(
                                &mut self.decode_scratch,
                                frame,
                                align,
                            );
                            ncore::decode_from_bytes::<M>(aligned)
                        } else {
                            ncore::decode_from_bytes::<M>(frame)
                        };
                        let decoded = decoded.map_err(|_| Error::MalformedPayloadFrame)?;
                        frame_messages.push_back((decoded, frame_len));
                        offset = offset.saturating_add(frame_len);
                    }
                    if offset != decrypted_len {
                        Err(Error::MalformedPayloadFrame)
                    } else {
                        Ok(frame_messages)
                    }
                }
            })();

            self.buffer.advance(size + Self::U32_SIZE);
            self.pending.extend(parsed?);

            Ok(true)
        }
    }

    struct MessageSender<E: Enc> {
        write: Box<dyn AsyncWrite + Send + Unpin>,
        cryptographer: Cryptographer<E>,
        /// Reusable buffer to encode a single Norito-framed message.
        buffer: Vec<u8>,
        /// Accumulated plaintext bytes for the next high-priority encrypted frame.
        plain_high: Vec<u8>,
        plain_high_msgs: usize,
        plain_high_class: Option<HighBatchClass>,
        /// Accumulated plaintext bytes for the next low-priority encrypted frame.
        plain_low: Vec<u8>,
        plain_low_msgs: usize,
        /// Reusable buffer for encrypted payloads (nonce || ciphertext || tag).
        encrypted: Vec<u8>,
        /// Reusable buffers for framing outbound messages.
        frame_pool: Vec<BytesMut>,
        /// Queues of encrypted high-priority frames by scheduling class.
        queue_high_control: VecDeque<BytesMut>,
        queue_high_consensus: VecDeque<BytesMut>,
        queue_high_consensus_payload: VecDeque<BytesMut>,
        queue_high_consensus_chunk: VecDeque<BytesMut>,
        queue_high_other: VecDeque<BytesMut>,
        /// Queue of encrypted messages waiting to be sent (low priority).
        queue_low: VecDeque<BytesMut>,
        /// In-flight coalesced bytes currently being written to the socket.
        batch: BytesMut,
        batch_offset: usize,
        /// Maximum payload size accepted per encrypted frame
        max_frame_bytes: usize,
        /// Number of consecutive control frames emitted before giving consensus/data a turn.
        high_control_burst: usize,
        /// Number of consecutive consensus frames emitted before giving payload/chunk a turn.
        high_consensus_burst: usize,
        /// Number of consecutive payload frames emitted before giving chunk a turn.
        high_payload_burst: usize,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum HighBatchClass {
        Control,
        Consensus,
        ConsensusPayload,
        ConsensusChunk,
        Other,
    }

    fn classify_high_batch(topic: Topic) -> HighBatchClass {
        match topic {
            Topic::Control => HighBatchClass::Control,
            Topic::Consensus => HighBatchClass::Consensus,
            Topic::ConsensusPayload => HighBatchClass::ConsensusPayload,
            Topic::ConsensusChunk => HighBatchClass::ConsensusChunk,
            _ => HighBatchClass::Other,
        }
    }

    impl<E: Enc> MessageSender<E> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();
        const FRAME_POOL_MAX: usize = 32;
        const MAX_BATCH_FRAMES: usize = 16;
        const MAX_BATCH_BYTES: usize = 64 * 1024;
        const MAX_BATCH_HI_BURST: usize = 4;
        const MAX_BATCH_CONTROL_BURST: usize = 4;
        const MAX_BATCH_CONSENSUS_BURST: usize = 4;
        const MAX_BATCH_PAYLOAD_BURST: usize = 2;
        const MAX_PLAINTEXT_MSGS_HI: usize = 16;
        const MAX_PLAINTEXT_MSGS_LO: usize = 32;
        const MAX_PLAINTEXT_BYTES_HI: usize = 64 * 1024;
        const MAX_PLAINTEXT_BYTES_LO: usize = 256 * 1024;

        fn new(
            write: Box<dyn AsyncWrite + Send + Unpin>,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
        ) -> Self {
            let prealloc = max_frame_bytes.min(DEFAULT_MESSAGE_PREALLOC_CAP);
            let capacity = DEFAULT_BUFFER_CAPACITY.max(prealloc);
            let batch_capacity = capacity.max(Self::MAX_BATCH_BYTES);
            Self {
                write,
                cryptographer,
                buffer: Vec::with_capacity(capacity),
                plain_high: Vec::with_capacity(capacity),
                plain_high_msgs: 0,
                plain_high_class: None,
                plain_low: Vec::with_capacity(capacity),
                plain_low_msgs: 0,
                encrypted: Vec::with_capacity(capacity),
                frame_pool: Vec::new(),
                queue_high_control: VecDeque::new(),
                queue_high_consensus: VecDeque::new(),
                queue_high_consensus_payload: VecDeque::new(),
                queue_high_consensus_chunk: VecDeque::new(),
                queue_high_other: VecDeque::new(),
                queue_low: VecDeque::new(),
                batch: BytesMut::with_capacity(batch_capacity),
                batch_offset: 0,
                max_frame_bytes,
                high_control_burst: 0,
                high_consensus_burst: 0,
                high_payload_burst: 0,
            }
        }

        /// Prepare message for the delivery and put it into the queue to be sent later
        ///
        /// # Errors
        /// - If encryption fail.
        fn prepare_message<T>(&mut self, msg: &T, priority: Priority) -> Result<(), Error>
        where
            T: Pload + ClassifyTopic,
        {
            ncore::to_bytes_in(msg, &mut self.buffer)?;

            let topic = msg.topic();
            let max_plaintext = crate::frame_plaintext_cap(self.max_frame_bytes);
            let msg_len = self.buffer.len();
            if msg_len > max_plaintext {
                return Err(Error::FrameTooLarge);
            }

            match priority {
                Priority::High => {
                    let class = classify_high_batch(topic);
                    // Control traffic should not be delayed behind other high batches.
                    if class == HighBatchClass::Control {
                        self.flush_plain_high()?;
                        self.enqueue_current_buffer(Priority::High, Some(class))?;
                        return Ok(());
                    }

                    if self.plain_high_class.is_some_and(|c| c != class) {
                        self.flush_plain_high()?;
                    }

                    let cap = Self::MAX_PLAINTEXT_BYTES_HI.min(max_plaintext);
                    let would_exceed_bytes = !self.plain_high.is_empty()
                        && self.plain_high.len().saturating_add(msg_len) > cap;
                    let would_exceed_msgs = self.plain_high_msgs >= Self::MAX_PLAINTEXT_MSGS_HI;
                    if would_exceed_bytes || would_exceed_msgs {
                        self.flush_plain_high()?;
                    }

                    // If the single message exceeds the high cap, still send it as its own frame.
                    if self.plain_high.is_empty() && msg_len > cap {
                        self.enqueue_current_buffer(Priority::High, Some(class))?;
                        return Ok(());
                    }

                    if self.plain_high.is_empty() {
                        // A cap-triggered flush clears the current class; restore it for the
                        // new plaintext batch before appending more high-priority bytes.
                        self.plain_high_class = Some(class);
                    }
                    self.plain_high.extend_from_slice(&self.buffer);
                    self.plain_high_msgs = self.plain_high_msgs.saturating_add(1);
                }
                Priority::Low => {
                    let cap = Self::MAX_PLAINTEXT_BYTES_LO.min(max_plaintext);
                    let would_exceed_bytes = !self.plain_low.is_empty()
                        && self.plain_low.len().saturating_add(msg_len) > cap;
                    let would_exceed_msgs = self.plain_low_msgs >= Self::MAX_PLAINTEXT_MSGS_LO;
                    if would_exceed_bytes || would_exceed_msgs {
                        self.flush_plain_low()?;
                    }

                    if self.plain_low.is_empty() && msg_len > cap {
                        self.enqueue_current_buffer(Priority::Low, None)?;
                        return Ok(());
                    }

                    self.plain_low.extend_from_slice(&self.buffer);
                    self.plain_low_msgs = self.plain_low_msgs.saturating_add(1);
                }
            }
            Ok(())
        }

        /// Send bytes of byte-encoded messages piled up in the message queue so far.
        /// On the other side peer will collect bytes and recreate original messages from them.
        ///
        /// # Errors
        /// - If write to `stream` fail.
        async fn send(&mut self) -> Result<(), Error> {
            // Ensure pending plaintext batches are flushed into encrypted frames.
            self.flush_plain_high()?;
            self.flush_plain_low()?;

            if self.batch_offset >= self.batch.len() {
                self.fill_batch();
            }
            if self.batch_offset >= self.batch.len() {
                return Ok(());
            }
            let chunk = &self.batch[self.batch_offset..];
            if !chunk.is_empty() {
                let n = self.write.write(chunk).await?;
                self.batch_offset = self.batch_offset.saturating_add(n);
            }
            if self.batch_offset >= self.batch.len() {
                self.write.flush().await?;
                self.batch.clear();
                self.batch_offset = 0;
            }
            Ok(())
        }

        /// Check if message sender has data ready to be sent.
        fn ready(&self) -> bool {
            self.batch_offset < self.batch.len()
                || !self.plain_high.is_empty()
                || !self.plain_low.is_empty()
                || !self.queue_high_control.is_empty()
                || !self.queue_high_consensus.is_empty()
                || !self.queue_high_consensus_payload.is_empty()
                || !self.queue_high_consensus_chunk.is_empty()
                || !self.queue_high_other.is_empty()
                || !self.queue_low.is_empty()
        }

        fn flush_plain_high(&mut self) -> Result<(), Error> {
            if self.plain_high.is_empty() {
                return Ok(());
            }
            let class = self
                .plain_high_class
                .expect("high plaintext batch must track its scheduling class");
            let plaintext = core::mem::take(&mut self.plain_high);
            match self.enqueue_encrypted(&plaintext, Priority::High, Some(class)) {
                Ok(()) => {
                    let mut plaintext = plaintext;
                    plaintext.clear();
                    self.plain_high = plaintext;
                }
                Err(err) => {
                    self.plain_high = plaintext;
                    return Err(err);
                }
            }
            self.plain_high_msgs = 0;
            self.plain_high_class = None;
            Ok(())
        }

        fn flush_plain_low(&mut self) -> Result<(), Error> {
            if self.plain_low.is_empty() {
                return Ok(());
            }
            let plaintext = core::mem::take(&mut self.plain_low);
            match self.enqueue_encrypted(&plaintext, Priority::Low, None) {
                Ok(()) => {
                    let mut plaintext = plaintext;
                    plaintext.clear();
                    self.plain_low = plaintext;
                }
                Err(err) => {
                    self.plain_low = plaintext;
                    return Err(err);
                }
            }
            self.plain_low_msgs = 0;
            Ok(())
        }

        /// Enqueue currently encoded message bytes from `self.buffer`.
        ///
        /// Keeps the original bytes intact when encryption/framing fails.
        fn enqueue_current_buffer(
            &mut self,
            priority: Priority,
            high_class: Option<HighBatchClass>,
        ) -> Result<(), Error> {
            let plaintext = core::mem::take(&mut self.buffer);
            match self.enqueue_encrypted(&plaintext, priority, high_class) {
                Ok(()) => {
                    let mut plaintext = plaintext;
                    plaintext.clear();
                    self.buffer = plaintext;
                    Ok(())
                }
                Err(err) => {
                    self.buffer = plaintext;
                    Err(err)
                }
            }
        }

        fn enqueue_encrypted(
            &mut self,
            plaintext: &[u8],
            priority: Priority,
            high_class: Option<HighBatchClass>,
        ) -> Result<(), Error> {
            let encrypted = self
                .cryptographer
                .encrypt_into(plaintext, &mut self.encrypted)?;

            let size = encrypted.len();
            if size > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            let mut frame = self.frame_pool.pop().unwrap_or_default();
            frame.clear();
            let needed = size.saturating_add(Self::U32_SIZE);
            if frame.capacity() < needed {
                frame.reserve(needed.saturating_sub(frame.len()));
            }
            #[allow(clippy::cast_possible_truncation)]
            frame.put_u32(size as u32);
            frame.put_slice(encrypted);
            match priority {
                Priority::High => match high_class.unwrap_or(HighBatchClass::Other) {
                    HighBatchClass::Control => self.queue_high_control.push_back(frame),
                    HighBatchClass::Consensus => self.queue_high_consensus.push_back(frame),
                    HighBatchClass::ConsensusPayload => {
                        self.queue_high_consensus_payload.push_back(frame);
                    }
                    HighBatchClass::ConsensusChunk => {
                        self.queue_high_consensus_chunk.push_back(frame);
                    }
                    HighBatchClass::Other => self.queue_high_other.push_back(frame),
                },
                Priority::Low => self.queue_low.push_back(frame),
            }
            Ok(())
        }

        fn next_high_background_class(&self) -> Option<HighBatchClass> {
            if self.high_payload_burst >= Self::MAX_BATCH_PAYLOAD_BURST
                && !self.queue_high_consensus_chunk.is_empty()
            {
                return Some(HighBatchClass::ConsensusChunk);
            }
            if !self.queue_high_consensus_payload.is_empty() {
                return Some(HighBatchClass::ConsensusPayload);
            }
            if !self.queue_high_consensus_chunk.is_empty() {
                return Some(HighBatchClass::ConsensusChunk);
            }
            None
        }

        fn next_high_batch_class(&self) -> Option<HighBatchClass> {
            let non_control_pending = !self.queue_high_consensus.is_empty()
                || !self.queue_high_consensus_payload.is_empty()
                || !self.queue_high_consensus_chunk.is_empty()
                || !self.queue_high_other.is_empty();
            if !self.queue_high_control.is_empty()
                && (!non_control_pending || self.high_control_burst < Self::MAX_BATCH_CONTROL_BURST)
            {
                return Some(HighBatchClass::Control);
            }

            let background_pending = !self.queue_high_consensus_payload.is_empty()
                || !self.queue_high_consensus_chunk.is_empty();
            if !self.queue_high_consensus.is_empty()
                && (!background_pending
                    || self.high_consensus_burst < Self::MAX_BATCH_CONSENSUS_BURST)
            {
                return Some(HighBatchClass::Consensus);
            }

            if let Some(class) = self.next_high_background_class() {
                return Some(class);
            }

            if !self.queue_high_control.is_empty() {
                return Some(HighBatchClass::Control);
            }
            if !self.queue_high_consensus.is_empty() {
                return Some(HighBatchClass::Consensus);
            }
            if !self.queue_high_other.is_empty() {
                return Some(HighBatchClass::Other);
            }
            None
        }

        fn high_queue_len(&self, class: HighBatchClass) -> usize {
            match class {
                HighBatchClass::Control => self.queue_high_control.front().map_or(0, BytesMut::len),
                HighBatchClass::Consensus => {
                    self.queue_high_consensus.front().map_or(0, BytesMut::len)
                }
                HighBatchClass::ConsensusPayload => self
                    .queue_high_consensus_payload
                    .front()
                    .map_or(0, BytesMut::len),
                HighBatchClass::ConsensusChunk => self
                    .queue_high_consensus_chunk
                    .front()
                    .map_or(0, BytesMut::len),
                HighBatchClass::Other => self.queue_high_other.front().map_or(0, BytesMut::len),
            }
        }

        fn pop_high_frame(&mut self, class: HighBatchClass) -> BytesMut {
            match class {
                HighBatchClass::Control => self
                    .queue_high_control
                    .pop_front()
                    .expect("selected control queue must contain a frame"),
                HighBatchClass::Consensus => self
                    .queue_high_consensus
                    .pop_front()
                    .expect("selected consensus queue must contain a frame"),
                HighBatchClass::ConsensusPayload => self
                    .queue_high_consensus_payload
                    .pop_front()
                    .expect("selected payload queue must contain a frame"),
                HighBatchClass::ConsensusChunk => self
                    .queue_high_consensus_chunk
                    .pop_front()
                    .expect("selected chunk queue must contain a frame"),
                HighBatchClass::Other => self
                    .queue_high_other
                    .pop_front()
                    .expect("selected high-other queue must contain a frame"),
            }
        }

        fn note_high_batch_sent(&mut self, class: HighBatchClass) {
            match class {
                HighBatchClass::Control => {
                    self.high_control_burst = self
                        .high_control_burst
                        .saturating_add(1)
                        .min(Self::MAX_BATCH_CONTROL_BURST);
                }
                HighBatchClass::Consensus => {
                    self.high_control_burst = 0;
                    self.high_consensus_burst = self
                        .high_consensus_burst
                        .saturating_add(1)
                        .min(Self::MAX_BATCH_CONSENSUS_BURST);
                }
                HighBatchClass::ConsensusPayload => {
                    self.high_control_burst = 0;
                    self.high_consensus_burst = 0;
                    self.high_payload_burst = self
                        .high_payload_burst
                        .saturating_add(1)
                        .min(Self::MAX_BATCH_PAYLOAD_BURST);
                }
                HighBatchClass::ConsensusChunk => {
                    self.high_control_burst = 0;
                    self.high_consensus_burst = 0;
                    self.high_payload_burst = 0;
                }
                HighBatchClass::Other => {
                    self.high_control_burst = 0;
                    self.high_consensus_burst = 0;
                }
            }
        }

        fn fill_batch(&mut self) {
            debug_assert!(self.batch_offset >= self.batch.len());
            self.batch.clear();
            self.batch_offset = 0;

            let mut frames_added = 0usize;
            let mut hi_burst = 0usize;

            while frames_added < Self::MAX_BATCH_FRAMES {
                let force_low = hi_burst >= Self::MAX_BATCH_HI_BURST && !self.queue_low.is_empty();
                let next_high = if force_low {
                    None
                } else {
                    self.next_high_batch_class()
                };
                let take_low = force_low || (next_high.is_none() && !self.queue_low.is_empty());
                if next_high.is_none() && !take_low {
                    break;
                }

                let frame_len = if let Some(class) = next_high {
                    self.high_queue_len(class)
                } else {
                    self.queue_low.front().map_or(0, BytesMut::len)
                };
                if frames_added > 0
                    && self.batch.len().saturating_add(frame_len) > Self::MAX_BATCH_BYTES
                {
                    break;
                }

                let mut frame = if let Some(class) = next_high {
                    self.pop_high_frame(class)
                } else {
                    self.queue_low.pop_front().expect("queue.front checked")
                };
                self.batch.extend_from_slice(&frame);
                frame.clear();
                if self.frame_pool.len() < Self::FRAME_POOL_MAX {
                    self.frame_pool.push(frame);
                }

                frames_added = frames_added.saturating_add(1);
                if let Some(class) = next_high {
                    self.note_high_batch_sent(class);
                    hi_burst = hi_burst.saturating_add(1);
                } else {
                    hi_burst = 0;
                }
            }
        }
    }

    /// Either message or ping
    #[derive(Encode, Decode, Clone, Debug)]
    enum Message<T> {
        Data(T),
        Ping,
        Pong,
    }

    impl<T: ClassifyTopic> ClassifyTopic for Message<T> {
        fn topic(&self) -> Topic {
            match self {
                Self::Data(payload) => payload.topic(),
                // Pings are internal to the peer and should not block other
                // traffic. Classify them as `Health` to keep them low-impact.
                Self::Ping | Self::Pong => Topic::Health,
            }
        }
    }

    impl<'a, T> ncore::DecodeFromSlice<'a> for Message<T>
    where
        T: ncore::NoritoSerialize + for<'de> ncore::NoritoDeserialize<'de>,
    {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            let archived = ncore::archived_from_slice::<Self>(bytes)?;
            let archived_bytes = archived.bytes();
            let _guard = ncore::PayloadCtxGuard::enter(archived_bytes);
            let value = <Self as ncore::NoritoDeserialize>::try_deserialize(archived.archived())?;
            Ok((value, archived_bytes.len()))
        }
    }

    pub fn data_message_wire_len<T: Encode>(payload: T) -> usize {
        let message = Message::Data(payload);
        ncore::to_bytes(&message)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX)
    }

    fn framed_message_len<M: Pload>(
        bytes: &[u8],
        expected_schema: [u8; 16],
        padding: usize,
    ) -> Result<usize, Error> {
        const LEN_OFF: usize = 4 + 1 + 1 + 16 + 1;
        if bytes.len() < ncore::Header::SIZE {
            return Err(Error::Format);
        }
        if bytes[..4] != ncore::MAGIC {
            return Err(Error::Format);
        }
        if bytes.get(4) != Some(&ncore::VERSION_MAJOR)
            || bytes.get(5) != Some(&ncore::VERSION_MINOR)
        {
            return Err(Error::Format);
        }
        // schema hash: bytes[6..22]
        let schema = bytes.get(6..22).ok_or(Error::Format)?;
        if schema != expected_schema.as_slice() {
            return Err(Error::Format);
        }
        // compression: bytes[22]
        if bytes.get(22) != Some(&(ncore::Compression::None as u8)) {
            return Err(Error::Format);
        }
        // payload length u64 LE: bytes[23..31]
        let len_bytes = bytes.get(LEN_OFF..LEN_OFF + 8).ok_or(Error::Format)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(len_bytes);
        let payload_len_u64 = u64::from_le_bytes(b);
        if payload_len_u64 > ncore::max_archive_len() {
            return Err(Error::Format);
        }
        let payload_len = usize::try_from(payload_len_u64).map_err(|_| Error::Format)?;
        let total = ncore::Header::SIZE
            .checked_add(padding)
            .and_then(|x| x.checked_add(payload_len))
            .ok_or(Error::Format)?;
        if total > bytes.len() {
            return Err(Error::Format);
        }
        Ok(total)
    }

    #[cfg(test)]
    mod tests {
        use std::{
            pin::Pin,
            sync::{Arc, Mutex},
            task::{Context, Poll},
        };

        use bytes::Bytes;
        use iroha_crypto::encryption::ChaCha20Poly1305;
        use norito::codec::{Decode, Encode};
        use tokio::io::{AsyncRead, AsyncWrite};

        use crate::Priority;

        use super::*;

        #[derive(Encode, Decode, Clone, Debug)]
        struct Dummy;

        impl ClassifyTopic for Dummy {}

        impl<'a> ncore::DecodeFromSlice<'a> for Dummy {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
                ncore::decode_field_canonical::<Self>(bytes)
            }
        }

        #[derive(Encode, Decode, Clone, Debug)]
        struct Blob(Vec<u8>);

        impl ClassifyTopic for Blob {}

        impl<'a> ncore::DecodeFromSlice<'a> for Blob {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
                ncore::decode_field_canonical::<Self>(bytes)
            }
        }

        #[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
        enum RoutedMsg {
            Control(u8),
            Consensus(u8),
            ConsensusPayload(u8),
            ConsensusChunk(u8),
        }

        impl ClassifyTopic for RoutedMsg {
            fn topic(&self) -> Topic {
                match self {
                    Self::Control(_) => Topic::Control,
                    Self::Consensus(_) => Topic::Consensus,
                    Self::ConsensusPayload(_) => Topic::ConsensusPayload,
                    Self::ConsensusChunk(_) => Topic::ConsensusChunk,
                }
            }
        }

        impl<'a> ncore::DecodeFromSlice<'a> for RoutedMsg {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
                ncore::decode_field_canonical::<Self>(bytes)
            }
        }

        #[derive(Default)]
        struct WriteStats {
            writes: usize,
            flushes: usize,
        }

        struct TrackingWrite {
            stats: Arc<Mutex<WriteStats>>,
        }

        struct CollectingWrite {
            buffer: Arc<Mutex<Vec<u8>>>,
        }

        impl AsyncWrite for TrackingWrite {
            fn poll_write(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<std::io::Result<usize>> {
                let mut stats = self.stats.lock().expect("stats lock");
                stats.writes = stats.writes.saturating_add(buf.len());
                Poll::Ready(Ok(buf.len()))
            }

            fn poll_flush(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                let mut stats = self.stats.lock().expect("stats lock");
                stats.flushes = stats.flushes.saturating_add(1);
                Poll::Ready(Ok(()))
            }

            fn poll_shutdown(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Poll::Ready(Ok(()))
            }
        }

        impl AsyncWrite for CollectingWrite {
            fn poll_write(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<std::io::Result<usize>> {
                let mut buffer = self.buffer.lock().expect("buffer lock");
                buffer.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            }

            fn poll_flush(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Poll::Ready(Ok(()))
            }

            fn poll_shutdown(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Poll::Ready(Ok(()))
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_flushes_after_send() {
            let stats = Arc::new(Mutex::new(WriteStats::default()));
            let writer = TrackingWrite {
                stats: stats.clone(),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[1u8; 32])
                    .expect("valid key length");
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            sender
                .prepare_message(&Message::Data(Dummy), Priority::High)
                .expect("prepare message");
            assert!(sender.ready(), "message sender should have queued data");
            sender.send().await.expect("send");
            assert!(!sender.ready(), "queue should be drained");

            let stats = stats.lock().expect("stats lock");
            assert!(stats.writes > 0, "expected at least one write");
            assert!(stats.flushes > 0, "expected at least one flush");
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_reuses_frame_buffers() {
            let stats = Arc::new(Mutex::new(WriteStats::default()));
            let writer = TrackingWrite { stats };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[7u8; 32])
                    .expect("valid key length");
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            sender
                .prepare_message(&Message::Data(Dummy), Priority::High)
                .expect("prepare first message");
            while sender.ready() {
                sender.send().await.expect("send");
            }
            assert_eq!(sender.frame_pool.len(), 1, "expected one pooled frame");

            sender
                .prepare_message(&Message::Data(Dummy), Priority::High)
                .expect("prepare second message");
            while sender.ready() {
                sender.send().await.expect("send");
            }
            assert_eq!(sender.frame_pool.len(), 1, "expected pooled frame reuse");
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_prioritizes_high_frames() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[9u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            let low = Blob(vec![1u8]);
            sender
                .prepare_message(&Message::Data(low), Priority::Low)
                .expect("prepare low message");
            let high = Blob(vec![2u8]);
            sender
                .prepare_message(&Message::Data(high), Priority::High)
                .expect("prepare high message");

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Message<Blob>> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let (first, _) = reader
                .read_message()
                .await
                .expect("read first")
                .expect("first frame");
            let (second, _) = reader
                .read_message()
                .await
                .expect("read second")
                .expect("second frame");

            match first {
                Message::Data(blob) => assert_eq!(blob.0, vec![2u8]),
                _ => panic!("expected high data frame"),
            }
            match second {
                Message::Data(blob) => assert_eq!(blob.0, vec![1u8]),
                _ => panic!("expected low data frame"),
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_keeps_high_batch_class_after_cap_flush() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[12u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            for _ in 0..=MessageSender::<ChaCha20Poly1305>::MAX_PLAINTEXT_MSGS_HI {
                sender
                    .prepare_message(&Message::Data(Dummy), Priority::High)
                    .expect("prepare high message");
            }

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Message<Dummy>> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let mut delivered = 0usize;
            while let Some((msg, _)) = reader.read_message().await.expect("read message") {
                match msg {
                    Message::Data(Dummy) => {
                        delivered = delivered.saturating_add(1);
                    }
                    other => panic!("expected data frame, got {other:?}"),
                }
            }

            assert_eq!(
                delivered,
                MessageSender::<ChaCha20Poly1305>::MAX_PLAINTEXT_MSGS_HI + 1
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_schedules_control_consensus_payload_then_chunk() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[10u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            for msg in [
                RoutedMsg::ConsensusPayload(1),
                RoutedMsg::ConsensusChunk(2),
                RoutedMsg::Consensus(3),
                RoutedMsg::Control(4),
            ] {
                sender
                    .prepare_message(&Message::Data(msg), Priority::High)
                    .expect("prepare routed message");
                sender.flush_plain_high().expect("flush routed batch");
            }

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Message<RoutedMsg>> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let mut delivered = Vec::new();
            while let Some((msg, _)) = reader.read_message().await.expect("read message") {
                match msg {
                    Message::Data(msg) => delivered.push(msg),
                    other => panic!("expected data frame, got {other:?}"),
                }
            }

            assert_eq!(
                delivered,
                vec![
                    RoutedMsg::Control(4),
                    RoutedMsg::Consensus(3),
                    RoutedMsg::ConsensusPayload(1),
                    RoutedMsg::ConsensusChunk(2),
                ]
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_high_lane_fairness_drains_payload_and_chunk_under_consensus() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[11u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            for id in 1..=9 {
                sender
                    .prepare_message(&Message::Data(RoutedMsg::Consensus(id)), Priority::High)
                    .expect("prepare consensus");
                sender.flush_plain_high().expect("flush consensus");
            }
            sender
                .prepare_message(
                    &Message::Data(RoutedMsg::ConsensusPayload(10)),
                    Priority::High,
                )
                .expect("prepare payload");
            sender.flush_plain_high().expect("flush payload");
            sender
                .prepare_message(
                    &Message::Data(RoutedMsg::ConsensusChunk(11)),
                    Priority::High,
                )
                .expect("prepare chunk");
            sender.flush_plain_high().expect("flush chunk");

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Message<RoutedMsg>> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let mut delivered = Vec::new();
            while let Some((msg, _)) = reader.read_message().await.expect("read message") {
                match msg {
                    Message::Data(msg) => delivered.push(msg),
                    other => panic!("expected data frame, got {other:?}"),
                }
            }

            assert_eq!(
                delivered,
                vec![
                    RoutedMsg::Consensus(1),
                    RoutedMsg::Consensus(2),
                    RoutedMsg::Consensus(3),
                    RoutedMsg::Consensus(4),
                    RoutedMsg::ConsensusPayload(10),
                    RoutedMsg::Consensus(5),
                    RoutedMsg::Consensus(6),
                    RoutedMsg::Consensus(7),
                    RoutedMsg::Consensus(8),
                    RoutedMsg::ConsensusChunk(11),
                    RoutedMsg::Consensus(9),
                ]
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_sender_restores_high_batch_class_after_msg_cap_flush() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[12u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);
            let max_msgs_hi =
                u8::try_from(MessageSender::<ChaCha20Poly1305>::MAX_PLAINTEXT_MSGS_HI)
                    .expect("high-priority plaintext cap fits in u8");

            for id in 1..=max_msgs_hi.saturating_add(1) {
                sender
                    .prepare_message(&Message::Data(RoutedMsg::Consensus(id)), Priority::High)
                    .expect("prepare consensus");
            }

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Message<RoutedMsg>> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let mut delivered = Vec::new();
            while let Some((msg, _)) = reader.read_message().await.expect("read message") {
                match msg {
                    Message::Data(RoutedMsg::Consensus(id)) => delivered.push(id),
                    other => panic!("expected consensus frame, got {other:?}"),
                }
            }

            assert_eq!(
                delivered,
                (1..=max_msgs_hi.saturating_add(1)).collect::<Vec<_>>()
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn message_reader_decodes_batched_encrypted_frame() {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = CollectingWrite {
                buffer: Arc::clone(&buffer),
            };
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[4u8; 32])
                    .expect("valid key length");
            let reader_cryptographer = cryptographer.clone();
            let mut sender = MessageSender::new(Box::new(writer), cryptographer, 1024);

            sender
                .prepare_message(&Blob(vec![1u8]), Priority::Low)
                .expect("prepare first");
            sender
                .prepare_message(&Blob(vec![2u8]), Priority::Low)
                .expect("prepare second");

            while sender.ready() {
                sender.send().await.expect("send");
            }

            let data = {
                let buffer = buffer.lock().expect("buffer lock");
                Bytes::from(buffer.clone())
            };
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead { data, pos: 0 });
            let mut reader: MessageReader<ChaCha20Poly1305, Blob> =
                MessageReader::new(read, reader_cryptographer, 1024);

            let (first, _) = reader
                .read_message()
                .await
                .expect("read first")
                .expect("first message");
            let (second, _) = reader
                .read_message()
                .await
                .expect("read second")
                .expect("second message");
            assert_eq!(first.0, vec![1u8]);
            assert_eq!(second.0, vec![2u8]);

            let none = reader.read_message().await.expect("read none");
            assert!(none.is_none());
        }

        #[test]
        fn message_decode_from_slice_roundtrip() {
            let message = Message::Data(Blob(vec![1u8, 2, 3]));
            let bytes = ncore::to_bytes(&message).expect("encode message");
            let view = ncore::from_bytes_view(&bytes).expect("message view");
            let payload = view.as_bytes();
            let (decoded, used) =
                <Message<Blob> as ncore::DecodeFromSlice>::decode_from_slice(payload)
                    .expect("decode from slice");
            assert_eq!(used, payload.len());

            match decoded {
                Message::Data(blob) => assert_eq!(blob.0, vec![1u8, 2, 3]),
                _ => panic!("expected data message"),
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn low_round_robin_serves_all_topics() {
            let (_bs_tx, mut lo_block_sync_rx) = post_channel::channel(4);
            let (tx_tx, mut lo_tx_gossip_rx) = post_channel::channel(4);
            let (peer_tx, mut lo_peer_gossip_rx) = post_channel::channel(4);
            let (_health_tx, mut lo_health_rx) = post_channel::channel(4);
            let (_other_tx, mut lo_other_rx) = post_channel::channel(4);

            tx_tx.send("tx-1").await.unwrap();
            tx_tx.send("tx-2").await.unwrap();
            peer_tx.send("peer").await.unwrap();
            let mut low_rr = 0u8;

            let first = recv_low_rr(
                &mut low_rr,
                &mut lo_block_sync_rx,
                &mut lo_tx_gossip_rx,
                &mut lo_peer_gossip_rx,
                &mut lo_health_rx,
                &mut lo_other_rx,
            )
            .await
            .expect("first low message");
            assert_eq!(first.0, LowTopic::TxGossip);
            assert_eq!(low_rr, 2);

            let second = recv_low_rr(
                &mut low_rr,
                &mut lo_block_sync_rx,
                &mut lo_tx_gossip_rx,
                &mut lo_peer_gossip_rx,
                &mut lo_health_rx,
                &mut lo_other_rx,
            )
            .await
            .expect("second low message");
            assert_eq!(second.0, LowTopic::PeerGossip);
            assert_eq!(low_rr, 3);

            let third = recv_low_rr(
                &mut low_rr,
                &mut lo_block_sync_rx,
                &mut lo_tx_gossip_rx,
                &mut lo_peer_gossip_rx,
                &mut lo_health_rx,
                &mut lo_other_rx,
            )
            .await
            .expect("third low message");
            assert_eq!(third.0, LowTopic::TxGossip);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn high_budget_exhaustion_services_low_message() {
            let (_bs_tx, mut lo_block_sync_rx) = post_channel::channel(4);
            let (tx_tx, mut lo_tx_gossip_rx) = post_channel::channel(4);
            let (_peer_tx, mut lo_peer_gossip_rx) = post_channel::channel(4);
            let (_health_tx, mut lo_health_rx) = post_channel::channel(4);
            let (_other_tx, mut lo_other_rx) = post_channel::channel(4);

            tx_tx.send("tx").await.unwrap();
            let mut hi_budget = 0u8;
            let mut low_rr = 0u8;
            let msg = maybe_take_low_after_hi(
                &mut hi_budget,
                &mut low_rr,
                &mut lo_block_sync_rx,
                &mut lo_tx_gossip_rx,
                &mut lo_peer_gossip_rx,
                &mut lo_health_rx,
                &mut lo_other_rx,
            )
            .expect("expected low message");

            assert_eq!(msg.0, LowTopic::TxGossip);
            assert_eq!(hi_budget, HI_BUDGET_RESET);
        }

        #[test]
        fn high_budget_unblocks_when_no_low_pending() {
            let (_bs_tx, mut lo_block_sync_rx) = post_channel::channel::<Dummy>(4);
            let (_tx_tx, mut lo_tx_gossip_rx) = post_channel::channel::<Dummy>(4);
            let (_peer_tx, mut lo_peer_gossip_rx) = post_channel::channel::<Dummy>(4);
            let (_health_tx, mut lo_health_rx) = post_channel::channel::<Dummy>(4);
            let (_other_tx, mut lo_other_rx) = post_channel::channel::<Dummy>(4);

            let mut hi_budget = 0u8;
            let mut low_rr = 0u8;
            let msg = maybe_take_low_after_hi(
                &mut hi_budget,
                &mut low_rr,
                &mut lo_block_sync_rx,
                &mut lo_tx_gossip_rx,
                &mut lo_peer_gossip_rx,
                &mut lo_health_rx,
                &mut lo_other_rx,
            );

            assert!(msg.is_none());
            assert_eq!(hi_budget, HI_BUDGET_FALLBACK);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn high_lane_consensus_bypasses_payload_and_chunk_posts() {
            let (control_tx, mut control_rx) = post_channel::channel(8);
            let (consensus_tx, mut consensus_rx) = post_channel::channel(8);
            let (payload_tx, mut payload_rx) = post_channel::channel(8);
            let (chunk_tx, mut chunk_rx) = post_channel::channel(8);
            let _ = control_tx;

            payload_tx.send("payload").await.expect("queue payload");
            chunk_tx.send("chunk").await.expect("queue chunk");
            consensus_tx
                .send("consensus")
                .await
                .expect("queue consensus");

            let mut consensus_burst = 0u8;
            let mut payload_burst = 0u8;
            let mut control_burst = 0u8;

            let first = try_recv_high_fair(
                &mut control_burst,
                &mut consensus_burst,
                &mut payload_burst,
                &mut control_rx,
                &mut consensus_rx,
                &mut payload_rx,
                &mut chunk_rx,
            )
            .expect("first high message");
            let second = try_recv_high_fair(
                &mut control_burst,
                &mut consensus_burst,
                &mut payload_burst,
                &mut control_rx,
                &mut consensus_rx,
                &mut payload_rx,
                &mut chunk_rx,
            )
            .expect("second high message");
            let third = try_recv_high_fair(
                &mut control_burst,
                &mut consensus_burst,
                &mut payload_burst,
                &mut control_rx,
                &mut consensus_rx,
                &mut payload_rx,
                &mut chunk_rx,
            )
            .expect("third high message");

            assert_eq!(first, (HighTopic::Consensus, "consensus"));
            assert_eq!(second, (HighTopic::ConsensusPayload, "payload"));
            assert_eq!(third, (HighTopic::ConsensusChunk, "chunk"));
        }

        #[tokio::test(flavor = "current_thread")]
        async fn high_lane_payload_and_chunk_progress_under_sustained_consensus() {
            let (control_tx, mut control_rx) = post_channel::channel(16);
            let (consensus_tx, mut consensus_rx) = post_channel::channel(16);
            let (payload_tx, mut payload_rx) = post_channel::channel(16);
            let (chunk_tx, mut chunk_rx) = post_channel::channel(16);
            let _ = control_tx;

            for id in 1..=9 {
                consensus_tx
                    .send(format!("c{id}"))
                    .await
                    .expect("queue consensus");
            }
            payload_tx
                .send(String::from("payload"))
                .await
                .expect("queue payload");
            chunk_tx
                .send(String::from("chunk"))
                .await
                .expect("queue chunk");

            let mut consensus_burst = 0u8;
            let mut payload_burst = 0u8;
            let mut control_burst = 0u8;
            let mut served = Vec::new();
            while let Some(item) = try_recv_high_fair(
                &mut control_burst,
                &mut consensus_burst,
                &mut payload_burst,
                &mut control_rx,
                &mut consensus_rx,
                &mut payload_rx,
                &mut chunk_rx,
            ) {
                served.push(item);
            }

            let expected = vec![
                (HighTopic::Consensus, String::from("c1")),
                (HighTopic::Consensus, String::from("c2")),
                (HighTopic::Consensus, String::from("c3")),
                (HighTopic::Consensus, String::from("c4")),
                (HighTopic::ConsensusPayload, String::from("payload")),
                (HighTopic::Consensus, String::from("c5")),
                (HighTopic::Consensus, String::from("c6")),
                (HighTopic::Consensus, String::from("c7")),
                (HighTopic::Consensus, String::from("c8")),
                (HighTopic::ConsensusChunk, String::from("chunk")),
                (HighTopic::Consensus, String::from("c9")),
            ];
            assert_eq!(served, expected);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn high_lane_control_priority_remains_unchanged() {
            let (control_tx, mut control_rx) = post_channel::channel(8);
            let (consensus_tx, mut consensus_rx) = post_channel::channel(8);
            let (payload_tx, mut payload_rx) = post_channel::channel(8);
            let (chunk_tx, mut chunk_rx) = post_channel::channel(8);

            payload_tx.send("payload").await.expect("queue payload");
            chunk_tx.send("chunk").await.expect("queue chunk");
            consensus_tx
                .send("consensus")
                .await
                .expect("queue consensus");
            control_tx.send("control").await.expect("queue control");

            let mut consensus_burst = 0u8;
            let mut payload_burst = 0u8;
            let mut control_burst = 0u8;
            let first = try_recv_high_fair(
                &mut control_burst,
                &mut consensus_burst,
                &mut payload_burst,
                &mut control_rx,
                &mut consensus_rx,
                &mut payload_rx,
                &mut chunk_rx,
            )
            .expect("first high message");

            assert_eq!(first, (HighTopic::Control, "control"));
        }

        #[test]
        fn inbound_priority_marks_control_planes_high() {
            assert_eq!(
                super::inbound_priority_from_topic(crate::network::message::Topic::Consensus),
                Priority::High
            );
            assert_eq!(
                super::inbound_priority_from_topic(
                    crate::network::message::Topic::ConsensusPayload
                ),
                Priority::High
            );
            assert_eq!(
                super::inbound_priority_from_topic(crate::network::message::Topic::Control),
                Priority::High
            );
            assert_eq!(
                super::inbound_priority_from_topic(crate::network::message::Topic::ConsensusChunk),
                Priority::High
            );
            assert_eq!(
                super::inbound_priority_from_topic(crate::network::message::Topic::TxGossip),
                Priority::Low
            );
        }

        fn framed_message<T: Encode>(value: &T) -> Vec<u8> {
            ncore::to_bytes(value).expect("encode framed message")
        }

        fn encrypted_frame(plaintext: &[u8], key_byte: u8) -> Vec<u8> {
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[key_byte; 32])
                    .expect("valid key length");
            let mut encrypted = Vec::new();
            let encrypted = cryptographer
                .encrypt_into(plaintext, &mut encrypted)
                .expect("encrypt frame");
            let mut frame = Vec::with_capacity(
                MessageReader::<ChaCha20Poly1305, Message<Blob>>::U32_SIZE + encrypted.len(),
            );
            let encrypted_len =
                u32::try_from(encrypted.len()).expect("encrypted frame length fits in u32");
            frame.extend_from_slice(&encrypted_len.to_be_bytes());
            frame.extend_from_slice(encrypted);
            frame
        }

        fn blob_message_frame(payload: &[u8]) -> Vec<u8> {
            framed_message(&Message::Data(Blob(payload.to_vec())))
        }

        struct FakeRead {
            data: Bytes,
            pos: usize,
        }

        impl AsyncRead for FakeRead {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                if self.pos >= self.data.len() {
                    return std::task::Poll::Ready(Ok(()));
                }
                let remaining = &self.data[self.pos..];
                let n = remaining.len().min(buf.remaining());
                buf.put_slice(&remaining[..n]);
                self.pos += n;
                std::task::Poll::Ready(Ok(()))
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn malformed_decrypted_payload_frame_is_dropped_and_next_valid_frame_is_delivered() {
            let key_byte = 5u8;
            let mut malformed_plain = blob_message_frame(&[1u8]);
            let mut truncated_inner = blob_message_frame(&[2u8]);
            truncated_inner.pop().expect("truncate inner frame");
            malformed_plain.extend_from_slice(&truncated_inner);
            let valid_plain = blob_message_frame(&[9u8]);

            let mut raw = encrypted_frame(&malformed_plain, key_byte);
            raw.extend_from_slice(&encrypted_frame(&valid_plain, key_byte));

            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead {
                data: Bytes::from(raw),
                pos: 0,
            });
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[key_byte; 32])
                    .expect("valid key length");
            let mut reader: MessageReader<ChaCha20Poly1305, Message<Blob>> =
                MessageReader::new(read, cryptographer, 1024);

            let err = reader
                .read_message()
                .await
                .expect_err("first decrypted frame should be dropped");
            assert!(matches!(err, Error::MalformedPayloadFrame));

            let (message, encoded_len) = reader
                .read_message()
                .await
                .expect("read next frame")
                .expect("valid frame should remain readable");
            assert_eq!(encoded_len, valid_plain.len());
            match message {
                Message::Data(blob) => assert_eq!(blob.0, vec![9u8]),
                other => panic!("expected valid data frame, got {other:?}"),
            }

            let none = reader.read_message().await.expect("stream exhausted");
            assert!(none.is_none());
        }

        #[tokio::test(flavor = "current_thread")]
        async fn malformed_payload_frame_counter_tracks_recovery_without_disconnect() {
            let key_byte = 6u8;
            let mut first_bad = blob_message_frame(&[1u8, 2u8]);
            first_bad.pop().expect("truncate first malformed frame");
            let valid_plain = blob_message_frame(&[7u8, 8u8]);
            let mut second_bad = blob_message_frame(&[3u8, 4u8]);
            second_bad.pop().expect("truncate second malformed frame");

            let mut raw = encrypted_frame(&first_bad, key_byte);
            raw.extend_from_slice(&encrypted_frame(&valid_plain, key_byte));
            raw.extend_from_slice(&encrypted_frame(&second_bad, key_byte));
            raw.extend_from_slice(&encrypted_frame(&valid_plain, key_byte));

            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead {
                data: Bytes::from(raw),
                pos: 0,
            });
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[key_byte; 32])
                    .expect("valid key length");
            let mut reader: MessageReader<ChaCha20Poly1305, Message<Blob>> =
                MessageReader::new(read, cryptographer, 1024);
            let counter_before = super::super::malformed_payload_frame_count();
            let mut streak = 0u32;

            let err = reader
                .read_message()
                .await
                .expect_err("first malformed frame should not decode");
            assert!(matches!(err, Error::MalformedPayloadFrame));
            assert!(
                !super::note_malformed_payload_frame(&mut streak),
                "one malformed decrypted frame must not disconnect the session"
            );
            assert!(
                super::super::malformed_payload_frame_count() >= counter_before.saturating_add(1)
            );

            let (message, _) = reader
                .read_message()
                .await
                .expect("valid frame after malformed one")
                .expect("message after malformed one");
            streak = 0;
            match message {
                Message::Data(blob) => assert_eq!(blob.0, vec![7u8, 8u8]),
                other => panic!("expected valid data frame, got {other:?}"),
            }

            let err = reader
                .read_message()
                .await
                .expect_err("second malformed frame should not decode");
            assert!(matches!(err, Error::MalformedPayloadFrame));
            assert!(
                !super::note_malformed_payload_frame(&mut streak),
                "streak should restart after a successfully decoded frame"
            );
            assert_eq!(streak, 1);
            assert!(
                super::super::malformed_payload_frame_count() >= counter_before.saturating_add(2)
            );

            let (message, _) = reader
                .read_message()
                .await
                .expect("reader should continue after second malformed frame")
                .expect("final valid message");
            match message {
                Message::Data(blob) => assert_eq!(blob.0, vec![7u8, 8u8]),
                other => panic!("expected valid data frame, got {other:?}"),
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn malformed_payload_frame_disconnects_after_three_consecutive_frames() {
            let key_byte = 7u8;
            let mut malformed_plain = blob_message_frame(&[0xAAu8]);
            malformed_plain.pop().expect("truncate malformed frame");

            let mut raw = Vec::new();
            for _ in 0..super::MALFORMED_PAYLOAD_FRAME_THRESHOLD {
                raw.extend_from_slice(&encrypted_frame(&malformed_plain, key_byte));
            }

            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead {
                data: Bytes::from(raw),
                pos: 0,
            });
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[key_byte; 32])
                    .expect("valid key length");
            let mut reader: MessageReader<ChaCha20Poly1305, Message<Blob>> =
                MessageReader::new(read, cryptographer, 1024);
            let counter_before = super::super::malformed_payload_frame_count();
            let mut streak = 0u32;

            for attempt in 1..=super::MALFORMED_PAYLOAD_FRAME_THRESHOLD {
                let err = reader
                    .read_message()
                    .await
                    .expect_err("malformed decrypted frame should be reported");
                assert!(matches!(err, Error::MalformedPayloadFrame));
                let disconnect = super::note_malformed_payload_frame(&mut streak);
                assert_eq!(
                    disconnect,
                    attempt == super::MALFORMED_PAYLOAD_FRAME_THRESHOLD
                );
            }

            assert_eq!(streak, super::MALFORMED_PAYLOAD_FRAME_THRESHOLD);
            assert!(
                super::super::malformed_payload_frame_count()
                    >= counter_before
                        .saturating_add(u64::from(super::MALFORMED_PAYLOAD_FRAME_THRESHOLD))
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn decrypt_failure_remains_fatal_and_log_sampling_limits_flood() {
            // Build a buffer with many bogus encrypted frames: [len=16][16 zero bytes] * N.
            const FRAMES: usize = 200;
            let mut raw = Vec::with_capacity(FRAMES * (4 + 16));
            for _ in 0..FRAMES {
                let len: u32 = 16;
                raw.extend_from_slice(&len.to_be_bytes());
                raw.extend_from_slice(&[0u8; 16]);
            }
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead {
                data: Bytes::from(raw),
                pos: 0,
            });
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[1u8; 32])
                    .expect("valid key length");
            let mut reader: MessageReader<ChaCha20Poly1305, Dummy> =
                MessageReader::new(read, cryptographer, 1024);

            let err = reader
                .read_message()
                .await
                .expect_err("undecryptable frame should remain fatal");
            assert!(matches!(err, Error::SymmetricEncryption(_)));

            let mut sampler = crate::sampler::LogSampler::new();
            let mut logged = 0u32;
            for _ in 0..FRAMES {
                if sampler
                    .should_log(tokio::time::Duration::from_millis(500))
                    .is_some()
                {
                    logged += 1;
                }
            }
            assert!(logged <= 1, "sampler should limit logs; got {logged}");
        }

        #[tokio::test(flavor = "current_thread")]
        async fn oversized_frame_is_rejected_early() {
            // Build a buffer with only a u32 length prefix larger than limit
            let mut raw = Vec::with_capacity(4);
            let declared: u32 = 10_000; // arbitrary large
            raw.extend_from_slice(&declared.to_be_bytes());
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(FakeRead {
                data: Bytes::from(raw),
                pos: 0,
            });
            let crypt =
                super::cryptographer::Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(
                    &[2u8; 32],
                )
                .expect("valid key length");
            let mut mr: MessageReader<ChaCha20Poly1305, Dummy> =
                MessageReader::new(read, crypt, 1024); // max_frame_bytes=1024
            let err = mr.read_message().await.err();
            assert!(matches!(err, Some(Error::FrameTooLarge)));
        }

        #[test]
        fn message_reader_reserves_capacity_for_declared_frame() {
            let read: Box<dyn AsyncRead + Send + Unpin> = Box::new(tokio::io::empty());
            let crypt =
                super::cryptographer::Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(
                    &[3u8; 32],
                )
                .expect("valid key length");
            let mut mr: MessageReader<ChaCha20Poly1305, Dummy> =
                MessageReader::new(read, crypt, 8192);
            let declared: u32 = 4096;
            mr.buffer.extend_from_slice(&declared.to_be_bytes());
            let before = mr.buffer.capacity();
            mr.reserve_for_frame().expect("reserve");
            let needed = (declared as usize) + MessageReader::<ChaCha20Poly1305, Dummy>::U32_SIZE;
            assert!(mr.buffer.capacity() >= needed);
            assert!(mr.buffer.capacity() >= before);
        }

        fn make_sender(max_frame_bytes: usize) -> MessageSender<ChaCha20Poly1305> {
            let writer: Box<dyn AsyncWrite + Send + Unpin> = Box::new(tokio::io::sink());
            let crypt =
                Cryptographer::new_with_raw_key_bytes(&[0u8; 32]).expect("valid key length");
            MessageSender::new(writer, crypt, max_frame_bytes)
        }

        fn assert_large_payload_rejected(max_frame_bytes: usize) {
            let mut sender = make_sender(max_frame_bytes);
            let payload = Blob(vec![0u8; max_frame_bytes.saturating_add(128)]);
            let err = sender
                .prepare_message(&payload, Priority::High)
                .unwrap_err();
            assert!(matches!(err, Error::FrameTooLarge));
            assert!(!sender.ready(), "rejected frame should not queue data");
        }

        #[test]
        fn message_sender_allows_within_cap() {
            let mut sender = make_sender(512);
            let small = Blob(vec![0u8; 8]);
            sender
                .prepare_message(&small, Priority::High)
                .expect("small payload must be accepted");
            assert!(sender.ready(), "accepted frame should be queued");
        }

        #[test]
        fn message_sender_rejects_oversized_frame_tcp() {
            assert_large_payload_rejected(256);
        }

        #[cfg(feature = "p2p_tls")]
        #[test]
        fn message_sender_rejects_oversized_frame_tls() {
            assert_large_payload_rejected(256);
        }

        #[cfg(feature = "quic")]
        #[test]
        fn message_sender_rejects_oversized_frame_quic() {
            assert_large_payload_rejected(256);
        }

        #[cfg(feature = "p2p_ws")]
        #[test]
        fn message_sender_rejects_oversized_frame_ws() {
            assert_large_payload_rejected(256);
        }
    }
}

mod state {
    //! Module for peer stages.

    use iroha_crypto::{KeyGenOption, KeyPair, PublicKey, Signature};
    use iroha_data_model::peer::Peer;
    use iroha_primitives::addr::SocketAddr;

    use super::{cryptographer::Cryptographer, *};

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeConfidentialDigest {
        vk_set_hash: Option<[u8; 32]>,
        poseidon_params_id: Option<u32>,
        pedersen_params_id: Option<u32>,
        conf_rules_version: Option<u32>,
    }

    impl From<&crate::ConfidentialFeatureDigest> for HandshakeConfidentialDigest {
        fn from(digest: &crate::ConfidentialFeatureDigest) -> Self {
            Self {
                vk_set_hash: digest.vk_set_hash,
                poseidon_params_id: digest.poseidon_params_id,
                pedersen_params_id: digest.pedersen_params_id,
                conf_rules_version: digest.conf_rules_version,
            }
        }
    }

    impl From<HandshakeConfidentialDigest> for crate::ConfidentialFeatureDigest {
        fn from(digest: HandshakeConfidentialDigest) -> Self {
            Self {
                vk_set_hash: digest.vk_set_hash,
                poseidon_params_id: digest.poseidon_params_id,
                pedersen_params_id: digest.pedersen_params_id,
                conf_rules_version: digest.conf_rules_version,
            }
        }
    }

    impl<'a> norito::core::DecodeFromSlice<'a> for HandshakeConfidentialDigest {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let mut offset = 0;
            let (vk_set_hash, used) =
                <Option<[u8; 32]> as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
            offset += used;
            let (poseidon_params_id, used) =
                <Option<u32> as norito::core::DecodeFromSlice>::decode_from_slice(
                    &bytes[offset..],
                )?;
            offset += used;
            let (pedersen_params_id, used) =
                <Option<u32> as norito::core::DecodeFromSlice>::decode_from_slice(
                    &bytes[offset..],
                )?;
            offset += used;
            let (conf_rules_version, used) =
                <Option<u32> as norito::core::DecodeFromSlice>::decode_from_slice(
                    &bytes[offset..],
                )?;
            offset += used;
            Ok((
                HandshakeConfidentialDigest {
                    vk_set_hash,
                    poseidon_params_id,
                    pedersen_params_id,
                    conf_rules_version,
                },
                offset,
            ))
        }
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeConsensusMeta {
        pub(super) mode_tag: Option<String>,
        pub(super) proto_version: Option<u32>,
        pub(super) consensus_fingerprint: Option<[u8; 32]>,
        pub(super) config: Option<ConsensusConfigCaps>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeConfidentialMeta {
        pub(super) enabled: Option<bool>,
        pub(super) assume_valid: Option<bool>,
        pub(super) verifier_backend: Option<String>,
        pub(super) features: Option<HandshakeConfidentialDigest>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeCryptoMeta {
        pub(super) sm_enabled: Option<bool>,
        pub(super) sm_openssl_preview: Option<bool>,
    }

    fn build_trust_meta(trust_gossip: bool, scion_supported: bool) -> HandshakeTrustMeta {
        HandshakeTrustMeta {
            trust_gossip,
            scion_supported,
        }
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeTrustMeta {
        pub(super) trust_gossip: bool,
        pub(super) scion_supported: bool,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeHelloV1 {
        pub(super) algorithm: iroha_crypto::Algorithm,
        pub(super) public_key: Vec<u8>,
        pub(super) signature: Vec<u8>,
        pub(super) addr: iroha_primitives::addr::SocketAddr,
        pub(super) relay: RelayRole,
        pub(super) consensus: HandshakeConsensusMeta,
        pub(super) confidential: HandshakeConfidentialMeta,
        pub(super) crypto: HandshakeCryptoMeta,
        pub(super) trust: HandshakeTrustMeta,
    }

    #[derive(Clone, Debug)]
    pub(super) enum HandshakeHello {
        V1(HandshakeHelloV1),
    }

    fn build_consensus_meta(caps: Option<&ConsensusHandshakeCaps>) -> HandshakeConsensusMeta {
        caps.map_or(
            HandshakeConsensusMeta {
                mode_tag: None,
                proto_version: None,
                consensus_fingerprint: None,
                config: None,
            },
            |caps| HandshakeConsensusMeta {
                mode_tag: Some(caps.mode_tag.clone()),
                proto_version: Some(caps.proto_version),
                consensus_fingerprint: Some(caps.consensus_fingerprint),
                config: Some(caps.config.clone()),
            },
        )
    }

    fn build_confidential_meta(
        caps: Option<&crate::ConfidentialHandshakeCaps>,
    ) -> HandshakeConfidentialMeta {
        caps.map_or(
            HandshakeConfidentialMeta {
                enabled: None,
                assume_valid: None,
                verifier_backend: None,
                features: None,
            },
            |caps| HandshakeConfidentialMeta {
                enabled: Some(caps.enabled),
                assume_valid: Some(caps.assume_valid),
                verifier_backend: Some(caps.verifier_backend.clone()),
                features: caps
                    .features
                    .as_ref()
                    .map(HandshakeConfidentialDigest::from),
            },
        )
    }

    fn build_crypto_meta(caps: Option<&crate::CryptoHandshakeCaps>) -> HandshakeCryptoMeta {
        caps.map_or(
            HandshakeCryptoMeta {
                sm_enabled: None,
                sm_openssl_preview: None,
            },
            |caps| HandshakeCryptoMeta {
                sm_enabled: Some(caps.sm_enabled),
                sm_openssl_preview: Some(caps.sm_openssl_preview),
            },
        )
    }

    fn enforce_consensus_caps(
        caps: Option<&crate::ConsensusHandshakeCaps>,
        meta: &HandshakeConsensusMeta,
    ) -> Result<(), crate::Error> {
        if let Some(caps) = caps {
            let Some(mode) = &meta.mode_tag else {
                return Err(crate::Error::HandshakeConsensusMismatch {
                    reason: "missing consensus mode tag".to_string(),
                });
            };
            if mode != &caps.mode_tag {
                let reason = format!(
                    "mode tag mismatch (expected {}, got {})",
                    caps.mode_tag, mode
                );
                iroha_logger::warn!(reason, "peer rejected due to consensus config mismatch");
                return Err(crate::Error::HandshakeConsensusMismatch { reason });
            }
            let Some(proto) = meta.proto_version else {
                return Err(crate::Error::HandshakeConsensusMismatch {
                    reason: "missing consensus proto version".to_string(),
                });
            };
            if proto != caps.proto_version {
                let reason = format!(
                    "proto version mismatch (expected {}, got {})",
                    caps.proto_version, proto
                );
                iroha_logger::warn!(reason, "peer rejected due to consensus config mismatch");
                return Err(crate::Error::HandshakeConsensusMismatch { reason });
            }
            let Some(fp) = meta.consensus_fingerprint else {
                return Err(crate::Error::HandshakeConsensusMismatch {
                    reason: "missing consensus fingerprint".to_string(),
                });
            };
            if fp != caps.consensus_fingerprint {
                let reason = format!(
                    "fingerprint mismatch (expected 0x{}, got 0x{})",
                    hex_bytes(&caps.consensus_fingerprint),
                    hex_bytes(&fp)
                );
                iroha_logger::warn!(reason, "peer rejected due to consensus config mismatch");
                return Err(crate::Error::HandshakeConsensusMismatch { reason });
            }

            let peer_config =
                meta.config
                    .as_ref()
                    .ok_or_else(|| crate::Error::HandshakeConsensusMismatch {
                        reason: "missing consensus runtime config".to_string(),
                    })?;
            if let Some(reason) = consensus_config_mismatch(&caps.config, peer_config) {
                iroha_logger::warn!(
                    ?peer_config,
                    expected=?caps.config,
                    %reason,
                    "peer rejected due to consensus config mismatch"
                );
                return Err(crate::Error::HandshakeConsensusMismatch { reason });
            }
        }
        Ok(())
    }

    fn consensus_config_mismatch(
        expected: &ConsensusConfigCaps,
        got: &ConsensusConfigCaps,
    ) -> Option<String> {
        macro_rules! check {
            ($field:ident) => {
                if expected.$field != got.$field {
                    return Some(format!(
                        "{} mismatch (expected {}, got {})",
                        stringify!($field),
                        expected.$field,
                        got.$field
                    ));
                }
            };
        }

        check!(collectors_k);
        check!(redundant_send_r);
        check!(da_enabled);
        check!(rbc_chunk_max_bytes);
        check!(rbc_session_ttl_ms);
        check!(rbc_store_max_sessions);
        check!(rbc_store_soft_sessions);
        check!(rbc_store_max_bytes);
        check!(rbc_store_soft_bytes);
        None
    }

    fn hex_bytes(bytes: &[u8]) -> String {
        use core::fmt::Write as _;

        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(&mut out, "{byte:02x}");
        }
        out
    }

    fn enforce_confidential_caps(
        caps: Option<&crate::ConfidentialHandshakeCaps>,
        meta: &HandshakeConfidentialMeta,
        _remote_addr: &iroha_primitives::addr::SocketAddr,
    ) -> Result<(), crate::Error> {
        let Some(caps) = caps else {
            return Ok(());
        };

        let HandshakeConfidentialMeta {
            enabled,
            assume_valid,
            verifier_backend,
            features,
        } = meta.clone();

        let enabled = enabled.ok_or(crate::Error::HandshakeConfidentialMismatch)?;
        let assume_valid = assume_valid.ok_or(crate::Error::HandshakeConfidentialMismatch)?;
        let backend = verifier_backend.ok_or(crate::Error::HandshakeConfidentialMismatch)?;
        let remote_features = features.map(crate::ConfidentialFeatureDigest::from);

        if enabled != caps.enabled
            || assume_valid != caps.assume_valid
            || backend != caps.verifier_backend
        {
            return Err(crate::Error::HandshakeConfidentialMismatch);
        }

        match (&caps.features, remote_features.as_ref()) {
            (Some(local), Some(remote)) if remote == local => {}
            (Some(_), Some(_) | None) => {
                return Err(crate::Error::HandshakeConfidentialMismatch);
            }
            (None, _) => {}
        }

        Ok(())
    }

    fn enforce_crypto_caps(
        caps: Option<&crate::CryptoHandshakeCaps>,
        meta: &HandshakeCryptoMeta,
        remote_addr: &iroha_primitives::addr::SocketAddr,
    ) -> Result<(), crate::Error> {
        let Some(caps) = caps else {
            return Ok(());
        };

        let HandshakeCryptoMeta {
            sm_enabled,
            sm_openssl_preview,
        } = meta.clone();

        match (sm_enabled, caps.require_sm_handshake_match) {
            (Some(remote_enabled), true) => {
                if remote_enabled != caps.sm_enabled {
                    return Err(crate::Error::HandshakeCryptoMismatch);
                }
            }
            (Some(remote_enabled), false) => {
                if remote_enabled != caps.sm_enabled {
                    iroha_logger::warn!(
                        %remote_enabled,
                        local_enabled = %caps.sm_enabled,
                        addr = ?remote_addr,
                        "Remote peer SM helper availability differs; permitted by configuration"
                    );
                }
            }
            (None, true) => return Err(crate::Error::HandshakeCryptoMismatch),
            (None, false) => {
                if caps.sm_enabled {
                    iroha_logger::warn!(
                        addr = ?remote_addr,
                        "Remote peer omitted SM helper capability flag; continuing due to permissive configuration"
                    );
                }
            }
        }

        match (sm_openssl_preview, caps.require_sm_openssl_preview_match) {
            (Some(remote_preview), true) => {
                if remote_preview != caps.sm_openssl_preview {
                    return Err(crate::Error::HandshakeCryptoMismatch);
                }
            }
            (Some(remote_preview), false) => {
                if remote_preview != caps.sm_openssl_preview {
                    iroha_logger::warn!(
                        %remote_preview,
                        local_preview = %caps.sm_openssl_preview,
                        addr = ?remote_addr,
                        "Remote peer OpenSSL preview flag differs; permitted by configuration"
                    );
                }
            }
            (None, true) => return Err(crate::Error::HandshakeCryptoMismatch),
            (None, false) => {
                if caps.sm_openssl_preview {
                    iroha_logger::warn!(
                        addr = ?remote_addr,
                        "Remote peer omitted OpenSSL preview capability; continuing due to permissive configuration"
                    );
                }
            }
        }

        Ok(())
    }

    fn handshake_signature_payload<K: Kex, E: Enc>(
        cryptographer: &Cryptographer<E>,
        advertised_addr: &iroha_primitives::addr::SocketAddr,
        local_pk: &K::PublicKey,
        remote_pk: &K::PublicKey,
        chain_id: Option<&iroha_data_model::ChainId>,
        transport_binding: Option<&[u8; iroha_crypto::Hash::LENGTH]>,
    ) -> Vec<u8> {
        let _ = (local_pk, remote_pk);
        let mut data = Vec::new();
        data.extend_from_slice(&cryptographer.disambiguator.to_be_bytes());
        data.extend_from_slice(&advertised_addr.encode());
        if let Some(cid) = chain_id {
            data.extend_from_slice(&cid.encode());
        }
        if let Some(binding) = transport_binding {
            data.extend_from_slice(binding);
        }
        data
    }

    pub(super) fn encode_handshake_message<E: Enc>(
        cryptographer: &Cryptographer<E>,
        message: &HandshakeHelloV1,
    ) -> Result<Vec<u8>, crate::Error> {
        let payload = message.encode();
        let mut encoded = Vec::with_capacity(payload.len().saturating_add(2));
        encoded.push(HANDSHAKE_HELLO_VERSION_PREFIX);
        encoded.push(HANDSHAKE_HELLO_VERSION);
        encoded.extend_from_slice(&payload);
        cryptographer.encrypt(&encoded)
    }

    pub(super) fn decode_handshake_message<E: Enc>(
        cryptographer: &Cryptographer<E>,
        payload: &[u8],
    ) -> Result<HandshakeHello, crate::Error> {
        let decrypted = cryptographer.decrypt(payload)?;
        let (&prefix, rest) = decrypted.split_first().ok_or(crate::Error::Format)?;
        if prefix != HANDSHAKE_HELLO_VERSION_PREFIX {
            return Err(crate::Error::Format);
        }
        let (&version, body) = rest.split_first().ok_or(crate::Error::Format)?;
        if version != HANDSHAKE_HELLO_VERSION {
            return Err(crate::Error::Format);
        }
        let mut slice = body;
        let hello = DecodeAll::decode_all(&mut slice)?;
        Ok(HandshakeHello::V1(hello))
    }

    /// Peer that is connecting. This is the initial stage of a new
    /// outgoing peer.
    #[allow(clippy::struct_excessive_bools)]
    pub(super) struct Connecting {
        pub peer_addr: SocketAddr,
        pub peer_id: iroha_data_model::prelude::PeerId,
        pub our_public_address: SocketAddr,
        pub key_pair: KeyPair,
        pub connection_id: ConnectionId,
        pub chain_id: Option<iroha_data_model::ChainId>,
        pub consensus_caps: Option<ConsensusHandshakeCaps>,
        pub confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub soranet_handshake: Arc<SoranetHandshakeConfig>,
        pub quic_enabled: bool,
        pub tls_enabled: bool,
        pub tls_fallback_to_plain: bool,
        pub prefer_scion: bool,
        pub local_scion_supported: bool,
        pub prefer_ws_fallback: bool,
        pub trust_gossip: bool,
        pub relay_role: RelayRole,
        pub dial_timeout: Duration,
        pub happy_eyeballs_stagger: Duration,
        pub tcp_nodelay: bool,
        pub tcp_keepalive: Option<Duration>,
        pub proxy_tls_verify: bool,
        pub proxy_tls_pinned_cert_der: Option<std::sync::Arc<[u8]>>,
        pub proxy_policy: crate::transport::ProxyPolicy,
        pub quic_dialer: Option<crate::transport::QuicDialer>,
    }

    impl Connecting {
        #[allow(unused_variables, clippy::too_many_lines, clippy::single_match_else)]
        pub(super) async fn connect_to(
            Self {
                peer_addr,
                peer_id,
                our_public_address,
                key_pair,
                connection_id,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
                quic_enabled,
                tls_enabled,
                tls_fallback_to_plain,
                prefer_scion,
                local_scion_supported,
                prefer_ws_fallback,
                trust_gossip,
                relay_role,
                dial_timeout,
                happy_eyeballs_stagger,
                tcp_nodelay,
                tcp_keepalive,
                proxy_tls_verify,
                proxy_tls_pinned_cert_der,
                proxy_policy,
                quic_dialer,
            }: Self,
        ) -> Result<ConnectedTo, crate::Error> {
            #[cfg(feature = "p2p_ws")]
            async fn dial_ws(
                peer_addr: &iroha_primitives::addr::SocketAddr,
                endpoint: &str,
                opts: &crate::transport::TcpConnectOptions,
                connection_id: ConnectionId,
                dial_timeout: Duration,
                tls_enabled: bool,
            ) -> Option<Connection> {
                // Avoid probing WSS first unless TLS is explicitly enabled. Some WS bridges (tests,
                // sidecars) accept a single connection and will tear down the listener after a
                // failed handshake, making subsequent WS attempts fail with connection refused.
                let order = if tls_enabled {
                    [true, false]
                } else {
                    [false, true]
                };
                for use_wss in order {
                    let url = if use_wss {
                        format!("wss://{endpoint}/p2p")
                    } else {
                        format!("ws://{endpoint}/p2p")
                    };
                    let res = tokio::time::timeout(dial_timeout, async {
                        let stream = crate::transport::connect(peer_addr, opts).await?;
                        match stream {
                            crate::transport::TcpConnectStream::Plain(tcp) => {
                                let ws =
                                    crate::transport::ws::connect_with_stream(url, tcp).await?;
                                let (r, w) = tokio::io::split(ws);
                                Ok::<_, std::io::Error>(Connection::from_split(connection_id, r, w))
                            }
                            #[cfg(feature = "p2p_tls")]
                            crate::transport::TcpConnectStream::Tls(tls) => {
                                let ws =
                                    crate::transport::ws::connect_with_stream(url, tls).await?;
                                let (r, w) = tokio::io::split(ws);
                                Ok::<_, std::io::Error>(Connection::from_split(connection_id, r, w))
                            }
                        }
                    })
                    .await;
                    if let Ok(Ok(conn)) = res {
                        crate::network::inc_ws_outbound();
                        return Some(conn);
                    }
                }
                None
            }

            async fn dial_tcp_plain(
                peer_addr: &iroha_primitives::addr::SocketAddr,
                opts: &crate::transport::TcpConnectOptions,
                dial_timeout: Duration,
            ) -> Result<crate::transport::TcpConnectStream, crate::Error> {
                match tokio::time::timeout(dial_timeout, crate::transport::connect(peer_addr, opts))
                    .await
                {
                    Ok(Ok(stream)) => Ok(stream),
                    Ok(Err(e)) => Err(e.into()),
                    Err(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "dial timeout",
                    )
                    .into()),
                }
            }

            async fn dial_tcp_like(
                peer_addr: &iroha_primitives::addr::SocketAddr,
                opts: &crate::transport::TcpConnectOptions,
                dial_timeout: Duration,
                connection_id: ConnectionId,
                tls_enabled: bool,
                tls_fallback_to_plain: bool,
            ) -> Result<Connection, crate::Error> {
                if tls_enabled && !cfg!(feature = "p2p_tls") && !tls_fallback_to_plain {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "TLS-only dialing requested but this build does not include iroha_p2p/p2p_tls",
                    )
                    .into());
                }
                #[cfg(feature = "p2p_tls")]
                if tls_enabled {
                    let sni_host = match peer_addr {
                        iroha_primitives::addr::SocketAddr::Host(host) => host.host.as_ref(),
                        _ => "iroha-p2p",
                    };

                    let tls = tokio::time::timeout(dial_timeout, async {
                        let stream = crate::transport::connect(peer_addr, opts).await?;
                        match stream {
                            crate::transport::TcpConnectStream::Plain(tcp) => {
                                let tls = crate::transport::tls::connect_tls(sni_host, tcp).await?;
                                let transport_binding =
                                    Some(crate::transport::tls_peer_certificate_fingerprint(&tls)?);
                                let (read_half, write_half) = tokio::io::split(tls);
                                Ok::<_, std::io::Error>(Connection::from_split_with_binding(
                                    connection_id,
                                    read_half,
                                    write_half,
                                    transport_binding,
                                ))
                            }
                            crate::transport::TcpConnectStream::Tls(proxy_tls) => {
                                let tls =
                                    crate::transport::tls::connect_tls(sni_host, proxy_tls).await?;
                                let transport_binding =
                                    Some(crate::transport::tls_peer_certificate_fingerprint(&tls)?);
                                let (read_half, write_half) = tokio::io::split(tls);
                                Ok::<_, std::io::Error>(Connection::from_split_with_binding(
                                    connection_id,
                                    read_half,
                                    write_half,
                                    transport_binding,
                                ))
                            }
                        }
                    })
                    .await;

                    match tls {
                        Ok(Ok(conn)) => return Ok(conn),
                        Ok(Err(e)) => {
                            if tls_fallback_to_plain {
                                iroha_logger::warn!(
                                    %e,
                                    addr=%peer_addr,
                                    "TLS dial failed; falling back to TCP"
                                );
                            } else {
                                return Err(e.into());
                            }
                        }
                        Err(_) => {
                            let err = std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "tls dial timeout",
                            );
                            if tls_fallback_to_plain {
                                iroha_logger::warn!(
                                    addr=%peer_addr,
                                    timeout=?dial_timeout,
                                    "TLS dial timed out; falling back to TCP"
                                );
                            } else {
                                return Err(err.into());
                            }
                        }
                    }
                }

                let stream = dial_tcp_plain(peer_addr, opts, dial_timeout).await?;
                match stream {
                    crate::transport::TcpConnectStream::Plain(tcp) => {
                        Ok(Connection::new(connection_id, tcp))
                    }
                    #[cfg(feature = "p2p_tls")]
                    crate::transport::TcpConnectStream::Tls(tls) => {
                        let (read_half, write_half) = tokio::io::split(tls);
                        Ok(Connection::from_split(connection_id, read_half, write_half))
                    }
                }
            }

            #[cfg(feature = "quic")]
            async fn dial_quic_like(
                peer_addr: &iroha_primitives::addr::SocketAddr,
                dialer: &crate::transport::QuicDialer,
                dial_timeout: Duration,
                connection_id: ConnectionId,
            ) -> Result<Connection, crate::Error> {
                use tokio::time::Instant;

                const QUIC_SERVER_NAME: &str = "iroha-quic";

                let deadline = Instant::now() + dial_timeout;

                let targets: Vec<std::net::SocketAddr> = match peer_addr {
                    iroha_primitives::addr::SocketAddr::Ipv4(v4) => vec![std::net::SocketAddr::V4(
                        std::net::SocketAddrV4::new(v4.ip.into(), v4.port),
                    )],
                    iroha_primitives::addr::SocketAddr::Ipv6(v6) => vec![std::net::SocketAddr::V6(
                        std::net::SocketAddrV6::new(v6.ip.into(), v6.port, 0, 0),
                    )],
                    iroha_primitives::addr::SocketAddr::Host(host) => {
                        let lookup = tokio::time::timeout_at(
                            deadline,
                            tokio::net::lookup_host((host.host.as_ref(), host.port)),
                        )
                        .await
                        .map_err(|_| {
                            std::io::Error::new(std::io::ErrorKind::TimedOut, "dial timeout")
                        })??;
                        lookup.collect()
                    }
                };

                if targets.is_empty() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "no socket addrs for peer",
                    )
                    .into());
                }

                let mut last_err: Option<std::io::Error> = None;
                for target in targets {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    let remaining = deadline - now;

                    let res = tokio::time::timeout(remaining, async {
                        let conn = dialer.connect(target, QUIC_SERVER_NAME).await?;
                        let transport_binding =
                            Some(crate::transport::quic_peer_certificate_fingerprint(&conn)?);
                        let remote = conn.remote_address();
                        let (send_hi, recv_hi) = conn
                            .open_bi()
                            .await
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                        let lo = tokio::time::timeout_at(deadline, conn.open_bi()).await;
                        let (send_low, recv_low) = match lo {
                            Ok(Ok((s, r))) => (Some(s), Some(r)),
                            Ok(Err(e)) => {
                                iroha_logger::debug!(%e, addr=%target, "QUIC low-priority stream open failed; continuing with single stream");
                                (None, None)
                            }
                            Err(_) => (None, None),
                        };
                        Ok::<_, std::io::Error>(Connection::from_quic(
                            connection_id,
                            conn,
                            send_hi,
                            recv_hi,
                            send_low,
                            recv_low,
                            Some(remote),
                            transport_binding,
                        ))
                    })
                    .await;

                    match res {
                        Ok(Ok(conn)) => return Ok(conn),
                        Ok(Err(e)) => last_err = Some(e),
                        Err(_) => {
                            last_err = Some(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "dial timeout",
                            ))
                        }
                    }
                }

                Err(last_err
                    .unwrap_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::Other, "quic dial failed")
                    })
                    .into())
            }

            #[cfg(feature = "p2p_ws")]
            fn ws_endpoint(peer_addr: &iroha_primitives::addr::SocketAddr) -> String {
                match peer_addr {
                    iroha_primitives::addr::SocketAddr::Ipv4(addr) => {
                        format!("{}:{}", addr.ip, addr.port)
                    }
                    iroha_primitives::addr::SocketAddr::Ipv6(addr) => {
                        // URLs require brackets around IPv6 literals.
                        format!("[{}]:{}", addr.ip, addr.port)
                    }
                    iroha_primitives::addr::SocketAddr::Host(addr) => {
                        format!("{}:{}", addr.host.as_ref(), addr.port)
                    }
                }
            }

            let tcp_opts = crate::transport::TcpConnectOptions {
                proxy: proxy_policy,
                proxy_tls_verify,
                proxy_tls_pinned_cert_der,
                tcp_nodelay,
                tcp_keepalive,
            };

            #[cfg(feature = "p2p_ws")]
            let mut ws_tried = false;
            #[cfg(not(feature = "p2p_ws"))]
            let ws_tried = true;

            #[cfg(feature = "p2p_ws")]
            if prefer_ws_fallback {
                ws_tried = true;
                let endpoint = ws_endpoint(&peer_addr);
                if let Some(conn) = dial_ws(
                    &peer_addr,
                    &endpoint,
                    &tcp_opts,
                    connection_id,
                    dial_timeout,
                    tls_enabled,
                )
                .await
                {
                    return Ok(ConnectedTo {
                        our_public_address,
                        expected_peer_id: peer_id.clone(),
                        key_pair,
                        connection: conn,
                        chain_id,
                        consensus_caps,
                        confidential_caps,
                        crypto_caps,
                        soranet_handshake,
                        local_scion_supported,
                        trust_gossip,
                        relay_role,
                    });
                }
            }

            if prefer_scion {
                #[cfg(feature = "quic")]
                if let Some(dialer) = &quic_dialer {
                    match dial_quic_like(&peer_addr, dialer, dial_timeout, connection_id).await {
                        Ok(connection) => {
                            crate::network::inc_scion_outbound();
                            return Ok(ConnectedTo {
                                our_public_address,
                                expected_peer_id: peer_id.clone(),
                                key_pair,
                                connection,
                                chain_id,
                                consensus_caps,
                                confidential_caps,
                                crypto_caps,
                                soranet_handshake,
                                local_scion_supported,
                                trust_gossip,
                                relay_role,
                            });
                        }
                        Err(err) => {
                            iroha_logger::warn!(
                                %err,
                                peer=%peer_addr,
                                "SCION-preferred dial failed; falling back to legacy dial strategy"
                            );
                        }
                    }
                }
            }

            let tcp_fut = dial_tcp_like(
                &peer_addr,
                &tcp_opts,
                dial_timeout,
                connection_id,
                tls_enabled,
                tls_fallback_to_plain,
            );
            tokio::pin!(tcp_fut);

            let connection_result: Result<Connection, crate::Error> = {
                #[cfg(feature = "quic")]
                {
                    if quic_enabled {
                        if let Some(dialer) = &quic_dialer {
                            let quic_fut =
                                dial_quic_like(&peer_addr, dialer, dial_timeout, connection_id);
                            tokio::pin!(quic_fut);

                            // Phase 1: give QUIC a head start, but don't stall on blocked UDP.
                            let stagger = tokio::time::sleep(happy_eyeballs_stagger);
                            tokio::pin!(stagger);
                            tokio::select! {
                                res = &mut quic_fut => match res {
                                    Ok(conn) => Ok(conn),
                                    Err(e) => {
                                        iroha_logger::warn!(%e, addr=%peer_addr, "QUIC dial failed; falling back to TCP-like");
                                        tcp_fut.await
                                    }
                                },
                                () = &mut stagger => {
                                    let mut quic_err: Option<crate::Error> = None;
                                    let mut tcp_err: Option<crate::Error> = None;
                                    loop {
                                        tokio::select! {
                                            res = &mut quic_fut, if quic_err.is_none() => match res {
                                                Ok(conn) => break Ok(conn),
                                                Err(e) => {
                                                    iroha_logger::debug!(%e, addr=%peer_addr, "QUIC dial failed while racing TCP-like");
                                                    quic_err = Some(e);
                                                    if tcp_err.is_some() {
                                                        break Err(quic_err.take().unwrap());
                                                    }
                                                }
                                            },
                                            res = &mut tcp_fut, if tcp_err.is_none() => match res {
                                                Ok(conn) => break Ok(conn),
                                                Err(e) => {
                                                    iroha_logger::debug!(%e, addr=%peer_addr, "TCP-like dial failed while racing QUIC");
                                                    tcp_err = Some(e);
                                                    if quic_err.is_some() {
                                                        break Err(tcp_err.take().unwrap());
                                                    }
                                                }
                                            },
                                            else => {
                                                break Err(tcp_err.or(quic_err).unwrap_or_else(|| {
                                                    std::io::Error::new(std::io::ErrorKind::Other, "dial failed").into()
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            tcp_fut.await
                        }
                    } else {
                        tcp_fut.await
                    }
                }

                #[cfg(not(feature = "quic"))]
                {
                    tcp_fut.await
                }
            };

            let connection = match connection_result {
                Ok(conn) => conn,
                Err(err) => {
                    #[cfg(feature = "p2p_ws")]
                    if !ws_tried {
                        let should_try_ws = prefer_ws_fallback
                            || matches!(peer_addr, iroha_primitives::addr::SocketAddr::Host(_));
                        if should_try_ws {
                            let endpoint = ws_endpoint(&peer_addr);
                            if let Some(conn) = dial_ws(
                                &peer_addr,
                                &endpoint,
                                &tcp_opts,
                                connection_id,
                                dial_timeout,
                                tls_enabled,
                            )
                            .await
                            {
                                return Ok(ConnectedTo {
                                    our_public_address,
                                    expected_peer_id: peer_id.clone(),
                                    key_pair,
                                    connection: conn,
                                    chain_id,
                                    consensus_caps,
                                    confidential_caps,
                                    crypto_caps,
                                    soranet_handshake,
                                    local_scion_supported,
                                    trust_gossip,
                                    relay_role,
                                });
                            }
                        }
                    }

                    crate::network::inc_dns_resolution_fail();
                    return Err(err);
                }
            };
            Ok(ConnectedTo {
                our_public_address,
                expected_peer_id: peer_id,
                key_pair,
                connection,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
                local_scion_supported,
                trust_gossip,
                relay_role,
            })
        }
    }

    #[cfg(test)]
    mod dial_policy_tests {
        use std::{sync::Arc, time::Duration};

        use super::*;

        fn connecting_to(
            peer_addr: std::net::SocketAddr,
            tls_enabled: bool,
            tls_fallback_to_plain: bool,
        ) -> Connecting {
            let our_public_address: std::net::SocketAddr = "127.0.0.1:0".parse().expect("addr");
            Connecting {
                peer_addr: peer_addr.into(),
                peer_id: iroha_data_model::prelude::PeerId::from(
                    KeyPair::random().public_key().clone(),
                ),
                our_public_address: our_public_address.into(),
                key_pair: KeyPair::random(),
                connection_id: 0,
                chain_id: None,
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                soranet_handshake: Arc::new(SoranetHandshakeConfig::defaults()),
                quic_enabled: false,
                tls_enabled,
                tls_fallback_to_plain,
                prefer_scion: false,
                local_scion_supported: true,
                prefer_ws_fallback: false,
                trust_gossip: false,
                relay_role: RelayRole::Disabled,
                dial_timeout: Duration::from_millis(200),
                happy_eyeballs_stagger: Duration::from_millis(10),
                tcp_nodelay: true,
                tcp_keepalive: None,
                proxy_tls_verify: true,
                proxy_tls_pinned_cert_der: None,
                proxy_policy: crate::transport::ProxyPolicy::disabled(),
                quic_dialer: None,
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn scion_preference_falls_back_to_legacy_when_unavailable() {
            let addr: std::net::SocketAddr = "127.0.0.1:1".parse().expect("addr");
            let mut connecting =
                connecting_to(addr, /*tls_enabled=*/ false, /*fallback=*/ true);
            connecting.prefer_scion = true;

            let res = Connecting::connect_to(connecting).await;
            assert!(matches!(
                res,
                Err(crate::Error::Io(e))
                    if e.kind() != std::io::ErrorKind::InvalidInput
                        && e.kind() != std::io::ErrorKind::NotFound
            ));
        }

        #[cfg(not(feature = "p2p_tls"))]
        #[tokio::test(flavor = "current_thread")]
        async fn tls_only_dial_requires_p2p_tls_feature_when_no_fallback() {
            let addr: std::net::SocketAddr = "127.0.0.1:1".parse().expect("addr");
            let connecting =
                connecting_to(addr, /*tls_enabled=*/ true, /*fallback=*/ false);
            let res = Connecting::connect_to(connecting).await;
            assert!(matches!(
                res,
                Err(crate::Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput
            ));
        }

        #[cfg(feature = "p2p_tls")]
        #[tokio::test(flavor = "current_thread")]
        async fn tls_dial_falls_back_to_plain_when_enabled() {
            use tokio::net::TcpListener;

            let listener = match TcpListener::bind("127.0.0.1:0").await {
                Ok(listener) => listener,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(e) => panic!("listener bind failed: {e:?}"),
            };
            let addr = listener.local_addr().expect("local addr");

            let accept_task = tokio::spawn(async move {
                loop {
                    let (sock, _) = match listener.accept().await {
                        Ok(ok) => ok,
                        Err(_) => break,
                    };
                    tokio::spawn(async move {
                        let _sock = sock;
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    });
                }
            });

            let ok = Connecting::connect_to(connecting_to(
                addr, /*tls_enabled=*/ true, /*fallback=*/ true,
            ))
            .await;
            assert!(ok.is_ok(), "TLS failure should fall back to TCP");

            let err = Connecting::connect_to(connecting_to(
                addr, /*tls_enabled=*/ true, /*fallback=*/ false,
            ))
            .await;
            assert!(err.is_err(), "TLS-only should not fall back to TCP");

            accept_task.abort();
        }
    }

    /// Peer that is being connected to.
    pub(super) struct ConnectedTo {
        our_public_address: SocketAddr,
        expected_peer_id: iroha_data_model::prelude::PeerId,
        key_pair: KeyPair,
        connection: Connection,
        chain_id: Option<iroha_data_model::ChainId>,
        consensus_caps: Option<ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        soranet_handshake: Arc<SoranetHandshakeConfig>,
        local_scion_supported: bool,
        trust_gossip: bool,
        relay_role: RelayRole,
    }

    impl ConnectedTo {
        #[allow(clippy::similar_names, clippy::too_many_lines)]
        pub(super) async fn send_client_hello<K: Kex, E: Enc>(
            Self {
                our_public_address,
                expected_peer_id,
                key_pair,
                mut connection,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
                local_scion_supported,
                trust_gossip,
                relay_role,
            }: Self,
        ) -> Result<SendKey<K, E>, crate::Error> {
            // Pre-handshake header: write ours, then read theirs.
            if let Err(e) = write_pre_handshake_header(&mut connection.write).await {
                return Err(crate::Error::from(e));
            }
            if let Err(e) = read_and_verify_pre_handshake_header(&mut connection.read).await {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    return Err(crate::Error::HandshakeBadPreface);
                }
                return Err(crate::Error::from(e));
            }
            let runtime_params = soranet_handshake.runtime_params();
            let mut rng = StdRng::from_os_rng();

            if let Some(minted) = soranet_handshake
                .mint_challenge_ticket(&mut rng)
                .map_err(|err| Error::HandshakeSoranet(err.to_string()))?
            {
                for frame in &minted.frames {
                    write_handshake_frame(&mut connection.write, frame).await?;
                }
            }

            let (client_hello, client_state) = build_client_hello(&runtime_params, &mut rng)
                .map_err(|err| Error::HandshakeSoranet(err.to_string()))?;
            write_handshake_frame(&mut connection.write, &client_hello).await?;

            let relay_hello = read_handshake_frame(&mut connection.read).await?;
            let (client_finish, secrets) = match client_handle_relay_hello(
                client_state,
                &relay_hello,
                &key_pair,
                &runtime_params,
                &mut rng,
            ) {
                Ok(success) => success,
                Err(HarnessError::Downgrade {
                    warnings,
                    telemetry,
                }) => {
                    let warning_messages = warnings
                        .iter()
                        .map(|w| w.message.clone())
                        .collect::<Vec<_>>();
                    if let Some(payload) = telemetry {
                        iroha_logger::warn!(
                            payload = %String::from_utf8_lossy(&payload),
                            "SoraNet handshake downgrade telemetry"
                        );
                    }
                    iroha_logger::warn!(
                        warnings = ?warning_messages,
                        "SoraNet handshake downgrade detected (outbound)"
                    );
                    let summary = if warning_messages.is_empty() {
                        "capability downgrade detected".to_string()
                    } else {
                        format!(
                            "capability downgrade detected: {}",
                            warning_messages.join("; ")
                        )
                    };
                    return Err(Error::HandshakeSoranet(summary));
                }
                Err(err) => return Err(Error::HandshakeSoranet(err.to_string())),
            };
            if let Some(client_finish) = client_finish {
                write_handshake_frame(&mut connection.write, &client_finish).await?;
            }

            if !secrets.warnings.is_empty() {
                iroha_logger::warn!(
                    warnings = ?secrets
                        .warnings
                        .iter()
                        .map(|w| w.message.clone())
                        .collect::<Vec<_>>(),
                    "SoraNet handshake reported capability warnings"
                );
            }
            if let Some(payload) = secrets.telemetry_payload.as_ref() {
                iroha_logger::debug!(
                    payload = %String::from_utf8_lossy(payload),
                    "SoraNet handshake telemetry"
                );
            }

            let cryptographer = {
                #[cfg(feature = "noise_handshake")]
                {
                    let key_bytes =
                        noise_handshake_initiator(&mut connection.read, &mut connection.write)
                            .await?;
                    Cryptographer::new_with_raw_key_bytes(&key_bytes)?
                }
                #[cfg(not(feature = "noise_handshake"))]
                {
                    let session_key = SessionKey::new(secrets.session_key);
                    Cryptographer::new(&session_key)?
                }
            };
            let kx_local_pk = K::new().keypair(KeyGenOption::Random).0;
            let kx_remote_pk = K::new().keypair(KeyGenOption::Random).0;
            Ok(SendKey {
                our_public_address,
                expected_peer_id: Some(expected_peer_id),
                key_pair,
                kx_local_pk,
                kx_remote_pk,
                connection,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            })
        }
    }

    /// Peer that is being connected from
    pub(super) struct ConnectedFrom {
        pub our_public_address: SocketAddr,
        pub key_pair: KeyPair,
        pub connection: Connection,
        pub chain_id: Option<iroha_data_model::ChainId>,
        pub consensus_caps: Option<ConsensusHandshakeCaps>,
        pub confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub soranet_handshake: Arc<SoranetHandshakeConfig>,
        pub local_scion_supported: bool,
        pub trust_gossip: bool,
        pub relay_role: RelayRole,
    }

    impl ConnectedFrom {
        #[allow(clippy::similar_names, clippy::too_many_lines)]
        pub(super) async fn read_client_hello<K: Kex, E: Enc>(
            Self {
                our_public_address,
                key_pair,
                mut connection,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
                local_scion_supported,
                trust_gossip,
                relay_role,
            }: Self,
        ) -> Result<SendKey<K, E>, crate::Error> {
            // Pre-handshake header: read theirs, then write ours.
            if let Err(e) = read_and_verify_pre_handshake_header(&mut connection.read).await {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    return Err(crate::Error::HandshakeBadPreface);
                }
                return Err(crate::Error::from(e));
            }
            if let Err(e) = write_pre_handshake_header(&mut connection.write).await {
                return Err(crate::Error::from(e));
            }
            let runtime_params = soranet_handshake.runtime_params();
            let mut rng = StdRng::from_os_rng();

            if soranet_handshake.pow_required() {
                let ticket = read_handshake_frame(&mut connection.read).await?;
                soranet_handshake
                    .verify_challenge_ticket(&ticket)
                    .map_err(|err| Error::HandshakeSoranet(err.to_string()))?;
            }

            let client_hello = read_handshake_frame(&mut connection.read).await?;
            let (relay_hello, relay_state) =
                match process_client_hello(&client_hello, &runtime_params, &key_pair, &mut rng) {
                    Ok(success) => success,
                    Err(HarnessError::Downgrade {
                        warnings,
                        telemetry,
                    }) => {
                        let warning_messages = warnings
                            .iter()
                            .map(|w| w.message.clone())
                            .collect::<Vec<_>>();
                        if let Some(payload) = telemetry {
                            iroha_logger::warn!(
                                payload = %String::from_utf8_lossy(&payload),
                                "SoraNet handshake downgrade telemetry"
                            );
                        }
                        iroha_logger::warn!(
                            warnings = ?warning_messages,
                            "SoraNet handshake downgrade detected (inbound)"
                        );
                        let summary = if warning_messages.is_empty() {
                            "capability downgrade detected".to_string()
                        } else {
                            format!(
                                "capability downgrade detected: {}",
                                warning_messages.join("; ")
                            )
                        };
                        return Err(Error::HandshakeSoranet(summary));
                    }
                    Err(err) => return Err(Error::HandshakeSoranet(err.to_string())),
                };
            write_handshake_frame(&mut connection.write, &relay_hello).await?;

            let secrets = if relay_state.requires_client_finish() {
                let client_finish = read_handshake_frame(&mut connection.read).await?;
                relay_finalize_handshake(relay_state, &client_finish, &key_pair)
                    .map_err(|err| Error::HandshakeSoranet(err.to_string()))?
            } else {
                relay_finalize_handshake(relay_state, &[], &key_pair)
                    .map_err(|err| Error::HandshakeSoranet(err.to_string()))?
            };

            if !secrets.warnings.is_empty() {
                iroha_logger::warn!(
                    warnings = ?secrets
                        .warnings
                        .iter()
                        .map(|w| w.message.clone())
                        .collect::<Vec<_>>(),
                    "SoraNet handshake reported capability warnings"
                );
            }
            if let Some(payload) = secrets.telemetry_payload.as_ref() {
                iroha_logger::debug!(
                    payload = %String::from_utf8_lossy(payload),
                    "SoraNet handshake telemetry"
                );
            }

            let cryptographer = {
                #[cfg(feature = "noise_handshake")]
                {
                    let key_bytes =
                        noise_handshake_responder(&mut connection.read, &mut connection.write)
                            .await?;
                    Cryptographer::new_with_raw_key_bytes(&key_bytes)?
                }
                #[cfg(not(feature = "noise_handshake"))]
                {
                    let session_key = SessionKey::new(secrets.session_key);
                    Cryptographer::new(&session_key)?
                }
            };
            let kx_local_pk = K::new().keypair(KeyGenOption::Random).0;
            let kx_remote_pk = K::new().keypair(KeyGenOption::Random).0;
            Ok(SendKey {
                our_public_address,
                expected_peer_id: None,
                key_pair,
                kx_local_pk,
                kx_remote_pk,
                connection,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            })
        }
    }

    #[cfg(test)]
    pub(super) struct SendKeyInit<K: Kex, E: Enc> {
        pub(super) our_public_address: SocketAddr,
        pub(super) expected_peer_id: Option<iroha_data_model::prelude::PeerId>,
        pub(super) key_pair: KeyPair,
        pub(super) kx_local_pk: K::PublicKey,
        pub(super) kx_remote_pk: K::PublicKey,
        pub(super) connection: Connection,
        pub(super) cryptographer: Cryptographer<E>,
        pub(super) chain_id: Option<iroha_data_model::ChainId>,
        pub(super) consensus_caps: Option<ConsensusHandshakeCaps>,
        pub(super) confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub(super) crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub(super) relay_role: RelayRole,
        pub(super) local_scion_supported: bool,
        pub(super) trust_gossip: bool,
    }

    /// Peer that needs to send key.
    pub(super) struct SendKey<K: Kex, E: Enc> {
        pub(super) our_public_address: SocketAddr,
        pub(super) expected_peer_id: Option<iroha_data_model::prelude::PeerId>,
        pub(super) key_pair: KeyPair,
        pub(super) kx_local_pk: K::PublicKey,
        pub(super) kx_remote_pk: K::PublicKey,
        pub(super) connection: Connection,
        pub(super) cryptographer: Cryptographer<E>,
        pub(super) chain_id: Option<iroha_data_model::ChainId>,
        pub(super) consensus_caps: Option<ConsensusHandshakeCaps>,
        pub(super) confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub(super) crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub(super) relay_role: RelayRole,
        pub(super) local_scion_supported: bool,
        pub(super) trust_gossip: bool,
    }

    impl<K: Kex, E: Enc> SendKey<K, E> {
        #[cfg(test)]
        pub(super) fn new(init: SendKeyInit<K, E>) -> Self {
            let SendKeyInit {
                our_public_address,
                expected_peer_id,
                key_pair,
                kx_local_pk,
                kx_remote_pk,
                connection,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            } = init;
            Self {
                our_public_address,
                expected_peer_id,
                key_pair,
                kx_local_pk,
                kx_remote_pk,
                connection,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            }
        }

        pub(super) async fn send_our_public_key(
            Self {
                our_public_address,
                expected_peer_id,
                key_pair,
                kx_local_pk,
                kx_remote_pk,
                mut connection,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            }: Self,
        ) -> Result<GetKey<K, E>, crate::Error> {
            let write_half = &mut connection.write;

            let our_addr = our_public_address;
            let payload = handshake_signature_payload::<K, E>(
                &cryptographer,
                &our_addr,
                &kx_local_pk,
                &kx_remote_pk,
                chain_id.as_ref(),
                connection.transport_binding.as_ref(),
            );
            let signature = Signature::new(key_pair.private_key(), &payload);
            let (alg, pk_bytes) = key_pair.public_key().to_bytes();
            let hello = HandshakeHelloV1 {
                algorithm: alg,
                public_key: pk_bytes.to_vec(),
                signature: signature.payload().to_vec(),
                addr: our_addr,
                relay: relay_role,
                consensus: build_consensus_meta(consensus_caps.as_ref()),
                confidential: build_confidential_meta(confidential_caps.as_ref()),
                crypto: build_crypto_meta(crypto_caps.as_ref()),
                trust: build_trust_meta(trust_gossip, local_scion_supported),
            };
            let encrypted = encode_handshake_message(&cryptographer, &hello)?;

            // Handshake messages can exceed 255 bytes once they include the
            // peer's public address and additional metadata. Encode the
            // payload length as a two-byte big-endian integer to support
            // larger messages.
            #[allow(clippy::cast_possible_truncation)]
            let size = u16::try_from(encrypted.len())
                .map_err(|_| crate::Error::HandshakeMessageTooLarge)?;
            let mut buf = Vec::<u8>::with_capacity(encrypted.len() + 2);
            buf.extend_from_slice(&size.to_be_bytes());
            buf.extend_from_slice(&encrypted);

            write_half.write_all(&buf).await?;
            write_half.flush().await?;
            Ok(GetKey {
                connection,
                expected_peer_id,
                kx_local_pk,
                kx_remote_pk,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                local_scion_supported,
                trust_gossip,
            })
        }
    }

    /// Peer that needs to get key.
    pub struct GetKey<K: Kex, E: Enc> {
        pub(super) connection: Connection,
        pub(super) expected_peer_id: Option<iroha_data_model::prelude::PeerId>,
        pub(super) kx_local_pk: K::PublicKey,
        pub(super) kx_remote_pk: K::PublicKey,
        pub(super) cryptographer: Cryptographer<E>,
        pub(super) chain_id: Option<iroha_data_model::ChainId>,
        pub(super) consensus_caps: Option<ConsensusHandshakeCaps>,
        pub(super) confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub(super) crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub(super) relay_role: RelayRole,
        pub(super) local_scion_supported: bool,
        pub(super) trust_gossip: bool,
    }

    impl<K: Kex, E: Enc> GetKey<K, E> {
        /// Read the peer's public key
        pub(super) async fn read_their_public_key(
            Self {
                mut connection,
                expected_peer_id,
                kx_local_pk,
                kx_remote_pk,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role: _relay_role,
                local_scion_supported: _local_scion_supported,
                trust_gossip,
            }: Self,
        ) -> Result<Ready<E>, crate::Error> {
            let read_half = &mut connection.read;
            // Read the length prefix encoded as a two-byte big-endian integer.
            let size = read_half.read_u16().await? as usize;
            // Reading public key
            let mut data = vec![0_u8; size];
            let _ = read_half.read_exact(&mut data).await?;

            let hello = decode_handshake_message(&cryptographer, data.as_slice())?;
            let (
                algorithm,
                public_key,
                signature,
                remote_public_address,
                relay,
                consensus,
                confidential,
                crypto,
                trust_gossip_remote,
                scion_supported_remote,
            ) = match hello {
                HandshakeHello::V1(HandshakeHelloV1 {
                    algorithm,
                    public_key,
                    signature,
                    addr,
                    relay,
                    consensus,
                    confidential,
                    crypto,
                    trust,
                }) => (
                    algorithm,
                    public_key,
                    signature,
                    addr,
                    relay,
                    consensus,
                    confidential,
                    crypto,
                    trust.trust_gossip,
                    trust.scion_supported,
                ),
            };
            let remote_pub_key = match PublicKey::from_bytes(algorithm, &public_key) {
                Ok(pk) => pk,
                Err(e) => return Err(crate::Error::from(iroha_crypto::error::Error::from(e))),
            };
            let signature = Signature::from_bytes(&signature);

            let payload = handshake_signature_payload::<K, E>(
                &cryptographer,
                &remote_public_address,
                &kx_remote_pk,
                &kx_local_pk,
                chain_id.as_ref(),
                connection.transport_binding.as_ref(),
            );
            signature.verify(&remote_pub_key, &payload)?;

            if let Some(expected_peer_id) = expected_peer_id {
                let found_peer_id = iroha_data_model::prelude::PeerId::from(remote_pub_key.clone());
                if found_peer_id != expected_peer_id {
                    return Err(crate::Error::HandshakePeerMismatch {
                        expected: expected_peer_id,
                        found: found_peer_id,
                    });
                }
            }

            enforce_consensus_caps(consensus_caps.as_ref(), &consensus)?;
            enforce_confidential_caps(
                confidential_caps.as_ref(),
                &confidential,
                &remote_public_address,
            )?;
            enforce_crypto_caps(crypto_caps.as_ref(), &crypto, &remote_public_address)?;

            let peer = Peer::new(remote_public_address, remote_pub_key);
            let trust_gossip = trust_gossip && trust_gossip_remote;
            let scion_supported = scion_supported_remote;

            Ok(Ready {
                peer,
                connection,
                cryptographer,
                relay_role: relay,
                scion_supported,
                trust_gossip,
            })
        }
    }

    /// Peer that is ready for communication after finishing the
    /// handshake process.
    pub(super) struct Ready<E: Enc> {
        pub peer: Peer,
        pub connection: Connection,
        pub cryptographer: Cryptographer<E>,
        pub relay_role: RelayRole,
        pub scion_supported: bool,
        pub trust_gossip: bool,
    }

    #[allow(dead_code)]
    fn create_payload<K: Kex>(kx_local_pk: &K::PublicKey, kx_remote_pk: &K::PublicKey) -> Vec<u8> {
        let mut payload = K::encode_public_key(kx_local_pk);
        let remote = K::encode_public_key(kx_remote_pk);
        payload.extend_from_slice(remote.as_ref());
        payload
    }

    /// Create a signature payload that binds ephemeral keys to the advertised address
    /// and optionally to the chain id.
    #[allow(dead_code)]
    pub(super) fn create_payload_with_address<K: Kex>(
        kx_local_pk: &K::PublicKey,
        kx_remote_pk: &K::PublicKey,
        addr: &iroha_primitives::addr::SocketAddr,
        chain_id: Option<&iroha_data_model::ChainId>,
    ) -> Vec<u8> {
        let mut payload = create_payload::<K>(kx_local_pk, kx_remote_pk);
        // Append Norito-encoded address bytes deterministically
        let addr_bytes = addr.encode();
        payload.extend_from_slice(&addr_bytes);
        #[cfg(feature = "handshake_chain_id")]
        if let Some(chain_id) = chain_id {
            let chain_bytes = chain_id.encode();
            payload.extend_from_slice(&chain_bytes);
        }
        #[cfg(not(feature = "handshake_chain_id"))]
        let _ = chain_id; // suppress unused parameter warning when feature is disabled
        payload
    }

    #[cfg(test)]
    mod tests {
        #[cfg(feature = "noise_handshake")]
        use std::sync::Arc;

        #[cfg(feature = "noise_handshake")]
        use iroha_crypto::{encryption::ChaCha20Poly1305, kex::X25519Sha256 as KexAlgo};

        #[cfg(feature = "noise_handshake")]
        use super::*;

        #[cfg(feature = "noise_handshake")]
        #[tokio::test(flavor = "current_thread")]
        async fn noise_handshake_derives_shared_disambiguator() {
            let soranet = Arc::new(SoranetHandshakeConfig::defaults());
            let key_pair_a = KeyPair::random();
            let key_pair_b = KeyPair::random();
            let addr_a: SocketAddr = "127.0.0.1:10001".parse().unwrap();
            let addr_b: SocketAddr = "127.0.0.1:10002".parse().unwrap();

            let (stream_a, stream_b) = tokio::io::duplex(2048);
            let (read_a, write_a) = tokio::io::split(stream_a);
            let (read_b, write_b) = tokio::io::split(stream_b);

            let outbound = ConnectedTo {
                our_public_address: addr_a,
                expected_peer_id: iroha_data_model::prelude::PeerId::from(
                    key_pair_b.public_key().clone(),
                ),
                key_pair: key_pair_a,
                connection: Connection::from_split(1, read_a, write_a),
                chain_id: None,
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                soranet_handshake: soranet.clone(),
                local_scion_supported: true,
                trust_gossip: true,
                relay_role: RelayRole::Disabled,
            };
            let inbound = ConnectedFrom {
                our_public_address: addr_b,
                key_pair: key_pair_b,
                connection: Connection::from_split(2, read_b, write_b),
                chain_id: None,
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                soranet_handshake: soranet.clone(),
                local_scion_supported: true,
                trust_gossip: true,
                relay_role: RelayRole::Disabled,
            };

            let (out_res, in_res) = tokio::join!(
                ConnectedTo::send_client_hello::<KexAlgo, ChaCha20Poly1305>(outbound),
                ConnectedFrom::read_client_hello::<KexAlgo, ChaCha20Poly1305>(inbound),
            );
            let outbound = out_res.expect("outbound handshake");
            let inbound = in_res.expect("inbound handshake");

            assert_eq!(
                outbound.cryptographer.disambiguator, inbound.cryptographer.disambiguator,
                "noise handshake must yield a shared disambiguator"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
    };

    use iroha_crypto::{
        KeyGenOption, KeyPair,
        encryption::ChaCha20Poly1305,
        kex::{KeyExchangeScheme, X25519Sha256 as KexAlgo},
    };
    use iroha_primitives::addr::SocketAddr;
    use norito::codec::Encode;
    use tokio::io::AsyncWrite;

    use super::{Connection, SoranetHandshakeConfig, cryptographer::Cryptographer, state::*};
    use crate::{ConfidentialHandshakeCaps, RelayRole};

    struct TrackingWrite {
        buffer: Vec<u8>,
        flushes: usize,
    }

    impl TrackingWrite {
        fn new() -> Self {
            Self {
                buffer: Vec::new(),
                flushes: 0,
            }
        }
    }

    impl AsyncWrite for TrackingWrite {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.buffer.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.flushes = self.flushes.saturating_add(1);
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_writes_flush_frames() {
        let mut writer = TrackingWrite::new();
        super::write_pre_handshake_header(&mut writer)
            .await
            .expect("preface write");
        assert_eq!(writer.flushes, 1, "preface should flush once");

        let payload = b"hello";
        super::write_handshake_frame(&mut writer, payload)
            .await
            .expect("handshake frame write");
        assert_eq!(writer.flushes, 2, "handshake frame should flush once");

        let mut expected = Vec::from(&super::PRE_MAGIC[..]);
        expected.push(super::PRE_VERSION);
        assert_eq!(
            &writer.buffer[..expected.len()],
            expected.as_slice(),
            "preface bytes should be written first"
        );

        let frame = &writer.buffer[expected.len()..];
        assert_eq!(frame.len(), 2 + payload.len());
        let len = u16::from_be_bytes([frame[0], frame[1]]);
        assert_eq!(len as usize, payload.len());
        assert_eq!(&frame[2..], payload);
    }

    #[test]
    fn payload_with_address_is_consistent_between_sides() {
        // Generate ephemeral keypairs for both sides
        let kx = KexAlgo::new();
        let (a_pk, _a_sk) = kx.keypair(KeyGenOption::Random);
        let (b_pk, _b_sk) = kx.keypair(KeyGenOption::Random);

        // Sender uses (local=a, remote=b) and their own address
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let chain_id: Option<iroha_data_model::ChainId> = None;
        let sender_payload =
            create_payload_with_address::<KexAlgo>(&a_pk, &b_pk, &addr, chain_id.as_ref());

        // Receiver verifies using (remote=a, local=b) and the same advertised address
        let receiver_payload =
            create_payload_with_address::<KexAlgo>(&a_pk, &b_pk, &addr, chain_id.as_ref());

        assert_eq!(sender_payload, receiver_payload);
    }

    #[test]
    fn payload_differs_when_chain_id_is_added() {
        let kx = KexAlgo::new();
        let (a_pk, _a_sk) = kx.keypair(KeyGenOption::Random);
        let (b_pk, _b_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();

        let without = create_payload_with_address::<KexAlgo>(&a_pk, &b_pk, &addr, None);

        let chain_id: iroha_data_model::ChainId =
            "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let with = create_payload_with_address::<KexAlgo>(&a_pk, &b_pk, &addr, Some(&chain_id));

        if cfg!(feature = "handshake_chain_id") {
            assert_ne!(without, with);
        } else {
            assert_eq!(without, with);
        }
    }

    #[test]
    fn untagged_handshake_is_rejected() {
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[9u8; 32]).unwrap();
        let key_pair = KeyPair::random();
        let (alg, pk_bytes) = key_pair.public_key().to_bytes();
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = HandshakeHelloV1 {
            algorithm: alg,
            public_key: pk_bytes.to_vec(),
            signature: vec![0u8; 64],
            addr: addr.clone(),
            relay: RelayRole::Disabled,
            consensus: HandshakeConsensusMeta {
                mode_tag: None,
                proto_version: None,
                consensus_fingerprint: None,
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: None,
                assume_valid: None,
                verifier_backend: None,
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: None,
                sm_openssl_preview: None,
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: false,
            },
        };

        let raw = hello.encode();
        let encrypted = cryptographer.encrypt(&raw).expect("encrypt raw handshake");
        let decoded = decode_handshake_message(&cryptographer, &encrypted);
        assert!(
            matches!(decoded, Err(crate::Error::Format)),
            "untagged handshake must be rejected"
        );
    }

    #[test]
    fn versioned_handshake_preserves_trust_flag() {
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[11u8; 32]).unwrap();
        let key_pair = KeyPair::random();
        let (alg, pk_bytes) = key_pair.public_key().to_bytes();
        let addr: SocketAddr = "127.0.0.1:1444".parse().unwrap();
        let hello = HandshakeHelloV1 {
            algorithm: alg,
            public_key: pk_bytes.to_vec(),
            signature: vec![1u8; 64],
            addr: addr.clone(),
            relay: RelayRole::Hub,
            consensus: HandshakeConsensusMeta {
                mode_tag: Some("mode".to_string()),
                proto_version: Some(1),
                consensus_fingerprint: Some([7u8; 32]),
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: Some(true),
                assume_valid: Some(false),
                verifier_backend: Some("backend".to_string()),
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: Some(false),
                sm_openssl_preview: Some(false),
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: true,
            },
        };

        let encrypted =
            encode_handshake_message(&cryptographer, &hello).expect("encode v1 handshake");
        let decoded =
            decode_handshake_message(&cryptographer, &encrypted).expect("decode v1 handshake");
        let HandshakeHello::V1(v1) = decoded;
        assert_eq!(v1.addr, addr);
        assert!(v1.trust.trust_gossip);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_fails_when_metadata_exceeds_limit() {
        let kx = KexAlgo::new();
        let (kx_local_pk, _kx_local_sk) = kx.keypair(KeyGenOption::Random);
        let (kx_remote_pk, _kx_remote_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let key_pair = KeyPair::random();
        let connection = Connection::from_split(7, tokio::io::empty(), tokio::io::sink());
        let cryptographer =
            super::cryptographer::Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(
                &[42u8; 32],
            )
            .expect("valid key length");
        let caps = ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-".repeat(7000),
            features: None,
        };
        let send_key = SendKey::<KexAlgo, ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            kx_local_pk,
            kx_remote_pk,
            connection,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: Some(caps),
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });
        let err = match SendKey::<KexAlgo, ChaCha20Poly1305>::send_our_public_key(send_key).await {
            Ok(_) => panic!("expected HandshakeMessageTooLarge error"),
            Err(err) => err,
        };
        assert!(
            matches!(err, crate::Error::HandshakeMessageTooLarge),
            "expected HandshakeMessageTooLarge, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_v1_defaults_to_trust_gossip() {
        let kx = KexAlgo::new();
        let (sender_kx, _sender_sk) = kx.keypair(KeyGenOption::Random);
        let (receiver_kx, _receiver_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[7u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<KexAlgo, ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr.clone(),
            expected_peer_id: None,
            key_pair,
            kx_local_pk: sender_kx.clone(),
            kx_remote_pk: receiver_kx.clone(),
            connection: Connection::from_split(1, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<KexAlgo, ChaCha20Poly1305> {
            connection: Connection::from_split(2, receiver_read, receiver_write),
            expected_peer_id: None,
            kx_local_pk: receiver_kx,
            kx_remote_pk: sender_kx,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let ready = GetKey::read_their_public_key(get_key)
            .await
            .expect("handshake should succeed");
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert!(ready.trust_gossip, "handshake should enable trust gossip");
        assert!(
            ready.scion_supported,
            "handshake should propagate SCION support flag"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_accepts_matching_transport_binding() {
        let kx = KexAlgo::new();
        let (sender_kx, _sender_sk) = kx.keypair(KeyGenOption::Random);
        let (receiver_kx, _receiver_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1444".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[9u8; 32]).unwrap();
        let transport_binding = [0x5Au8; iroha_crypto::Hash::LENGTH];

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<KexAlgo, ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            kx_local_pk: sender_kx.clone(),
            kx_remote_pk: receiver_kx.clone(),
            connection: Connection::from_split_with_binding(
                11,
                sender_read,
                sender_write,
                Some(transport_binding),
            ),
            cryptographer: cryptographer.clone(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<KexAlgo, ChaCha20Poly1305> {
            connection: Connection::from_split_with_binding(
                12,
                receiver_read,
                receiver_write,
                Some(transport_binding),
            ),
            expected_peer_id: None,
            kx_local_pk: receiver_kx,
            kx_remote_pk: sender_kx,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let ready = GetKey::read_their_public_key(get_key)
            .await
            .expect("handshake should succeed with matching transport binding");
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert_eq!(ready.connection.transport_binding, Some(transport_binding));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_mismatched_transport_binding() {
        let kx = KexAlgo::new();
        let (sender_kx, _sender_sk) = kx.keypair(KeyGenOption::Random);
        let (receiver_kx, _receiver_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1446".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[10u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<KexAlgo, ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            kx_local_pk: sender_kx.clone(),
            kx_remote_pk: receiver_kx.clone(),
            connection: Connection::from_split_with_binding(
                13,
                sender_read,
                sender_write,
                Some([0x11u8; iroha_crypto::Hash::LENGTH]),
            ),
            cryptographer: cryptographer.clone(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<KexAlgo, ChaCha20Poly1305> {
            connection: Connection::from_split_with_binding(
                14,
                receiver_read,
                receiver_write,
                Some([0x22u8; iroha_crypto::Hash::LENGTH]),
            ),
            expected_peer_id: None,
            kx_local_pk: receiver_kx,
            kx_remote_pk: sender_kx,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("mismatched transport binding must be rejected"),
            Err(err) => err,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert!(
            matches!(err, crate::Error::Keys(_)),
            "expected signature verification failure, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outgoing_handshake_rejects_unexpected_peer_identity() {
        let kx = KexAlgo::new();
        let (sender_kx, _sender_sk) = kx.keypair(KeyGenOption::Random);
        let (receiver_kx, _receiver_sk) = kx.keypair(KeyGenOption::Random);
        let addr: SocketAddr = "127.0.0.1:1445".parse().unwrap();
        let actual_key_pair = KeyPair::random();
        let expected_peer_id =
            iroha_data_model::prelude::PeerId::from(KeyPair::random().public_key().clone());
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[8u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<KexAlgo, ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr.clone(),
            expected_peer_id: None,
            key_pair: actual_key_pair,
            kx_local_pk: sender_kx.clone(),
            kx_remote_pk: receiver_kx.clone(),
            connection: Connection::from_split(3, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<KexAlgo, ChaCha20Poly1305> {
            connection: Connection::from_split(4, receiver_read, receiver_write),
            expected_peer_id: Some(expected_peer_id.clone()),
            kx_local_pk: receiver_kx,
            kx_remote_pk: sender_kx,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("unexpected peer identity must be rejected"),
            Err(err) => err,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        match err {
            crate::Error::HandshakePeerMismatch { expected, found } => {
                assert_eq!(expected, expected_peer_id);
                assert_ne!(expected, found);
            }
            other => panic!("expected HandshakePeerMismatch, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pre_handshake_header_rejects_garbage() {
        // Build a duplex to simulate a remote sending garbage preface
        let (a, mut b) = tokio::io::duplex(64);
        // Writer side: send wrong 5 bytes then close
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = b.write_all(b"BAD!!").await;
        });

        // ConnectedFrom will attempt to read the preface and should error out
        let key_pair = iroha_crypto::KeyPair::random();
        let our_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let (r, w) = tokio::io::split(a);
        let conn = Connection::from_split(1, r, w);
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        let cf = ConnectedFrom {
            our_public_address: our_addr,
            key_pair,
            connection: conn,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            soranet_handshake: soranet,
            local_scion_supported: true,
            trust_gossip: true,
            relay_role: RelayRole::Disabled,
        };
        let err = ConnectedFrom::read_client_hello::<
            KexAlgo,
            iroha_crypto::encryption::ChaCha20Poly1305,
        >(cf)
        .await
        .err()
        .expect("expected error on bad preface");
        let _ = err; // just ensure it errs
    }

    #[cfg(feature = "noise_handshake")]
    #[tokio::test(flavor = "current_thread")]
    async fn noise_handshake_roundtrip_keys_match() {
        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (mut a_read, mut a_write) = tokio::io::split(stream_a);
        let (mut b_read, mut b_write) = tokio::io::split(stream_b);

        let (init_res, resp_res) = tokio::join!(
            super::noise_handshake_initiator(&mut a_read, &mut a_write),
            super::noise_handshake_responder(&mut b_read, &mut b_write),
        );

        let init_key = init_res.expect("initiator handshake");
        let resp_key = resp_res.expect("responder handshake");
        assert_eq!(init_key, resp_key, "handshake keys must match");
        assert_eq!(init_key.len(), 32, "handshake key must be 32 bytes");
    }
}

// handshake payload is encoded/decoded as a tuple to avoid extra type definitions

mod handshake_flow {
    //! Implementations of the handshake process.

    use async_trait::async_trait;

    use super::{state::*, *};

    #[async_trait]
    pub(super) trait Stage<K: Kex, E: Enc> {
        type NextStage;

        async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error>;
    }

    macro_rules! stage {
        ( $func:ident : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func(self).await ; $curstage => $nextstage);
        };
        ( $func:ident :: <$($generic_param:ident),+> : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func::<$($generic_param),+>(self).await ; $curstage => $nextstage);
        };
        // Internal case
        (@base $self:ident $call:expr ; $curstage:ty => $nextstage:ty ) => {
            #[async_trait]
            impl<K: Kex, E: Enc> Stage<K, E> for $curstage {
                type NextStage = $nextstage;

                async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error> {
                    // NOTE: Need this due to macro hygiene
                    let $self = self;
                    $call
                }
            }
        }
    }

    stage!(connect_to: Connecting => ConnectedTo);
    stage!(send_client_hello::<K, E>: ConnectedTo => SendKey<K, E>);
    stage!(read_client_hello::<K, E>: ConnectedFrom => SendKey<K, E>);
    stage!(send_our_public_key: SendKey<K, E> => GetKey<K, E>);
    stage!(read_their_public_key: GetKey<K, E> => Ready<E>);

    #[async_trait]
    pub(super) trait Handshake<K: Kex, E: Enc> {
        async fn handshake(self) -> Result<Ready<E>, crate::Error>;
    }

    macro_rules! impl_handshake {
        ( base_case $typ:ty ) => {
            // Base case, should be all states that lead to `Ready`
            #[async_trait]
            impl<K: Kex, E: Enc> Handshake<K, E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    <$typ as Stage<K, E>>::advance_to_next_stage(self).await
                }
            }
        };
        ( $typ:ty ) => {
            #[async_trait]
            impl<K: Kex, E: Enc> Handshake<K, E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    let next_stage = <$typ as Stage<K, E>>::advance_to_next_stage(self).await?;
                    <_ as Handshake<K, E>>::handshake(next_stage).await
                }
            }
        };
    }

    impl_handshake!(base_case GetKey<K, E>);
    impl_handshake!(SendKey<K, E>);
    impl_handshake!(ConnectedFrom);
    impl_handshake!(ConnectedTo);
    impl_handshake!(Connecting);
}

pub(crate) use run::data_message_wire_len;

pub mod message {
    //! Module for peer messages

    use iroha_data_model::peer::Peer;

    use super::*;

    /// Connection and Handshake was successful
    pub struct Connected<T: Pload> {
        /// Peer
        pub peer: Peer,
        /// Connection Id
        pub connection_id: ConnectionId,
        /// Handle for peer to send messages and terminate command
        pub ready_peer_handle: handles::PeerHandle<T>,
        /// Channel to send peer messages channel
        pub peer_message_sender: oneshot::Sender<PeerMessageSenders<T>>,
        /// Disambiguator of connection (equal for both peers)
        pub disambiguator: u64,
        /// Relay role advertised during handshake.
        pub relay_role: RelayRole,
        /// Whether the remote supports SCION transport preference.
        pub scion_supported: bool,
        /// Whether the remote supports trust gossip.
        pub trust_gossip: bool,
    }

    /// High/low priority senders for inbound peer messages.
    pub struct PeerMessageSenders<T: Pload> {
        /// Sender for high-priority inbound peer messages.
        pub high: mpsc::Sender<PeerMessage<T>>,
        /// Sender for low-priority inbound peer messages.
        pub low: mpsc::Sender<PeerMessage<T>>,
    }

    /// Messages received from Peer along with their encoded size (in bytes).
    #[derive(Clone)]
    pub struct PeerMessage<T: Pload> {
        /// Remote peer that delivered this payload.
        pub peer: Peer,
        /// Fully decoded payload content.
        pub payload: T,
        /// Size of the payload on the wire (Norito-encoded) in bytes.
        pub payload_bytes: usize,
    }

    /// Peer faced error or `Terminate` message, send to indicate that it is terminated
    pub struct Terminated {
        /// Peer
        pub peer: Option<Peer>,
        /// Connection Id
        pub conn_id: ConnectionId,
    }

    /// Messages sent by peer during connection process
    pub enum ServiceMessage<T: Pload> {
        /// Connection and Handshake was successful
        Connected(Connected<T>),
        /// Peer faced error or `Terminate` message, send to indicate that it is terminated
        Terminated(Terminated),
        /// A newly accepted incoming connection is pending handshake (used by
        /// alternative listeners to register a connection id prior to `Connected`).
        ///
        /// NOTE: This allows the network to account for incoming caps while the
        /// handshake is in progress for transports accepted outside the TCP
        /// listener loop (e.g., QUIC).
        InboundPending(ConnectionId),
        /// Ask the network actor if an inbound connection should be accepted,
        /// applying caps and per‑IP throttle identically to TCP accepts.
        /// If accepted, the network actor should insert the `conn_id` into
        /// `incoming_pending` and reply `true`.
        InboundAsk {
            /// Temporary connection id
            conn_id: ConnectionId,
            /// Remote socket address reported by transport
            remote_addr: std::net::SocketAddr,
            /// Reply whether to accept (true) or drop (false)
            reply: tokio::sync::oneshot::Sender<bool>,
        },
        /// Provide an externally accepted inbound stream (e.g., via Torii `/p2p`).
        /// The network actor will spawn a peer in `ConnectedFrom` state.
        InboundStream {
            /// Connection id allocated by the caller (should be unique).
            conn_id: ConnectionId,
            /// Reader half of the stream.
            read: Box<dyn AsyncRead + Send + Unpin>,
            /// Writer half of the stream.
            write: Box<dyn AsyncWrite + Send + Unpin>,
        },
    }
}

mod cryptographer {
    use iroha_crypto::{SessionKey, encryption::SymmetricEncryptor};

    use super::*;
    use crate::blake2b_hash;

    /// Peer's cryptographic primitives
    #[derive(Clone)]
    pub struct Cryptographer<E: Enc> {
        /// Blake2b hash of the session key, used as unique shared value between two peers
        pub disambiguator: u64,
        /// Encryptor created from session key, that we got by Diffie-Hellman scheme
        pub encryptor: SymmetricEncryptor<E>,
    }

    impl<E: Enc> Cryptographer<E> {
        /// Construct from raw key bytes (e.g., derived via Noise)
        #[cfg(any(feature = "noise_handshake", test))]
        pub fn new_with_raw_key_bytes(key_bytes: &[u8]) -> Result<Self, Error> {
            let disambiguator = blake2b_hash(key_bytes);
            let encryptor = SymmetricEncryptor::<E>::new_with_key(key_bytes)?;
            Ok(Self {
                disambiguator,
                encryptor,
            })
        }
        /// Decrypt bytes.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy`] error
        pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            self.encryptor
                .decrypt_easy(DEFAULT_AAD.as_ref(), data)
                .map_err(Into::into)
        }

        /// Decrypt bytes into a reusable buffer.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy_into`] error
        pub fn decrypt_into<'a>(
            &self,
            data: &[u8],
            out: &'a mut Vec<u8>,
        ) -> Result<&'a [u8], Error> {
            self.encryptor
                .decrypt_easy_into(DEFAULT_AAD.as_ref(), data, out)
                .map_err(Into::into)
        }

        /// Encrypt bytes.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy`] error
        pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            self.encryptor
                .encrypt_easy(DEFAULT_AAD.as_ref(), data)
                .map_err(Into::into)
        }

        /// Encrypt bytes into a reusable buffer.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::encrypt_easy_into`] error
        pub fn encrypt_into<'a>(
            &self,
            data: &[u8],
            out: &'a mut Vec<u8>,
        ) -> Result<&'a [u8], Error> {
            self.encryptor
                .encrypt_easy_into(DEFAULT_AAD.as_ref(), data, out)
                .map_err(Into::into)
        }

        /// Derives shared key from local private key and remote public key.
        #[cfg_attr(feature = "noise_handshake", allow(dead_code))]
        pub fn new(shared_key: &SessionKey) -> Result<Self, Error> {
            let disambiguator = blake2b_hash(shared_key.payload());

            let encryptor = SymmetricEncryptor::<E>::new_from_session_key(shared_key)?;
            Ok(Self {
                disambiguator,
                encryptor,
            })
        }
    }
}

/// An identification for peer connections.
pub type ConnectionId = u64;
/// Hash-sized binding for authenticated transport sessions.
pub type TransportBinding = [u8; iroha_crypto::Hash::LENGTH];

/// P2P connection
pub struct Connection {
    /// A unique connection id
    pub id: ConnectionId,
    /// Reader half of the stream
    pub read: Box<dyn AsyncRead + Send + Unpin>,
    /// Writer half of the stream
    pub write: Box<dyn AsyncWrite + Send + Unpin>,
    /// Optional low-priority reader half (e.g., second QUIC stream).
    pub read_low: Option<Box<dyn AsyncRead + Send + Unpin>>,
    /// Optional low-priority writer half (e.g., second QUIC stream).
    pub write_low: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    /// QUIC connection handle (only set when the underlying transport is QUIC).
    pub quic: Option<crate::transport::QuicConnection>,
    /// Remote addr, for logging purpose.
    pub remote_addr: Option<SocketAddr>,
    /// Optional certificate fingerprint for TLS/QUIC channel binding.
    pub transport_binding: Option<TransportBinding>,
}

impl Connection {
    /// Instantiate new connection from `connection_id` and `stream`.
    pub fn new(id: ConnectionId, stream: TcpStream) -> Self {
        let remote_addr = stream.peer_addr().ok();
        let (read_half, write_half) = stream.into_split();
        Connection {
            id,
            read: Box::new(read_half),
            write: Box::new(write_half),
            read_low: None,
            write_low: None,
            quic: None,
            remote_addr,
            transport_binding: None,
        }
    }

    /// Instantiate a connection from arbitrary read/write halves.
    pub fn from_split<R, W>(id: ConnectionId, read: R, write: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Self::from_split_with_binding(id, read, write, None)
    }

    /// Instantiate a connection from arbitrary read/write halves with an optional
    /// transport certificate binding.
    pub fn from_split_with_binding<R, W>(
        id: ConnectionId,
        read: R,
        write: W,
        transport_binding: Option<TransportBinding>,
    ) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Connection {
            id,
            read: Box::new(read),
            write: Box::new(write),
            read_low: None,
            write_low: None,
            quic: None,
            remote_addr: None,
            transport_binding,
        }
    }

    /// Instantiate connection from QUIC streams.
    #[cfg(feature = "quic")]
    pub fn from_quic(
        id: ConnectionId,
        quic: quinn::Connection,
        send_hi: quinn::SendStream,
        recv_hi: quinn::RecvStream,
        send_low: Option<quinn::SendStream>,
        recv_low: Option<quinn::RecvStream>,
        remote_addr: Option<SocketAddr>,
        transport_binding: Option<TransportBinding>,
    ) -> Self {
        Connection {
            id,
            read: Box::new(recv_hi),
            write: Box::new(send_hi),
            read_low: recv_low.map(|s| {
                let boxed: Box<dyn AsyncRead + Send + Unpin> = Box::new(s);
                boxed
            }),
            write_low: send_low.map(|s| {
                let boxed: Box<dyn AsyncWrite + Send + Unpin> = Box::new(s);
                boxed
            }),
            quic: Some(quic),
            remote_addr,
            transport_binding,
        }
    }
}
