//! `Argon2id`-based puzzle helpers for the `SoraNet` admission path.
//!
//! The puzzle format intentionally mirrors the existing hashcash-style `PoW` tickets so clients can
//! attach a single frame regardless of which policy a relay enforces. Difficulty adjustments and
//! TTL validation follow the same rules as the `PoW` implementation, while the work predicate is
//! backed by Argon2id to raise the cost of GPU/ASIC optimisations.
use crate::soranet::pow::{CHALLENGE_DOMAIN, SOLUTION_DOMAIN, Ticket, ticket_binding_commitment};
use argon2::{Algorithm, Argon2, Params, Version};
use blake3::Hasher;
use rand_core::TryCryptoRng;
use std::{
    fmt,
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq as _;
use thiserror::Error;
const OUTPUT_LEN: usize = 32;
const SOLUTION_SALT_LEN: usize = SOLUTION_DOMAIN.len() + OUTPUT_LEN;
const TTL_GRACE: Duration = Duration::from_secs(1);
const BINDING_FIELD_LEN: usize = 32;
/// Binding inputs mixed into the puzzle challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeBinding<'a> {
    /// Descriptor commitment advertised by the relay (32 bytes).
    pub descriptor_commit: &'a [u8],
    /// Relay identity bytes (32 bytes).
    pub relay_id: &'a [u8],
    /// Transcript hash binding the puzzle to this admission attempt.
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
/// Smallest supported Argon2 puzzle memory cost, in KiB.
pub const MIN_MEMORY_KIB: u32 = 4 * 1024;
/// Largest supported Argon2 puzzle memory cost, in KiB.
///
/// This hard ceiling keeps a single verification within a bounded 128 MiB
/// working set even if an operator supplies a malformed runtime update.
pub const MAX_MEMORY_KIB: u32 = 128 * 1024;
/// Largest supported Argon2 iteration count.
pub const MAX_TIME_COST: u32 = 8;
/// Largest supported Argon2 lane count.
pub const MAX_LANES: u32 = 16;
/// Largest supported proof-of-work difficulty.
///
/// Higher values are operationally indistinguishable from disabling inbound connectivity and
/// therefore are rejected instead of silently partitioning a node.
pub const MAX_DIFFICULTY: u8 = 32;
/// Default first-release proof-of-work difficulty.
///
/// Difficulty zero makes every Argon2 output pass, forcing the verifier to pay
/// the memory-hard cost without requiring equivalent client work.
pub const DEFAULT_DIFFICULTY: u8 = 6;
/// Errors surfaced while constructing Argon2 puzzle policy parameters.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum ParameterError {
    /// The memory cost is outside the supported resource corridor.
    #[error(
        "puzzle memory_kib {configured} is outside the supported range {MIN_MEMORY_KIB}..={MAX_MEMORY_KIB}"
    )]
    MemoryOutOfRange {
        /// Configured memory cost in KiB.
        configured: u32,
    },
    /// The iteration count exceeds the supported resource corridor.
    #[error("puzzle time_cost {configured} exceeds the supported maximum {MAX_TIME_COST}")]
    TimeCostTooHigh {
        /// Configured iteration count.
        configured: u32,
    },
    /// The lane count exceeds the supported resource corridor.
    #[error("puzzle lanes {configured} exceeds the supported maximum {MAX_LANES}")]
    LanesTooHigh {
        /// Configured lane count.
        configured: u32,
    },
    /// The difficulty exceeds the operational connectivity corridor.
    #[error("puzzle difficulty {configured} exceeds the supported maximum {MAX_DIFFICULTY}")]
    DifficultyTooHigh {
        /// Configured difficulty.
        configured: u8,
    },
    /// A zero difficulty would impose verifier cost without client work.
    #[error("puzzle difficulty must be greater than zero")]
    DifficultyZero,
    /// The minimum ticket TTL must be non-zero.
    #[error("puzzle min_ticket_ttl must be greater than zero")]
    MinTicketTtlZero,
    /// The maximum future skew must leave time to solve the puzzle before the
    /// minimum remaining ticket TTL is enforced.
    #[error(
        "puzzle max_future_skew {max_future_skew:?} must exceed min_ticket_ttl {min_ticket_ttl:?}"
    )]
    MaxFutureSkewTooShort {
        /// Configured maximum future skew.
        max_future_skew: Duration,
        /// Configured minimum ticket TTL.
        min_ticket_ttl: Duration,
    },
}
impl Parameters {
    /// Construct a new parameter set.
    ///
    /// Invalid timing bounds produce a fail-closed policy that rejects all minted and verified
    /// tickets. Runtime configuration loaders should prefer [`Parameters::try_new`] so invalid
    /// policy input can be surfaced as a configuration error.
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
        .unwrap_or_else(|_| Self::fail_closed(memory_kib, time_cost, lanes, difficulty))
    }
    fn fail_closed(
        memory_kib: NonZeroU32,
        time_cost: NonZeroU32,
        lanes: NonZeroU32,
        difficulty: u8,
    ) -> Self {
        Self {
            memory_kib: NonZeroU32::new(memory_kib.get().clamp(MIN_MEMORY_KIB, MAX_MEMORY_KIB))
                .expect("bounded puzzle memory is non-zero"),
            time_cost: NonZeroU32::new(time_cost.get().min(MAX_TIME_COST))
                .expect("bounded puzzle time cost is non-zero"),
            lanes: NonZeroU32::new(lanes.get().min(MAX_LANES))
                .expect("bounded puzzle lanes are non-zero"),
            difficulty: difficulty.min(MAX_DIFFICULTY),
            max_future_skew: Duration::ZERO,
            min_ticket_ttl: Duration::MAX,
        }
    }
    /// Construct a new parameter set.
    ///
    /// # Errors
    /// Returns [`ParameterError`] if a computational resource bound, the
    /// difficulty, or the ticket timing corridor is invalid.
    pub fn try_new(
        memory_kib: NonZeroU32,
        time_cost: NonZeroU32,
        lanes: NonZeroU32,
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
    ) -> Result<Self, ParameterError> {
        if !(MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&memory_kib.get()) {
            return Err(ParameterError::MemoryOutOfRange {
                configured: memory_kib.get(),
            });
        }
        if time_cost.get() > MAX_TIME_COST {
            return Err(ParameterError::TimeCostTooHigh {
                configured: time_cost.get(),
            });
        }
        if lanes.get() > MAX_LANES {
            return Err(ParameterError::LanesTooHigh {
                configured: lanes.get(),
            });
        }
        if difficulty == 0 {
            return Err(ParameterError::DifficultyZero);
        }
        if difficulty > MAX_DIFFICULTY {
            return Err(ParameterError::DifficultyTooHigh {
                configured: difficulty,
            });
        }
        if min_ticket_ttl.is_zero() {
            return Err(ParameterError::MinTicketTtlZero);
        }
        if max_future_skew <= min_ticket_ttl {
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
    /// Ticket expiry timestamp cannot be represented by `SystemTime`.
    #[error("puzzle ticket expiry timestamp {0} overflows system time")]
    ExpiryTimestampOverflow(u64),
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
    /// Requested TTL does not leave time to solve the puzzle before the policy
    /// minimum remaining lifetime is enforced.
    #[error("requested ttl {requested:?} must exceed required minimum {required:?}")]
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
    /// System clock could not be queried.
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
    /// The system clock moved backwards while a puzzle solution was being
    /// searched, so the resulting expiry window cannot be trusted.
    #[error("system clock moved backwards while minting puzzle ticket")]
    ClockMovedBackwards,
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
    unix_time_from_secs(ticket.expires_at)
        .ok_or(Error::ExpiryTimestampOverflow(ticket.expires_at))?;
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
    let expected_binding = ticket_binding_commitment(
        binding.descriptor_commit,
        binding.relay_id,
        binding.transcript_hash,
    );
    if !bool::from(ticket.client_nonce.ct_eq(&expected_binding)) {
        return Err(Error::InvalidSolution);
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
/// Returns [`MintError`] when the requested TTL falls outside policy bounds,
/// random bytes cannot be generated, or digest derivation fails.
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
    D: FnMut(&blake3::Hash, &[u8; 32], &Parameters) -> Result<[u8; OUTPUT_LEN], DigestError>,
{
    validate_binding(binding).map_err(MintError::MalformedBinding)?;
    if ttl <= params.min_ticket_ttl {
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
    let client_nonce = ticket_binding_commitment(
        binding.descriptor_commit,
        binding.relay_id,
        binding.transcript_hash,
    );
    let mut previous_solution = None;
    loop {
        let minted_at = now();
        let expires_at = minted_at
            .checked_add(ttl)
            .ok_or(MintError::ExpiryTimestampOverflow(ttl))?;
        let expires_at_secs = expires_at.duration_since(UNIX_EPOCH)?.as_secs();
        let wire_expires_at =
            unix_time_from_secs(expires_at_secs).ok_or(MintError::ExpiryTimestampOverflow(ttl))?;
        let mut prior = Vec::with_capacity(2);
        prior.push(("ticket binding commitment", &client_nonce));
        if let Some(previous) = previous_solution.as_ref() {
            prior.push(("previous solution nonce", previous));
        }
        let challenge = derive_challenge(binding, client_nonce, expires_at_secs);
        let mut solution = [0u8; 32];
        fill_random(rng, "minting puzzle solution nonce", &mut solution)?;
        reject_repeated_nonce_material("minting puzzle solution nonce", &solution, &prior)?;
        previous_solution = Some(solution);
        let digest = derive_digest(&challenge, &solution, params).map_err(|err| match err {
            DigestError::Parameters(msg) => MintError::Parameters(msg),
            DigestError::Hash(msg) => MintError::Hash(msg),
        })?;
        let solved_at = now();
        if solved_at < minted_at {
            return Err(MintError::ClockMovedBackwards);
        }
        let remaining = wire_expires_at
            .duration_since(solved_at)
            .unwrap_or(Duration::ZERO);
        if remaining < params.min_ticket_ttl {
            // The expiry is part of the Argon2 challenge and cannot be
            // extended after solving. Discard the stale candidate and derive
            // a fresh expiry/challenge before the next expensive evaluation.
            continue;
        }
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
    hasher.update(binding.transcript_hash);
    hasher.update(&client_nonce);
    hasher.update(&expires_at.to_be_bytes());
    hasher.finalize()
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
    let salt = derive_solution_salt(challenge);
    let mut output = [0u8; OUTPUT_LEN];
    argon2
        .hash_password_into(solution, &salt, &mut output)
        .map_err(|err| DigestError::Hash(err.to_string()))?;
    Ok(output)
}
fn derive_solution_salt(challenge: &blake3::Hash) -> [u8; SOLUTION_SALT_LEN] {
    let mut salt = [0u8; SOLUTION_SALT_LEN];
    let (domain, challenge_bytes) = salt.split_at_mut(SOLUTION_DOMAIN.len());
    domain.copy_from_slice(SOLUTION_DOMAIN);
    challenge_bytes.copy_from_slice(challenge.as_bytes());
    salt
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
fn unix_time_from_secs(secs: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
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
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{TryCryptoRng, TryRngCore};
    const DESCRIPTOR: [u8; 32] = [0x11; 32];
    const RELAY: [u8; 32] = [0x22; 32];
    const TRANSCRIPT: [u8; 32] = [0x33; 32];
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
    struct FailingTryRng;
    #[derive(Debug)]
    struct FailingTryRngError;
    impl std::fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing puzzle ticket RNG")
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
    #[test]
    #[expect(clippy::too_many_lines, reason = "cohesive parameter rejection matrix")]
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
        for configured in [MIN_MEMORY_KIB - 1, MAX_MEMORY_KIB + 1] {
            let error = Parameters::try_new(
                NonZeroU32::new(configured).expect("non-zero memory"),
                time,
                lanes,
                4,
                Duration::from_secs(30),
                Duration::from_secs(5),
            )
            .expect_err("out-of-range memory must fail");
            assert_eq!(error, ParameterError::MemoryOutOfRange { configured });
        }
        assert_eq!(
            Parameters::try_new(
                memory,
                time,
                lanes,
                0,
                Duration::from_secs(30),
                Duration::from_secs(5),
            )
            .expect_err("zero difficulty must fail"),
            ParameterError::DifficultyZero
        );
        assert_eq!(
            Parameters::try_new(
                memory,
                NonZeroU32::new(MAX_TIME_COST + 1).expect("non-zero time"),
                lanes,
                4,
                Duration::from_secs(30),
                Duration::from_secs(5),
            )
            .expect_err("excessive time cost must fail"),
            ParameterError::TimeCostTooHigh {
                configured: MAX_TIME_COST + 1,
            }
        );
        assert_eq!(
            Parameters::try_new(
                memory,
                time,
                NonZeroU32::new(MAX_LANES + 1).expect("non-zero lanes"),
                4,
                Duration::from_secs(30),
                Duration::from_secs(5),
            )
            .expect_err("excessive lane count must fail"),
            ParameterError::LanesTooHigh {
                configured: MAX_LANES + 1,
            }
        );
        assert_eq!(
            Parameters::try_new(
                memory,
                time,
                lanes,
                MAX_DIFFICULTY + 1,
                Duration::from_secs(30),
                Duration::from_secs(5),
            )
            .expect_err("excessive difficulty must fail"),
            ParameterError::DifficultyTooHigh {
                configured: MAX_DIFFICULTY + 1,
            }
        );
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
        let no_solution_window = Parameters::try_new(
            memory,
            time,
            lanes,
            4,
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .expect_err("equal future skew and minimum ttl leave no puzzle solution window");
        assert!(matches!(
            no_solution_window,
            ParameterError::MaxFutureSkewTooShort {
                max_future_skew,
                min_ticket_ttl
            } if max_future_skew == Duration::from_secs(5)
                && min_ticket_ttl == Duration::from_secs(5)
        ));
    }
    #[test]
    fn parameters_new_invalid_bounds_fail_closed_without_panic() {
        let memory = NonZeroU32::new(8 * 1024).expect("non-zero memory");
        let time = NonZeroU32::new(2).expect("non-zero time");
        let lanes = NonZeroU32::new(1).expect("non-zero lanes");
        let zero_ttl = Parameters::new(
            memory,
            time,
            lanes,
            0,
            Duration::from_secs(30),
            Duration::ZERO,
        );
        assert_eq!(zero_ttl.max_future_skew(), Duration::ZERO);
        assert_eq!(zero_ttl.min_ticket_ttl(), Duration::MAX);
        let inverted = Parameters::new(
            memory,
            time,
            lanes,
            0,
            Duration::from_secs(4),
            Duration::from_secs(5),
        );
        assert_eq!(inverted.max_future_skew(), Duration::ZERO);
        assert_eq!(inverted.min_ticket_ttl(), Duration::MAX);
        let excessive = Parameters::new(
            NonZeroU32::new(u32::MAX).expect("non-zero memory"),
            NonZeroU32::new(u32::MAX).expect("non-zero time"),
            NonZeroU32::new(u32::MAX).expect("non-zero lanes"),
            u8::MAX,
            Duration::from_secs(30),
            Duration::from_secs(5),
        );
        assert_eq!(excessive.memory_kib().get(), MAX_MEMORY_KIB);
        assert_eq!(excessive.time_cost().get(), MAX_TIME_COST);
        assert_eq!(excessive.lanes().get(), MAX_LANES);
        assert_eq!(excessive.difficulty(), MAX_DIFFICULTY);
        assert_eq!(excessive.max_future_skew(), Duration::ZERO);
        assert_eq!(excessive.min_ticket_ttl(), Duration::MAX);
        let mut rng = ChaCha20Rng::seed_from_u64(99);
        let mint_err = mint_ticket(&zero_ttl, &binding(), Duration::from_secs(5), &mut rng)
            .expect_err("fail-closed params must reject minting");
        assert!(matches!(
            mint_err,
            MintError::TtlTooShort {
                required: Duration::MAX,
                ..
            }
        ));
        let ticket = Ticket {
            version: 1,
            difficulty: 0,
            expires_at: 1_120,
            client_nonce: [0u8; 32],
            solution: [0u8; 32],
        };
        let verify_err = verify_at(
            &ticket,
            &binding(),
            &inverted,
            UNIX_EPOCH + Duration::from_secs(1_000),
        )
        .expect_err("fail-closed params must reject verification");
        assert!(matches!(
            verify_err,
            Error::ExpiryWindowTooSmall(Duration::MAX)
        ));
    }
    fn binding() -> ChallengeBinding<'static> {
        ChallengeBinding::new(&DESCRIPTOR, &RELAY, &TRANSCRIPT)
    }
    #[test]
    fn mint_ticket_reports_rng_failure() {
        let mut rng = FailingTryRng;
        let err = mint_ticket(
            &test_parameters(),
            &binding(),
            Duration::from_secs(6),
            &mut rng,
        )
        .expect_err("failing RNG must abort ticket minting");
        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting puzzle solution nonce");
                assert!(
                    message.contains("failing puzzle ticket RNG"),
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
        let err = fill_random(&mut rng, "minting puzzle solution nonce", &mut nonce)
            .expect_err("all-zero puzzle nonce material must fail");
        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting puzzle solution nonce");
                assert!(message.contains("all-zero material"));
            }
            other => panic!("expected all-zero nonce RandomBytes error, got {other:?}"),
        }
    }
    #[test]
    fn mint_ticket_rejects_repeated_nonzero_rng_material() {
        let mut rng = FixedTryRng { byte: 0xA5 };
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let error = mint_ticket_with_clock_and_digest(
            &test_parameters(),
            &binding(),
            Duration::from_secs(10),
            &mut rng,
            || now,
            |_, _, _| Ok([0xFF; OUTPUT_LEN]),
        )
        .expect_err("a stuck nonzero RNG must fail before repeated Argon2 work");
        match error {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting puzzle solution nonce");
                assert!(message.contains("all-identical-byte material"));
            }
            other => panic!("expected repeated nonce failure, got {other:?}"),
        }
    }
    #[test]
    fn challenge_hashes_match_canonical_contiguous_layout() {
        let transcript = [0x33; 32];
        let client_nonce = [0x44; 32];
        let expires_at = 1_700_000_123_u64;
        let binding = ChallengeBinding::new(&DESCRIPTOR, &RELAY, &transcript);
        let mut expected_challenge = Vec::with_capacity(
            CHALLENGE_DOMAIN.len()
                + DESCRIPTOR.len()
                + RELAY.len()
                + transcript.len()
                + client_nonce.len()
                + 8,
        );
        expected_challenge.extend_from_slice(CHALLENGE_DOMAIN);
        expected_challenge.extend_from_slice(&DESCRIPTOR);
        expected_challenge.extend_from_slice(&RELAY);
        expected_challenge.extend_from_slice(&transcript);
        expected_challenge.extend_from_slice(&client_nonce);
        expected_challenge.extend_from_slice(&expires_at.to_be_bytes());
        assert_eq!(
            derive_challenge(&binding, client_nonce, expires_at),
            blake3::hash(&expected_challenge)
        );
        let params = test_parameters();
        let binding = ChallengeBinding::new(&DESCRIPTOR, &RELAY, &transcript);
        let challenge = derive_challenge(&binding, client_nonce, expires_at);
        let mut expected_salt =
            Vec::with_capacity(SOLUTION_DOMAIN.len() + challenge.as_bytes().len());
        expected_salt.extend_from_slice(SOLUTION_DOMAIN);
        expected_salt.extend_from_slice(challenge.as_bytes());
        assert_eq!(
            derive_solution_salt(&challenge).as_slice(),
            expected_salt.as_slice()
        );
        let solution = [0x55; 32];
        let argon_params = Params::new(
            params.memory_kib.get(),
            params.time_cost.get(),
            params.lanes.get(),
            Some(OUTPUT_LEN),
        )
        .expect("valid argon parameters");
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
        let mut expected = [0u8; OUTPUT_LEN];
        argon2
            .hash_password_into(&solution, &expected_salt, &mut expected)
            .expect("canonical argon2 digest");
        assert_eq!(
            derive_solution_digest(&challenge, &solution, &params)
                .expect("derive puzzle solution digest"),
            expected
        );
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
    fn first_invalid_expiry(
        ticket: &Ticket,
        binding: &ChallengeBinding<'_>,
        params: &Parameters,
    ) -> u64 {
        let max_delta = params
            .max_future_skew()
            .saturating_sub(params.min_ticket_ttl())
            .as_secs();
        for delta in 1..=max_delta {
            let Some(expires_at) = ticket.expires_at.checked_add(delta) else {
                break;
            };
            let challenge = derive_challenge(binding, ticket.client_nonce, expires_at);
            let digest = derive_solution_digest(&challenge, &ticket.solution, params)
                .expect("digest derivation should succeed");
            if !leading_zero_bits_at_least(&digest, params.difficulty) {
                return expires_at;
            }
        }
        panic!("failed to construct an invalid expiry candidate")
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
        let err = mint_ticket(&params, &binding, params.min_ticket_ttl(), &mut rng)
            .expect_err("ttl equal to the required remaining lifetime leaves no solve window");
        assert!(matches!(
            err,
            MintError::TtlTooShort {
                requested,
                required
            } if requested == params.min_ticket_ttl()
                && required == params.min_ticket_ttl()
        ));
    }
    #[test]
    fn mint_reanchors_each_candidate_across_long_search() {
        let params = Parameters::try_new(
            NonZeroU32::new(MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
        .expect("valid puzzle parameters");
        let mut rng = ChaCha20Rng::from_seed([0x37; 32]);
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut clock_reads = 0_u64;
        let mut digest_trials = 0_u64;
        let ticket = mint_ticket_with_clock_and_digest(
            &params,
            &binding(),
            Duration::from_secs(10),
            &mut rng,
            || {
                let read = clock_reads;
                clock_reads += 1;
                let candidate = read / 2;
                let offset = candidate * 6 + u64::from(read % 2 == 1);
                base + Duration::from_secs(offset)
            },
            |challenge, solution, params| {
                digest_trials += 1;
                let digest = derive_solution_digest(challenge, solution, params)?;
                if digest_trials <= 7 {
                    Ok([0xFF; OUTPUT_LEN])
                } else {
                    Ok(digest)
                }
            },
        )
        .expect("failed search history must not consume the successful candidate's ttl");
        assert!(digest_trials >= 8, "seven forced failures must be retried");
        let successful_candidate = clock_reads / 2 - 1;
        let solved_at = base + Duration::from_secs(successful_candidate * 6 + 1);
        assert_eq!(ticket.expires_at, 1_700_000_010 + successful_candidate * 6);
        assert!(solved_at.duration_since(base).expect("ordered clock") >= Duration::from_secs(43));
        verify_at(&ticket, &binding(), &params, solved_at)
            .expect("fresh successful candidate must satisfy remaining-ttl policy");
    }
    #[test]
    fn mint_discards_valid_candidate_that_completed_below_ttl_floor() {
        let params = Parameters::try_new(
            NonZeroU32::new(MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
        .expect("valid puzzle parameters");
        let mut rng = ChaCha20Rng::from_seed([0x48; 32]);
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let after_stale_candidate = base + Duration::from_secs(6);
        let mut clock_reads = 0_u64;
        let mut digest_trials = 0_u64;
        let ticket = mint_ticket_with_clock_and_digest(
            &params,
            &binding(),
            Duration::from_secs(10),
            &mut rng,
            || {
                clock_reads += 1;
                if clock_reads == 1 {
                    base
                } else {
                    after_stale_candidate
                }
            },
            |challenge, solution, params| {
                digest_trials += 1;
                if digest_trials == 1 {
                    Ok([0; OUTPUT_LEN])
                } else {
                    derive_solution_digest(challenge, solution, params)
                }
            },
        )
        .expect("valid-but-stale candidate must be discarded before returning");
        assert!(digest_trials >= 2, "stale valid candidate must be retried");
        assert_eq!(ticket.expires_at, 1_700_000_016);
        verify_at(&ticket, &binding(), &params, after_stale_candidate)
            .expect("replacement candidate must verify with a fresh ttl window");
    }
    #[test]
    fn changing_refreshed_expiry_invalidates_solution_binding() {
        let params = test_parameters();
        let binding = binding();
        let mut rng = ChaCha20Rng::from_seed([0x59; 32]);
        let mut ticket =
            mint_ticket(&params, &binding, Duration::from_secs(10), &mut rng).expect("mint");
        let verify_time = stable_verify_time(&ticket, &params);
        verify_at(&ticket, &binding, &params, verify_time).expect("baseline ticket verifies");
        ticket.expires_at = first_invalid_expiry(&ticket, &binding, &params);
        let err = verify_at(&ticket, &binding, &params, verify_time)
            .expect_err("expiry substitution must invalidate the Argon2 challenge");
        assert!(matches!(err, Error::InvalidSolution));
    }
    #[test]
    fn mint_fails_closed_when_clock_moves_backwards_during_search() {
        let params = Parameters::try_new(
            NonZeroU32::new(MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
        .expect("valid puzzle parameters");
        let mut rng = ChaCha20Rng::from_seed([0x83; 32]);
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut clock_reads = 0_u8;
        let err = mint_ticket_with_clock(
            &params,
            &binding(),
            Duration::from_secs(10),
            &mut rng,
            || {
                clock_reads = clock_reads.saturating_add(1);
                if clock_reads == 1 {
                    base
                } else {
                    base - Duration::from_secs(1)
                }
            },
        )
        .expect_err("clock rollback must not produce a future-skewed ticket");
        assert!(matches!(err, MintError::ClockMovedBackwards));
    }
    #[test]
    fn mint_rejects_ttl_that_overflows_system_time() {
        let memory = NonZeroU32::new(MIN_MEMORY_KIB).expect("non-zero memory");
        let time = NonZeroU32::new(1).expect("non-zero time");
        let lanes = NonZeroU32::new(1).expect("non-zero lanes");
        let params = Parameters::try_new(
            memory,
            time,
            lanes,
            1,
            Duration::from_secs(u64::MAX),
            Duration::from_secs(1),
        )
        .expect("huge bounds are structurally valid");
        let mut rng = ChaCha20Rng::from_seed([0x42; 32]);
        let binding = binding();
        let err = mint_ticket(&params, &binding, Duration::from_secs(u64::MAX), &mut rng)
            .expect_err("overflowing ttl should fail closed");
        assert!(matches!(
            err,
            MintError::ExpiryTimestampOverflow(ttl)
                if ttl == Duration::from_secs(u64::MAX)
        ));
    }
    #[test]
    fn verify_rejects_unrepresentable_expiry_before_argon2_work() {
        let params = test_parameters();
        let ticket = Ticket {
            version: Ticket::VERSION,
            difficulty: params.difficulty(),
            expires_at: u64::MAX,
            client_nonce: [0xAA; 32],
            solution: [0xBB; 32],
        };
        let err = verify_at(
            &ticket,
            &binding(),
            &params,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .expect_err("unrepresentable expiry must fail before Argon2 work");
        assert!(matches!(err, Error::ExpiryTimestampOverflow(u64::MAX)));
    }
    #[test]
    fn mint_rejects_malformed_binding_before_solution_search() {
        let params = test_parameters().with_difficulty(0);
        let mut rng = ChaCha20Rng::from_seed([0x66; 32]);
        let short_descriptor = [0x11; 31];
        let malformed = ChallengeBinding::new(&short_descriptor, &RELAY, &TRANSCRIPT);
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
        let binding = ChallengeBinding::new(&DESCRIPTOR, &RELAY, &transcript_a);
        let ticket =
            mint_ticket(&params, &binding, Duration::from_secs(12), &mut rng).expect("mint");
        let now = stable_verify_time(&ticket, &params);
        verify_at(&ticket, &binding, &params, now).expect("expected transcript to verify");
        let transcript_b = [0x22; 32];
        let mismatched = ChallengeBinding::new(&DESCRIPTOR, &RELAY, &transcript_b);
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
        let mismatched_relay = [0x44; 32];
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
