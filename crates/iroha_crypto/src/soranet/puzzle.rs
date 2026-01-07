//! `Argon2id`-based puzzle helpers for the `SoraNet` admission path.
//!
//! The puzzle format intentionally mirrors the existing hashcash-style `PoW`
//! tickets so clients can attach a single frame regardless of which policy a
//! relay enforces. Difficulty adjustments and TTL validation follow the same
//! rules as the `PoW` implementation, while the work predicate is backed by
//! Argon2id to raise the cost of GPU/ASIC optimisations.

use std::{
    fmt,
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{Algorithm, Argon2, Params, Version};
use rand::{CryptoRng, RngCore};
use thiserror::Error;

use crate::soranet::pow::{CHALLENGE_DOMAIN, SOLUTION_DOMAIN, Ticket};

const OUTPUT_LEN: usize = 32;
const TTL_GRACE: Duration = Duration::from_secs(1);

/// Binding inputs mixed into the puzzle challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeBinding<'a> {
    /// Descriptor commitment advertised by the relay (32 bytes).
    pub descriptor_commit: &'a [u8],
    /// Relay identity bytes (32 bytes).
    pub relay_id: &'a [u8],
    /// Optional transcript hash carried across resumed circuits.
    ///
    /// Present when the client resumes a circuit and both parties agree on the
    /// previously negotiated transcript hash.
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

/// Argon2 puzzle policy parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameters {
    memory_kib: NonZeroU32,
    time_cost: NonZeroU32,
    lanes: NonZeroU32,
    difficulty: u8,
    max_future_skew: Duration,
    min_ticket_ttl: Duration,
}

impl Parameters {
    /// Construct a new parameter set.
    ///
    /// # Panics
    /// Panics when `min_ticket_ttl` is zero or when `max_future_skew` is shorter than the minimum TTL.
    #[must_use]
    pub fn new(
        memory_kib: NonZeroU32,
        time_cost: NonZeroU32,
        lanes: NonZeroU32,
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
    ) -> Self {
        assert!(
            min_ticket_ttl > Duration::ZERO,
            "min_ticket_ttl must be greater than zero"
        );
        assert!(
            max_future_skew >= min_ticket_ttl,
            "max_future_skew must be at least min_ticket_ttl"
        );
        Self {
            memory_kib,
            time_cost,
            lanes,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
        }
    }

    /// Returns the configured memory cost (in KiB).
    #[must_use]
    pub fn memory_kib(&self) -> NonZeroU32 {
        self.memory_kib
    }

    /// Returns the iteration count.
    #[must_use]
    pub fn time_cost(&self) -> NonZeroU32 {
        self.time_cost
    }

    /// Returns the configured parallelism level.
    #[must_use]
    pub fn lanes(&self) -> NonZeroU32 {
        self.lanes
    }

    /// Returns the number of leading zero bits required in the puzzle digest.
    #[must_use]
    pub fn difficulty(&self) -> u8 {
        self.difficulty
    }

    /// Returns the maximum allowed future skew for ticket expiry.
    #[must_use]
    pub fn max_future_skew(&self) -> Duration {
        self.max_future_skew
    }

    /// Returns the minimum ticket lifetime enforced by the policy.
    #[must_use]
    pub fn min_ticket_ttl(&self) -> Duration {
        self.min_ticket_ttl
    }

    /// Clone the parameter set with a different difficulty value.
    #[must_use]
    pub fn with_difficulty(self, difficulty: u8) -> Self {
        Self { difficulty, ..self }
    }
}

/// Errors surfaced while verifying puzzle tickets.
#[derive(Debug, Error)]
pub enum Error {
    /// Ticket uses an unsupported version.
    #[error("unsupported puzzle ticket version {0}")]
    UnsupportedVersion(u8),
    /// Ticket difficulty does not match the required policy.
    #[error("ticket difficulty {ticket} does not match required {required}")]
    DifficultyMismatch {
        /// Difficulty embedded in the ticket metadata.
        ticket: u8,
        /// Difficulty required by the relay policy.
        required: u8,
    },
    /// Ticket expired prior to verification.
    #[error("puzzle ticket expired at {0}, current time {1}")]
    Expired(u64, u64),
    /// Ticket expires too far in the future relative to the relay clock.
    #[error("puzzle ticket expires too far in the future (>{0:?})")]
    FutureSkewExceeded(Duration),
    /// Ticket lifetime is too short for the configured policy.
    #[error("puzzle ticket ttl shorter than required min ({0:?})")]
    ExpiryWindowTooSmall(Duration),
    /// Argon2 parameter set was invalid.
    #[error("argon2 parameter error: {0}")]
    Parameters(String),
    /// Argon2 hashing failed.
    #[error("argon2 hashing error: {0}")]
    Hash(String),
    /// Ticket failed the Argon2 digest predicate.
    #[error("puzzle ticket solution invalid")]
    InvalidSolution,
    /// System clock could not be queried.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
}

/// Errors surfaced while minting puzzle tickets (used for tests and fixtures).
#[derive(Debug, Error)]
pub enum MintError {
    /// Requested TTL shorter than the policy minimum.
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
        /// Maximum future skew derived from policy.
        max_skew: Duration,
    },
    /// System clock could not be queried.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
    /// Argon2 parameter set was invalid.
    #[error("argon2 parameter error: {0}")]
    Parameters(String),
    /// Argon2 hashing failed.
    #[error("argon2 hashing error: {0}")]
    Hash(String),
}

/// Verify a puzzle ticket using the supplied policy.
///
/// # Errors
/// Returns [`Error`] if the ticket metadata violates the policy or if the Argon2 digest
/// fails the work predicate.
pub fn verify(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
) -> Result<(), Error> {
    verify_at(ticket, binding, params, SystemTime::now())
}

/// Verify a ticket at a fixed timestamp (exposed for testing).
///
/// # Errors
/// Returns [`Error`] when the ticket metadata violates policy bounds or the derived digest
/// fails the work predicate.
#[allow(clippy::too_many_lines)]
pub fn verify_at(
    ticket: &Ticket,
    binding: &ChallengeBinding<'_>,
    params: &Parameters,
    now: SystemTime,
) -> Result<(), Error> {
    if ticket.version != 1 {
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
    let deficit = params.min_ticket_ttl.saturating_sub(ttl_remaining);
    if deficit > TTL_GRACE {
        return Err(Error::ExpiryWindowTooSmall(params.min_ticket_ttl));
    }
    if ttl_remaining > params.max_future_skew {
        return Err(Error::FutureSkewExceeded(params.max_future_skew));
    }

    let challenge = derive_challenge(binding, ticket.client_nonce, ticket.expires_at);
    let digest =
        derive_solution_digest(&challenge, &ticket.solution, params).map_err(|err| match err {
            DigestError::Parameters(msg) => Error::Parameters(msg),
            DigestError::Hash(msg) => Error::Hash(msg),
        })?;
    if !leading_zero_bits_at_least(&digest, params.difficulty) {
        return Err(Error::InvalidSolution);
    }

    Ok(())
}

/// Mint a puzzle ticket for the given descriptor commitment and TTL.
///
/// This helper exists primarily for tests/fixtures; production clients should
/// derive their own solution search strategy.
///
/// # Errors
/// Returns [`MintError`] when the requested TTL falls outside policy bounds or when digest
/// derivation fails.
pub fn mint_ticket<R: RngCore + CryptoRng>(
    params: &Parameters,
    binding: &ChallengeBinding<'_>,
    ttl: Duration,
    rng: &mut R,
) -> Result<Ticket, MintError> {
    if ttl < params.min_ticket_ttl {
        return Err(MintError::TtlTooShort {
            requested: ttl,
            required: params.min_ticket_ttl,
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
        let digest =
            derive_solution_digest(&challenge, &solution, params).map_err(|err| match err {
                DigestError::Parameters(msg) => MintError::Parameters(msg),
                DigestError::Hash(msg) => MintError::Hash(msg),
            })?;
        if leading_zero_bits_at_least(&digest, params.difficulty) {
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
    let relay_len = binding.relay_id.len();
    let mut input = Vec::with_capacity(
        CHALLENGE_DOMAIN.len()
            + binding.descriptor_commit.len()
            + relay_len
            + binding.transcript_hash.map_or(0, <[u8]>::len)
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
    blake3::hash(&input)
}

fn derive_solution_digest(
    challenge: &blake3::Hash,
    solution: &[u8; 32],
    params: &Parameters,
) -> Result<[u8; OUTPUT_LEN], DigestError> {
    let argon_params = Params::new(
        params.memory_kib.get(),
        params.time_cost.get(),
        params.lanes.get(),
        Some(OUTPUT_LEN),
    )
    .map_err(|err| DigestError::Parameters(err.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

    let mut input = Vec::with_capacity(SOLUTION_DOMAIN.len() + challenge.as_bytes().len());
    input.extend_from_slice(SOLUTION_DOMAIN);
    input.extend_from_slice(challenge.as_bytes());

    let mut output = [0u8; OUTPUT_LEN];
    argon2
        .hash_password_into(solution, &input, &mut output)
        .map_err(|err| DigestError::Hash(err.to_string()))?;
    Ok(output)
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
    let mask = 0xFFu8 << (8 - rem_bits);
    bytes[full_bytes] & mask == 0
}

#[derive(Debug)]
enum DigestError {
    Parameters(String),
    Hash(String),
}

impl fmt::Display for DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Parameters(err) | Self::Hash(err) => err,
        };
        write!(f, "{message}")
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    use super::*;

    const DESCRIPTOR: [u8; 32] = [0x11; 32];
    const RELAY: [u8; 32] = [0x22; 32];
    const OTHER_RELAY: [u8; 32] = [0x99; 32];

    fn test_parameters() -> Parameters {
        Parameters::new(
            NonZeroU32::new(8 * 1024).unwrap(),
            NonZeroU32::new(2).unwrap(),
            NonZeroU32::new(1).unwrap(),
            4,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
    }

    fn binding() -> ChallengeBinding<'static> {
        ChallengeBinding::new(&DESCRIPTOR, &RELAY, None)
    }

    #[test]
    fn mint_and_verify_ticket() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([7u8; 32]);
        let binding = binding();
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");
        verify(&ticket, &binding, &params).expect("verify");
    }

    #[test]
    fn invalid_solution_rejected() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([9u8; 32]);
        let binding = binding();
        let mut ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");
        ticket.solution[0] ^= 0xFF;
        let err = verify(&ticket, &binding, &params).expect_err("should fail");
        assert!(matches!(err, Error::InvalidSolution));
    }

    #[test]
    fn ttl_constraints_enforced() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        let binding = binding();
        let err =
            mint_ticket(&params, &binding, Duration::from_secs(1), &mut rng).expect_err("ttl");
        assert!(matches!(err, MintError::TtlTooShort { .. }));

        let err = verify_at(
            &Ticket {
                version: 1,
                difficulty: params.difficulty,
                expires_at: 1,
                client_nonce: [0u8; 32],
                solution: [0u8; 32],
            },
            &binding,
            &params,
            UNIX_EPOCH + Duration::from_secs(2),
        )
        .expect_err("expired");
        assert!(matches!(err, Error::Expired(_, _)));
    }

    #[test]
    fn rejects_mismatched_transcript_hash() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([0xAA; 32]);
        let transcript_a = [0x11; 32];
        let transcript_b = [0x22; 32];
        let binding = ChallengeBinding::new(&DESCRIPTOR, &RELAY, Some(&transcript_a));
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(12), &mut rng).expect("mint");

        verify(&ticket, &binding, &params).expect("expected transcript to verify");
        let mismatched = ChallengeBinding::new(&DESCRIPTOR, &RELAY, Some(&transcript_b));
        let err = verify(&ticket, &mismatched, &params)
            .expect_err("transcript mismatch should reject ticket");
        assert!(matches!(err, Error::InvalidSolution));
    }

    #[test]
    fn binding_to_relay_id_enforced() {
        // Keep the test difficulty modest so the Argon2 search finishes quickly
        // while still exercising the relay binding logic.
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([5u8; 32]);
        let binding = binding();
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");

        let mismatched = ChallengeBinding::new(binding.descriptor_commit, &OTHER_RELAY, None);
        let err = verify(&ticket, &mismatched, &params).expect_err("relay binding should fail");
        assert!(matches!(err, Error::InvalidSolution));
    }
}
