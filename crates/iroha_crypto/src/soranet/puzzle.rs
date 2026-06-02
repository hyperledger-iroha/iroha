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
const BINDING_FIELD_LEN: usize = 32;

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

/// Errors surfaced while constructing Argon2 puzzle policy parameters.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum ParameterError {
    /// The minimum ticket TTL must be non-zero.
    #[error("puzzle min_ticket_ttl must be greater than zero")]
    MinTicketTtlZero,
    /// The maximum future skew must cover the minimum ticket TTL.
    #[error(
        "puzzle max_future_skew {max_future_skew:?} is shorter than min_ticket_ttl {min_ticket_ttl:?}"
    )]
    MaxFutureSkewTooShort {
        /// Configured maximum future skew.
        max_future_skew: Duration,
        /// Configured minimum ticket TTL.
        min_ticket_ttl: Duration,
    },
}

impl Parameters {
    /// Construct a new parameter set, panicking when bounds are invalid.
    ///
    /// # Panics
    /// Panics when `min_ticket_ttl` is zero or when `max_future_skew` is
    /// shorter than the minimum TTL. Runtime configuration loaders should
    /// prefer [`Parameters::try_new`] so invalid policy input can fail closed
    /// without unwinding.
    #[must_use]
    pub fn new(
        memory_kib: NonZeroU32,
        time_cost: NonZeroU32,
        lanes: NonZeroU32,
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
    ) -> Self {
        Self::try_new(
            memory_kib,
            time_cost,
            lanes,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
        )
        .unwrap_or_else(|err| match err {
            ParameterError::MinTicketTtlZero => {
                panic!("min_ticket_ttl must be greater than zero")
            }
            ParameterError::MaxFutureSkewTooShort { .. } => {
                panic!("max_future_skew must be at least min_ticket_ttl")
            }
        })
    }

    /// Construct a new parameter set.
    ///
    /// # Errors
    /// Returns [`ParameterError`] if the minimum ticket TTL is zero or if the
    /// maximum future skew is shorter than the minimum ticket TTL.
    pub fn try_new(
        memory_kib: NonZeroU32,
        time_cost: NonZeroU32,
        lanes: NonZeroU32,
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
    ) -> Result<Self, ParameterError> {
        if min_ticket_ttl.is_zero() {
            return Err(ParameterError::MinTicketTtlZero);
        }
        if max_future_skew < min_ticket_ttl {
            return Err(ParameterError::MaxFutureSkewTooShort {
                max_future_skew,
                min_ticket_ttl,
            });
        }
        Ok(Self {
            memory_kib,
            time_cost,
            lanes,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
        })
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
    /// Binding material has an invalid field length.
    #[error("malformed puzzle binding: {0}")]
    MalformedBinding(String),
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
    /// Binding material has an invalid field length.
    #[error("malformed puzzle binding: {0}")]
    MalformedBinding(String),
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
    validate_binding(binding).map_err(Error::MalformedBinding)?;
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
    validate_binding(binding).map_err(MintError::MalformedBinding)?;
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

    #[test]
    fn parameters_try_new_rejects_invalid_runtime_bounds() {
        let memory = NonZeroU32::new(8 * 1024).expect("non-zero memory");
        let time = NonZeroU32::new(2).expect("non-zero time");
        let lanes = NonZeroU32::new(1).expect("non-zero lanes");

        let valid = Parameters::try_new(
            memory,
            time,
            lanes,
            4,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
        .expect("valid bounds");
        assert_eq!(valid.difficulty(), 4);

        let zero_ttl = Parameters::try_new(
            memory,
            time,
            lanes,
            4,
            Duration::from_secs(30),
            Duration::ZERO,
        )
        .expect_err("zero min ttl must fail");
        assert!(matches!(zero_ttl, ParameterError::MinTicketTtlZero));

        let inverted = Parameters::try_new(
            memory,
            time,
            lanes,
            4,
            Duration::from_secs(4),
            Duration::from_secs(5),
        )
        .expect_err("max future skew shorter than min ttl must fail");
        assert!(matches!(
            inverted,
            ParameterError::MaxFutureSkewTooShort {
                max_future_skew,
                min_ticket_ttl
            } if max_future_skew == Duration::from_secs(4)
                && min_ticket_ttl == Duration::from_secs(5)
        ));
    }

    fn binding() -> ChallengeBinding<'static> {
        ChallengeBinding::new(&DESCRIPTOR, &RELAY, None)
    }

    fn first_invalid_solution(
        ticket: Ticket,
        binding: &ChallengeBinding<'_>,
        params: &Parameters,
    ) -> [u8; 32] {
        for idx in 0..ticket.solution.len() {
            for bit in 0..8 {
                let mut candidate = ticket.solution;
                candidate[idx] ^= 1u8 << bit;
                let challenge = derive_challenge(binding, ticket.client_nonce, ticket.expires_at);
                let digest = derive_solution_digest(&challenge, &candidate, params)
                    .expect("digest derivation should succeed");
                if !leading_zero_bits_at_least(&digest, params.difficulty) {
                    return candidate;
                }
            }
        }
        panic!("failed to construct an invalid solution candidate")
    }

    fn first_invalid_relay_id(
        ticket: &Ticket,
        params: &Parameters,
        base: &ChallengeBinding<'_>,
    ) -> [u8; 32] {
        for seed in 0u8..=u8::MAX {
            let mut relay = [0u8; 32];
            for (idx, byte) in relay.iter_mut().enumerate() {
                let idx = u8::try_from(idx).expect("relay index fits in u8");
                *byte = seed.wrapping_add(idx);
            }
            if relay.as_slice() == base.relay_id {
                continue;
            }
            let candidate =
                ChallengeBinding::new(base.descriptor_commit, &relay, base.transcript_hash);
            let challenge = derive_challenge(&candidate, ticket.client_nonce, ticket.expires_at);
            let digest = derive_solution_digest(&challenge, &ticket.solution, params)
                .expect("digest derivation should succeed");
            if !leading_zero_bits_at_least(&digest, params.difficulty) {
                return relay;
            }
        }
        panic!("failed to construct an invalid relay binding candidate")
    }

    fn first_invalid_transcript_hash(
        ticket: &Ticket,
        params: &Parameters,
        base: &ChallengeBinding<'_>,
    ) -> [u8; 32] {
        for seed in 0u8..=u8::MAX {
            let mut transcript = [0u8; 32];
            for (idx, byte) in transcript.iter_mut().enumerate() {
                let idx = u8::try_from(idx).expect("transcript index fits in u8");
                *byte = seed.wrapping_add(idx);
            }
            if base.transcript_hash == Some(transcript.as_slice()) {
                continue;
            }
            let candidate =
                ChallengeBinding::new(base.descriptor_commit, base.relay_id, Some(&transcript));
            let challenge = derive_challenge(&candidate, ticket.client_nonce, ticket.expires_at);
            let digest = derive_solution_digest(&challenge, &ticket.solution, params)
                .expect("digest derivation should succeed");
            if !leading_zero_bits_at_least(&digest, params.difficulty) {
                return transcript;
            }
        }
        panic!("failed to construct an invalid transcript binding candidate")
    }

    fn stable_verify_time(ticket: &Ticket, params: &Parameters) -> SystemTime {
        let ttl_floor = params.min_ticket_ttl().as_secs();
        let now_secs = ticket.expires_at.saturating_sub(ttl_floor);
        UNIX_EPOCH + Duration::from_secs(now_secs)
    }

    #[test]
    fn mint_and_verify_ticket() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([7u8; 32]);
        let binding = binding();
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");
        verify_at(
            &ticket,
            &binding,
            &params,
            stable_verify_time(&ticket, &params),
        )
        .expect("verify");
    }

    #[test]
    fn invalid_solution_rejected() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([9u8; 32]);
        let binding = binding();
        let mut ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");
        ticket.solution = first_invalid_solution(ticket, &binding, &params);
        let err = verify_at(
            &ticket,
            &binding,
            &params,
            stable_verify_time(&ticket, &params),
        )
        .expect_err("should fail");
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
    fn verify_rejects_malformed_binding_before_argon2_work() {
        let params = test_parameters().with_difficulty(0);
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: 1_700_000_120,
            client_nonce: [0x66; 32],
            solution: [0x77; 32],
        };
        let short_transcript = [0x33; 31];
        let malformed = ChallengeBinding::new(&DESCRIPTOR, &RELAY, Some(&short_transcript));
        let err = verify_at(&ticket, &malformed, &params, now)
            .expect_err("malformed binding must fail before Argon2 derivation");
        match err {
            Error::MalformedBinding(message) => assert!(
                message.contains("transcript_hash"),
                "unexpected error: {message}"
            ),
            other => panic!("expected malformed binding error, got {other:?}"),
        }
    }

    #[test]
    fn mint_rejects_malformed_binding_before_solution_search() {
        let params = test_parameters().with_difficulty(0);
        let mut rng = ChaCha20Rng::from_seed([0x66; 32]);
        let short_descriptor = [0x11; 31];
        let malformed = ChallengeBinding::new(&short_descriptor, &RELAY, None);
        let err = mint_ticket(&params, &malformed, Duration::from_secs(12), &mut rng)
            .expect_err("malformed binding must fail before minting");
        match err {
            MintError::MalformedBinding(message) => assert!(
                message.contains("descriptor_commit"),
                "unexpected error: {message}"
            ),
            other => panic!("expected malformed binding error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_mismatched_transcript_hash() {
        let params = test_parameters();
        let mut rng = ChaCha20Rng::from_seed([0xAA; 32]);
        let transcript_a = [0x11; 32];
        let binding = ChallengeBinding::new(&DESCRIPTOR, &RELAY, Some(&transcript_a));
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(12), &mut rng).expect("mint");

        let now = stable_verify_time(&ticket, &params);
        verify_at(&ticket, &binding, &params, now).expect("expected transcript to verify");
        let transcript_b = first_invalid_transcript_hash(&ticket, &params, &binding);
        let mismatched = ChallengeBinding::new(&DESCRIPTOR, &RELAY, Some(&transcript_b));
        let err = verify_at(&ticket, &mismatched, &params, now)
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

        let mismatched_relay = first_invalid_relay_id(&ticket, &params, &binding);
        let mismatched = ChallengeBinding::new(
            binding.descriptor_commit,
            &mismatched_relay,
            binding.transcript_hash,
        );
        let err = verify_at(
            &ticket,
            &mismatched,
            &params,
            stable_verify_time(&ticket, &params),
        )
        .expect_err("relay binding should fail");
        assert!(matches!(err, Error::InvalidSolution));
    }
}
