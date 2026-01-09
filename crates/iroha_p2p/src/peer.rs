//! Tokio actor Peer

use std::{
    net::SocketAddr,
    panic,
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
use norito::codec::{Decode, DecodeAll, Encode};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
#[cfg(feature = "noise_handshake")]
use snow::{Builder, params::NoiseParams};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::Duration,
};

use crate::{
    ConsensusConfigCaps, ConsensusHandshakeCaps, Error, RelayRole, boilerplate::*,
    sampler::LogSampler,
};
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
            5,
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
        let last = corrupted.len() - 1;
        corrupted[last] ^= 0xFF;
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
    pub(crate) fn connecting<T: Pload, K: Kex, E: Enc>(
        peer_addr: SocketAddr,
        our_public_address: SocketAddr,
        key_pair: KeyPair,
        connection_id: ConnectionId,
        service_message_sender: mpsc::Sender<ServiceMessage<T>>,
        idle_timeout: Duration,
        chain_id: Option<iroha_data_model::ChainId>,
        consensus_caps: Option<crate::ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        soranet_handshake: Arc<SoranetHandshakeConfig>,
        post_capacity: usize,
        quic_enabled: bool,
        tls_enabled: bool,
        prefer_ws_fallback: bool,
        trust_gossip: bool,
        max_frame_bytes: usize,
        relay_role: RelayRole,
        _tcp_nodelay: bool,
        _tcp_keepalive: Option<Duration>,
    ) {
        #[cfg(test)]
        crate::peer::test_support::record(
            crate::peer::test_support::SpawnPath::Connecting,
            max_frame_bytes,
        );
        let peer = state::Connecting {
            peer_addr,
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
            prefer_ws_fallback,
            trust_gossip,
            relay_role,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Start Peer in `state::ConnectedFrom` state
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connected_from<T: Pload, K: Kex, E: Enc>(
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
        post_capacity: usize,
        relay_role: RelayRole,
        trust_gossip: bool,
        max_frame_bytes: usize,
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
            trust_gossip,
            relay_role,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Per-topic senders for peer substreams.
    pub(super) struct TopicSenders<T> {
        pub(super) hi_consensus: post_channel::Sender<T>,
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
}

mod run {
    //! Module with peer [`run`] function.

    use std::task::Poll;

    use futures::future::poll_fn;
    use iroha_logger::prelude::*;
    use norito::codec::Decode;
    use tokio::time::Instant;
    use tracing;

    use super::{
        cryptographer::Cryptographer,
        handshake_flow::Handshake,
        state::{ConnectedFrom, Connecting, Ready},
        *,
    };

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

    fn low_topic_label(topic: LowTopic) -> &'static str {
        match topic {
            LowTopic::BlockSync => "low:block_sync",
            LowTopic::TxGossip => "low:tx_gossip",
            LowTopic::PeerGossip => "low:peer_gossip",
            LowTopic::Health => "low:health",
            LowTopic::Other => "low:other",
        }
    }

    fn bump_low_rr(low_rr: &mut u8, served_idx: usize) {
        *low_rr = u8::try_from((served_idx + 1) % LOW_TOPIC_COUNT)
            .expect("LOW_TOPIC_COUNT must fit in u8");
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
    pub(super) async fn run<T: Pload, K: Kex, E: Enc, P: Entrypoint<K, E>>(
        RunPeerArgs {
            peer,
            service_message_sender,
            idle_timeout,
            post_capacity,
            max_frame_bytes,
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
                        id: connection_id,
                        ..
                    },
                cryptographer,
                relay_role,
                trust_gossip,
            } = ready_peer;
            let peer_id = peer_id.insert(new_peer_id);

            let disambiguator = cryptographer.disambiguator;

            tracing::Span::current().record("peer", peer_id.to_string());
            tracing::Span::current().record("disambiguator", disambiguator);

            // Create per-topic substreams (bounded or unbounded depending on feature).
            let (hi_consensus_tx, mut hi_consensus_rx) = post_channel::channel(post_capacity);
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
            let Ok(peer_message_sender) = peer_message_receiver.await else {
                // NOTE: this is not considered as error, because network might decide not to connect peer.
                iroha_logger::debug!(
                    "Network decide not to connect peer."
                );
                return;
            };

            iroha_logger::trace!("Peer connected");

            let mut message_reader = MessageReader::new(read, cryptographer.clone(), max_frame_bytes);
            // Sampler for repeated read/parse errors to avoid log floods from malformed peers
            let mut read_err_sampler = LogSampler::new();
            let mut message_sender = MessageSender::new(write, cryptographer, max_frame_bytes);

            let mut idle_interval = tokio::time::interval_at(Instant::now() + idle_timeout, idle_timeout);
            let mut ping_interval = tokio::time::interval_at(Instant::now() + idle_timeout / 2, idle_timeout / 2);

            // Fairness scheduler: opportunistically service one low-priority topic
            // after processing a burst of high-priority posts. This avoids starving
            // low topics during sustained consensus traffic.
            let mut hi_budget: u8 = HI_BUDGET_RESET;
            let mut low_rr: u8 = 0;

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
                    if let Err(error) = message_sender.prepare_message(&Message::Data(msg)) {
                        iroha_logger::error!(%error, "Failed to encrypt message.");
                        break;
                    }
                    continue;
                }

                tokio::select! {
                    // High-priority topics first (budgeted to avoid starvation).
                    _ = ping_interval.tick() => {
                        iroha_logger::trace!(
                            ping_period=?ping_interval.period(),
                            "The connection has been idle, pinging to check if it's alive"
                        );
                        if let Err(error) = message_sender.prepare_message(&Message::<T>::Ping) {
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
                    msg = hi_consensus_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg { iroha_logger::trace!("Post message (hi:consensus)"); if let Err(error) = message_sender.prepare_message(&Message::Data(m)) { iroha_logger::error!(%error, "Failed to encrypt message."); break; } hi_budget = hi_budget.saturating_sub(1); }
                    }
                    msg = hi_control_rx.recv(), if hi_budget > 0 => {
                        if let Some(m) = msg { iroha_logger::trace!("Post message (hi:control)"); if let Err(error) = message_sender.prepare_message(&Message::Data(m)) { iroha_logger::error!(%error, "Failed to encrypt message."); break; } hi_budget = hi_budget.saturating_sub(1); }
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
                            if let Err(error) = message_sender.prepare_message(&Message::Data(msg)) { iroha_logger::error!(%error, "Failed to encrypt message."); break; }
                            hi_budget = HI_BUDGET_RESET;
                        }
                    }
                    msg = message_reader.read_message() => {
                        let (message, encoded_len) = match msg {
                            Ok(Some((msg, encoded_len))) => (msg, encoded_len),
                            Ok(None) => {
                                iroha_logger::debug!("Peer send whole message and close connection");
                                break;
                            }
                            Err(error) => {
                                if let Some(supp) = read_err_sampler.should_log(tokio::time::Duration::from_millis(500)) {
                                    iroha_logger::error!(?error, suppressed=supp, "Error while reading message from peer.");
                                }
                                break;
                            }
                        };
                        match message {
                            Message::Ping => {
                                iroha_logger::trace!("Received peer ping");
                                if let Err(error) = message_sender.prepare_message(&Message::<T>::Pong) {
                                    iroha_logger::error!(%error, "Failed to encrypt message.");
                                    break;
                                }
                            },
                            Message::Pong => {
                                iroha_logger::trace!("Received peer pong");
                            }
                            Message::Data(payload) => {
                                iroha_logger::trace!("Received peer message");
                                let peer_message = PeerMessage {
                                    peer: peer_id.clone(),
                                    payload,
                                    payload_bytes: encoded_len,
                                };
                                if peer_message_sender.send(peer_message).await.is_err() {
                                    iroha_logger::error!("Network dropped peer message channel.");
                                    break;
                                }
                            }
                        }
                        // Reset idle and ping timeout as peer received message from another peer
                        idle_interval.reset();
                        ping_interval.reset();
                    }
                    // `message_sender.send()` is safe to be cancelled, it won't advance the queue or write anything if another branch completes first.
                    //
                    // We need to conditionally disable it in case there is no data is to be sent, otherwise `message_sender.send()` will complete immediately
                    //
                    // The only source of data to be sent is other branches of this loop, so we do not need any async waiting mechanism for waiting for readiness.
                    result = message_sender.send(), if message_sender.ready() => {
                        if let Err(error) = result {
                            iroha_logger::error!(%error, "Failed to send message to peer.");
                            break;
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
                        if let Err(error) = message_sender.prepare_message(&Message::Data(m)) {
                            iroha_logger::error!(%error, "Failed to encrypt message.");
                            break;
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

    /// Cancellation-safe way to read messages from tcp stream
    struct MessageReader<E: Enc> {
        read: Box<dyn AsyncRead + Send + Unpin>,
        buffer: bytes::BytesMut,
        cryptographer: Cryptographer<E>,
        max_frame_bytes: usize,
    }

    impl<E: Enc> MessageReader<E> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();

        fn new(
            read: Box<dyn AsyncRead + Send + Unpin>,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
        ) -> Self {
            Self {
                read,
                cryptographer,
                buffer: BytesMut::with_capacity(DEFAULT_BUFFER_CAPACITY),
                max_frame_bytes,
            }
        }

        /// Read message by first reading it's size as u32 and then rest of the message
        ///
        /// # Errors
        /// - Fail in case reading from stream fails
        /// - Connection is closed by there is still unfinished message in buffer
        /// - Forward errors from [`Self::parse_message`]
        async fn read_message<T: Pload>(&mut self) -> Result<Option<(T, usize)>, Error> {
            loop {
                // Try to get full message
                if let Some(msg) = self.parse_message()? {
                    return Ok(Some(msg));
                }

                if 0 == self.read.read_buf(&mut self.buffer).await? {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    return Err(Error::ConnectionResetByPeer);
                }
            }
        }

        /// Parse message
        ///
        /// # Errors
        /// - Fail to decrypt message
        /// - Fail to decode message
        fn parse_message<T: Pload>(&mut self) -> Result<Option<(T, usize)>, Error> {
            let mut buf = &self.buffer[..];
            if buf.remaining() < Self::U32_SIZE {
                // Not enough data to read u32
                return Ok(None);
            }
            let size = buf.get_u32() as usize;
            if size > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            if buf.remaining() < size {
                // Not enough data to read the whole data
                return Ok(None);
            }

            let data = &buf[..size];
            let decrypted = self.cryptographer.decrypt(data)?;
            let encoded_len = decrypted.len();
            let decoded =
                match panic::catch_unwind(|| DecodeAll::decode_all(&mut decrypted.as_slice())) {
                    Ok(result) => result?,
                    Err(panic) => {
                        iroha_logger::warn!(?panic, "Norito decode panicked; dropping connection");
                        return Err(Error::Format);
                    }
                };

            self.buffer.advance(size + Self::U32_SIZE);

            Ok(Some((decoded, encoded_len)))
        }
    }

    struct MessageSender<E: Enc> {
        write: Box<dyn AsyncWrite + Send + Unpin>,
        cryptographer: Cryptographer<E>,
        /// Reusable buffer to encode messages
        buffer: Vec<u8>,
        /// Queue of encrypted messages waiting to be sent
        queue: BytesMut,
        /// Maximum payload size accepted per encrypted frame
        max_frame_bytes: usize,
    }

    impl<E: Enc> MessageSender<E> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();

        fn new(
            write: Box<dyn AsyncWrite + Send + Unpin>,
            cryptographer: Cryptographer<E>,
            max_frame_bytes: usize,
        ) -> Self {
            Self {
                write,
                cryptographer,
                buffer: Vec::with_capacity(DEFAULT_BUFFER_CAPACITY),
                queue: BytesMut::with_capacity(DEFAULT_BUFFER_CAPACITY),
                max_frame_bytes,
            }
        }

        /// Prepare message for the delivery and put it into the queue to be sent later
        ///
        /// # Errors
        /// - If encryption fail.
        fn prepare_message<T: Pload>(&mut self, msg: &T) -> Result<(), Error> {
            // Start with fresh buffer
            self.buffer.clear();
            msg.encode_to(&mut self.buffer);
            let encrypted = self.cryptographer.encrypt(&self.buffer)?;

            let size = encrypted.len();
            if size > self.max_frame_bytes {
                return Err(Error::FrameTooLarge);
            }
            self.queue.reserve(size + Self::U32_SIZE);
            #[allow(clippy::cast_possible_truncation)]
            self.queue.put_u32(size as u32);
            self.queue.put_slice(encrypted.as_slice());
            Ok(())
        }

        /// Send bytes of byte-encoded messages piled up in the message queue so far.
        /// On the other side peer will collect bytes and recreate original messages from them.
        ///
        /// Sends only as much data as the underlying writer will accept in one `.write` call,
        /// so must be called in a loop to ensure everything will get sent.
        ///
        /// # Errors
        /// - If write to `stream` fail.
        async fn send(&mut self) -> Result<(), Error> {
            let chunk = self.queue.chunk();
            if !chunk.is_empty() {
                let n = self.write.write(chunk).await?;
                self.queue.advance(n);
                self.write.flush().await?;
            }
            Ok(())
        }

        /// Check if message sender has data ready to be sent.
        fn ready(&self) -> bool {
            !self.queue.is_empty()
        }
    }

    /// Either message or ping
    #[derive(Encode, Decode, Clone, Debug)]
    enum Message<T> {
        Data(T),
        Ping,
        Pong,
    }

    pub(crate) fn data_message_wire_len<T: Encode>(payload: T) -> usize {
        Message::Data(payload).encode().len()
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

        use super::*;

        #[derive(Encode, Decode, Clone, Debug)]
        struct Dummy;

        #[derive(Encode, Decode, Clone, Debug)]
        struct Blob(Vec<u8>);

        #[derive(Default)]
        struct WriteStats {
            writes: usize,
            flushes: usize,
        }

        struct TrackingWrite {
            stats: Arc<Mutex<WriteStats>>,
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
                .prepare_message(&Message::Data(Dummy))
                .expect("prepare message");
            assert!(sender.ready(), "message sender should have queued data");
            sender.send().await.expect("send");
            assert!(!sender.ready(), "queue should be drained");

            let stats = stats.lock().expect("stats lock");
            assert!(stats.writes > 0, "expected at least one write");
            assert!(stats.flushes > 0, "expected at least one flush");
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
        async fn malformed_frame_flood_is_sampled() {
            // Build a buffer with many bogus frames: [len=16][16 zero bytes] * N
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
            // Any key works; decrypt will fail on random zeros
            let crypt =
                super::cryptographer::Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(
                    &[1u8; 32],
                )
                .expect("valid key length");
            let mut mr = MessageReader::new(read, crypt, 1024);

            // First attempt should yield an error due to malformed/cannot decrypt frame
            let err = mr.read_message::<Dummy>().await.err();
            assert!(err.is_some(), "expected read error from malformed frame");

            // Now simulate a flood of such errors and ensure our sampler would emit at most once
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
            let mut mr = MessageReader::new(read, crypt, 1024); // max_frame_bytes=1024
            let err = mr.read_message::<Dummy>().await.err();
            assert!(matches!(err, Some(Error::FrameTooLarge)));
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
            let err = sender.prepare_message(&payload).unwrap_err();
            assert!(matches!(err, Error::FrameTooLarge));
            assert!(!sender.ready(), "rejected frame should not queue data");
        }

        #[test]
        fn message_sender_allows_within_cap() {
            let mut sender = make_sender(512);
            let small = Blob(vec![0u8; 8]);
            sender
                .prepare_message(&small)
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

    fn build_trust_meta(trust_gossip: bool) -> HandshakeTrustMeta {
        HandshakeTrustMeta { trust_gossip }
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub(super) struct HandshakeTrustMeta {
        pub(super) trust_gossip: bool,
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
    ) -> Vec<u8> {
        let _ = (local_pk, remote_pk);
        let mut data = Vec::new();
        data.extend_from_slice(&cryptographer.disambiguator.to_be_bytes());
        data.extend_from_slice(&advertised_addr.encode());
        if let Some(cid) = chain_id {
            data.extend_from_slice(&cid.encode());
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
        pub prefer_ws_fallback: bool,
        pub trust_gossip: bool,
        pub relay_role: RelayRole,
    }

    impl Connecting {
        #[allow(unused_variables, clippy::too_many_lines, clippy::single_match_else)]
        pub(super) async fn connect_to(
            Self {
                peer_addr,
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
                prefer_ws_fallback,
                trust_gossip,
                relay_role,
            }: Self,
        ) -> Result<ConnectedTo, crate::Error> {
            let connection = match &peer_addr {
                iroha_primitives::addr::SocketAddr::Host(host) => {
                    #[cfg(feature = "p2p_ws")]
                    if prefer_ws_fallback {
                        let endpoint = format!("{}:{}", host.host.as_ref(), host.port);
                        if let Ok(ws) = crate::transport::ws::connect_wss(&endpoint)
                            .await
                            .or_else(|_| async {
                                crate::transport::ws::connect_ws(&endpoint).await
                            })
                            .await
                        {
                            crate::network::inc_ws_outbound();
                            let (r, w) = tokio::io::split(ws);
                            return Ok(ConnectedTo {
                                connection: Connection::from_split(connection_id, r, w),
                                our_public_address,
                                key_pair,
                                chain_id,
                                consensus_caps,
                                confidential_caps,
                                crypto_caps,
                                soranet_handshake,
                                trust_gossip,
                                relay_role,
                                trust_gossip,
                            });
                        }
                    }
                    #[cfg(feature = "quic")]
                    if quic_enabled {
                        let server_name = host.host.as_ref();
                        let addr = format!("{}:{}", server_name, host.port);
                        match crate::transport::quic::connect_and_open_bi(server_name, &addr).await
                        {
                            Ok((_conn, send, recv)) => {
                                Connection::from_quic(connection_id, send, recv)
                            }
                            Err(e) => {
                                iroha_logger::warn!(%e, "QUIC connect failed; falling back to TCP");
                                match crate::transport::connect(&peer_addr).await {
                                    Ok(stream) => Connection::new(connection_id, stream),
                                    Err(err) => {
                                        crate::network::inc_dns_resolution_fail();
                                        return Err(err.into());
                                    }
                                }
                            }
                        }
                    } else {
                        #[cfg(feature = "p2p_tls")]
                        {
                            if tls_enabled {
                                let endpoint = format!("{}:{}", host.host.as_ref(), host.port);
                                match crate::transport::tls::connect_tls(&endpoint).await {
                                    Ok(tls) => {
                                        let (read_half, write_half) = tokio::io::split(tls);
                                        Connection::from_split(connection_id, read_half, write_half)
                                    }
                                    Err(e) => {
                                        iroha_logger::warn!(%e, endpoint=%endpoint, "TLS connect failed; trying TCP then WSS (if enabled)");
                                        match crate::transport::connect(&peer_addr).await {
                                            Ok(stream) => Connection::new(connection_id, stream),
                                            Err(tcp_err) => {
                                                #[cfg(feature = "p2p_ws")]
                                                {
                                                    match crate::transport::ws::connect_wss(
                                                        &endpoint,
                                                    )
                                                    .await
                                                    {
                                                        Ok(ws) => {
                                                            let (r, w) = tokio::io::split(ws);
                                                            Connection::from_split(
                                                                connection_id,
                                                                r,
                                                                w,
                                                            )
                                                        }
                                                        Err(ws_err) => {
                                                            static CONNECT_ERR_SAMPLER:
                                                                std::sync::OnceLock<
                                                                    std::sync::Mutex<LogSampler>,
                                                                > = std::sync::OnceLock::new();
                                                            let sampler = CONNECT_ERR_SAMPLER
                                                                .get_or_init(|| {
                                                                    std::sync::Mutex::new(
                                                                        LogSampler::new(),
                                                                    )
                                                                });
                                                            if let Ok(mut s) = sampler.lock() {
                                                                if let Some(supp) = s.should_log(tokio::time::Duration::from_millis(500)) {
                                                                    iroha_logger::warn!(tcp_err=%tcp_err, ws_err=%ws_err, addr=%peer_addr, suppressed=supp, "TCP and WSS fallbacks failed");
                                                                }
                                                            }
                                                            crate::network::inc_dns_resolution_fail(
                                                            );
                                                            return Err(tcp_err.into());
                                                        }
                                                    }
                                                }
                                                #[cfg(not(feature = "p2p_ws"))]
                                                {
                                                    static CONNECT_ERR_SAMPLER:
                                                        std::sync::OnceLock<
                                                            std::sync::Mutex<LogSampler>,
                                                        > = std::sync::OnceLock::new();
                                                    let sampler =
                                                        CONNECT_ERR_SAMPLER.get_or_init(|| {
                                                            std::sync::Mutex::new(LogSampler::new())
                                                        });
                                                    if let Ok(mut s) = sampler.lock() {
                                                        if let Some(supp) = s.should_log(
                                                            tokio::time::Duration::from_millis(500),
                                                        ) {
                                                            iroha_logger::warn!(%tcp_err, addr=%peer_addr, suppressed=supp, "TCP fallback failed");
                                                        }
                                                    }
                                                    crate::network::inc_dns_resolution_fail();
                                                    return Err(tcp_err.into());
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                match crate::transport::connect(&peer_addr).await {
                                    Ok(stream) => Connection::new(connection_id, stream),
                                    Err(err) => {
                                        static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                            std::sync::Mutex<LogSampler>,
                                        > = std::sync::OnceLock::new();
                                        let sampler = CONNECT_ERR_SAMPLER.get_or_init(|| {
                                            std::sync::Mutex::new(LogSampler::new())
                                        });
                                        if let Ok(mut s) = sampler.lock() {
                                            if let Some(supp) = s
                                                .should_log(tokio::time::Duration::from_millis(500))
                                            {
                                                iroha_logger::warn!(%err, addr=%peer_addr, suppressed=supp, "TCP connect failed");
                                            }
                                        }
                                        crate::network::inc_dns_resolution_fail();
                                        return Err(err.into());
                                    }
                                }
                            }
                        }
                        #[cfg(not(feature = "p2p_tls"))]
                        {
                            match crate::transport::connect(&peer_addr).await {
                                Ok(stream) => Connection::new(connection_id, stream),
                                Err(err) => {
                                    #[cfg(feature = "p2p_ws")]
                                    {
                                        let endpoint =
                                            format!("{}:{}", host.host.as_ref(), host.port);
                                        match crate::transport::ws::connect_wss(&endpoint)
                                            .await
                                            .or_else(|_| async {
                                                crate::transport::ws::connect_ws(&endpoint).await
                                            })
                                            .await
                                        {
                                            Ok(ws) => {
                                                let (r, w) = tokio::io::split(ws);
                                                Connection::from_split(connection_id, r, w)
                                            }
                                            Err(ws_err) => {
                                                static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                                    std::sync::Mutex<LogSampler>,
                                                > = std::sync::OnceLock::new();
                                                let sampler =
                                                    CONNECT_ERR_SAMPLER.get_or_init(|| {
                                                        std::sync::Mutex::new(LogSampler::new())
                                                    });
                                                if let Ok(mut s) = sampler.lock() {
                                                    if let Some(supp) = s.should_log(
                                                        tokio::time::Duration::from_millis(500),
                                                    ) {
                                                        iroha_logger::warn!(%err, ws_err=%ws_err, addr=%peer_addr, suppressed=supp, "TCP and WS fallbacks failed");
                                                    }
                                                }
                                                crate::network::inc_dns_resolution_fail();
                                                return Err(err.into());
                                            }
                                        }
                                    }
                                    #[cfg(not(feature = "p2p_ws"))]
                                    {
                                        static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                            std::sync::Mutex<LogSampler>,
                                        > = std::sync::OnceLock::new();
                                        let sampler = CONNECT_ERR_SAMPLER.get_or_init(|| {
                                            std::sync::Mutex::new(LogSampler::new())
                                        });
                                        if let Ok(mut s) = sampler.lock() {
                                            if let Some(supp) = s
                                                .should_log(tokio::time::Duration::from_millis(500))
                                            {
                                                iroha_logger::warn!(%err, addr=%peer_addr, suppressed=supp, "TCP connect failed");
                                            }
                                        }
                                        crate::network::inc_dns_resolution_fail();
                                        return Err(err.into());
                                    }
                                }
                            }
                        }
                    }
                    #[cfg(not(feature = "quic"))]
                    {
                        #[cfg(feature = "p2p_tls")]
                        {
                            if tls_enabled {
                                let endpoint = format!("{}:{}", host.host.as_ref(), host.port);
                                match crate::transport::tls::connect_tls(&endpoint).await {
                                    Ok(tls) => {
                                        let (read_half, write_half) = tokio::io::split(tls);
                                        Connection::from_split(connection_id, read_half, write_half)
                                    }
                                    Err(e) => {
                                        iroha_logger::warn!(%e, endpoint=%endpoint, "TLS connect failed; falling back to TCP");
                                        match crate::transport::connect(&peer_addr).await {
                                            Ok(stream) => Connection::new(connection_id, stream),
                                            Err(err) => {
                                                static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                                    std::sync::Mutex<LogSampler>,
                                                > = std::sync::OnceLock::new();
                                                let sampler =
                                                    CONNECT_ERR_SAMPLER.get_or_init(|| {
                                                        std::sync::Mutex::new(LogSampler::new())
                                                    });
                                                if let Ok(mut s) = sampler.lock() {
                                                    if let Some(supp) = s.should_log(
                                                        tokio::time::Duration::from_millis(500),
                                                    ) {
                                                        iroha_logger::warn!(%err, addr=%peer_addr, suppressed=supp, "TCP connect failed");
                                                    }
                                                }
                                                crate::network::inc_dns_resolution_fail();
                                                return Err(err.into());
                                            }
                                        }
                                    }
                                }
                            } else {
                                match crate::transport::connect(&peer_addr).await {
                                    Ok(stream) => Connection::new(connection_id, stream),
                                    Err(err) => {
                                        static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                            std::sync::Mutex<LogSampler>,
                                        > = std::sync::OnceLock::new();
                                        let sampler = CONNECT_ERR_SAMPLER.get_or_init(|| {
                                            std::sync::Mutex::new(LogSampler::new())
                                        });
                                        if let Ok(mut s) = sampler.lock() {
                                            if let Some(supp) = s
                                                .should_log(tokio::time::Duration::from_millis(500))
                                            {
                                                iroha_logger::warn!(%err, addr=%peer_addr, suppressed=supp, "TCP connect failed");
                                            }
                                        }
                                        crate::network::inc_dns_resolution_fail();
                                        return Err(err.into());
                                    }
                                }
                            }
                        }
                        #[cfg(not(feature = "p2p_tls"))]
                        {
                            match crate::transport::connect(&peer_addr).await {
                                Ok(stream) => Connection::new(connection_id, stream),
                                Err(err) => {
                                    static CONNECT_ERR_SAMPLER: std::sync::OnceLock<
                                        std::sync::Mutex<LogSampler>,
                                    > = std::sync::OnceLock::new();
                                    let sampler = CONNECT_ERR_SAMPLER
                                        .get_or_init(|| std::sync::Mutex::new(LogSampler::new()));
                                    if let Ok(mut s) = sampler.lock() {
                                        if let Some(supp) =
                                            s.should_log(tokio::time::Duration::from_millis(500))
                                        {
                                            iroha_logger::warn!(%err, addr=%peer_addr, suppressed=supp, "TCP connect failed");
                                        }
                                    }
                                    crate::network::inc_dns_resolution_fail();
                                    return Err(err.into());
                                }
                            }
                        }
                    }
                }
                _ => {
                    let stream = crate::transport::connect(&peer_addr).await?;
                    let _ = stream.set_nodelay(true);
                    Connection::new(connection_id, stream)
                }
            };
            Ok(ConnectedTo {
                our_public_address,
                key_pair,
                connection,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
                trust_gossip,
                relay_role,
            })
        }
    }

    /// Peer that is being connected to.
    pub(super) struct ConnectedTo {
        our_public_address: SocketAddr,
        key_pair: KeyPair,
        connection: Connection,
        chain_id: Option<iroha_data_model::ChainId>,
        consensus_caps: Option<ConsensusHandshakeCaps>,
        confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        crypto_caps: Option<crate::CryptoHandshakeCaps>,
        soranet_handshake: Arc<SoranetHandshakeConfig>,
        trust_gossip: bool,
        relay_role: RelayRole,
    }

    impl ConnectedTo {
        #[allow(clippy::similar_names, clippy::too_many_lines)]
        pub(super) async fn send_client_hello<K: Kex, E: Enc>(
            Self {
                our_public_address,
                key_pair,
                mut connection,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                soranet_handshake,
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
                trust_gossip,
            })
        }
    }

    #[cfg(test)]
    pub(super) struct SendKeyInit<K: Kex, E: Enc> {
        pub(super) our_public_address: SocketAddr,
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
        pub(super) trust_gossip: bool,
    }

    /// Peer that needs to send key.
    pub(super) struct SendKey<K: Kex, E: Enc> {
        pub(super) our_public_address: SocketAddr,
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
        pub(super) trust_gossip: bool,
    }

    impl<K: Kex, E: Enc> SendKey<K, E> {
        #[cfg(test)]
        pub(super) fn new(init: SendKeyInit<K, E>) -> Self {
            let SendKeyInit {
                our_public_address,
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
                trust_gossip,
            } = init;
            Self {
                our_public_address,
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
                trust_gossip,
            }
        }

        pub(super) async fn send_our_public_key(
            Self {
                our_public_address,
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
                trust: build_trust_meta(trust_gossip),
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
                kx_local_pk,
                kx_remote_pk,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role,
                trust_gossip,
            })
        }
    }

    /// Peer that needs to get key.
    pub struct GetKey<K: Kex, E: Enc> {
        pub(super) connection: Connection,
        pub(super) kx_local_pk: K::PublicKey,
        pub(super) kx_remote_pk: K::PublicKey,
        pub(super) cryptographer: Cryptographer<E>,
        pub(super) chain_id: Option<iroha_data_model::ChainId>,
        pub(super) consensus_caps: Option<ConsensusHandshakeCaps>,
        pub(super) confidential_caps: Option<crate::ConfidentialHandshakeCaps>,
        pub(super) crypto_caps: Option<crate::CryptoHandshakeCaps>,
        pub(super) relay_role: RelayRole,
        pub(super) trust_gossip: bool,
    }

    impl<K: Kex, E: Enc> GetKey<K, E> {
        /// Read the peer's public key
        pub(super) async fn read_their_public_key(
            Self {
                mut connection,
                kx_local_pk,
                kx_remote_pk,
                cryptographer,
                chain_id,
                consensus_caps,
                confidential_caps,
                crypto_caps,
                relay_role: _relay_role,
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
            );
            signature.verify(&remote_pub_key, &payload)?;

            enforce_consensus_caps(consensus_caps.as_ref(), &consensus)?;
            enforce_confidential_caps(
                confidential_caps.as_ref(),
                &confidential,
                &remote_public_address,
            )?;
            enforce_crypto_caps(crypto_caps.as_ref(), &crypto, &remote_public_address)?;

            let peer = Peer::new(remote_public_address, remote_pub_key);
            let trust_gossip = trust_gossip && trust_gossip_remote;

            Ok(Ready {
                peer,
                connection,
                cryptographer,
                relay_role: relay,
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
                key_pair: key_pair_a,
                connection: Connection::from_split(1, read_a, write_a),
                chain_id: None,
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                soranet_handshake: soranet.clone(),
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
            trust: HandshakeTrustMeta { trust_gossip: true },
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
            trust: HandshakeTrustMeta { trust_gossip: true },
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
            trust_gossip: true,
        });

        let get_key = GetKey::<KexAlgo, ChaCha20Poly1305> {
            connection: Connection::from_split(2, receiver_read, receiver_write),
            kx_local_pk: receiver_kx,
            kx_remote_pk: sender_kx,
            cryptographer,
            chain_id: None,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
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
        pub peer_message_sender: oneshot::Sender<mpsc::Sender<PeerMessage<T>>>,
        /// Disambiguator of connection (equal for both peers)
        pub disambiguator: u64,
        /// Relay role advertised during handshake.
        pub relay_role: RelayRole,
        /// Whether the remote supports trust gossip.
        pub trust_gossip: bool,
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

        /// Encrypt bytes.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy`] error
        pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            self.encryptor
                .encrypt_easy(DEFAULT_AAD.as_ref(), data)
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

/// P2P connection
pub struct Connection {
    /// A unique connection id
    pub id: ConnectionId,
    /// Reader half of the stream
    pub read: Box<dyn AsyncRead + Send + Unpin>,
    /// Writer half of the stream
    pub write: Box<dyn AsyncWrite + Send + Unpin>,
    /// Remote addr, for logging purpose.
    pub remote_addr: Option<SocketAddr>,
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
            remote_addr,
        }
    }

    /// Instantiate a connection from arbitrary read/write halves.
    pub fn from_split<R, W>(id: ConnectionId, read: R, write: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Connection {
            id,
            read: Box::new(read),
            write: Box::new(write),
            remote_addr: None,
        }
    }

    /// Instantiate connection from QUIC streams.
    #[cfg(feature = "quic")]
    pub fn from_quic(id: ConnectionId, send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Connection {
            id,
            read: Box::new(recv),
            write: Box::new(send),
            remote_addr: None,
        }
    }
}
