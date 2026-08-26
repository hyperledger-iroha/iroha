//! Native wallet boundary for restart-safe SORA Parliament timed-OVN ballots.
//!
//! The caller retains one independently generated 32-byte seed in its platform
//! keystore. This module derives purpose- and session-separated deterministic
//! randomness from that seed, reconstructs the deliberately non-serializable
//! registration secret, and returns only public registration or masked-ballot
//! records. Before the seed is read, the bridge authenticates a terminal
//! checkpoint-to-tip proof against an independently configured network,
//! checkpoint context, and ballot attempt. It then replay-validates the Core
//! archive and requires its compact binding to equal the authenticated leaf.

use core::ffi::c_char;
use std::{ptr, slice};

use iroha_core::{
    governance::timed_ovn::{
        TIMED_OVN_BALLOT_RECORD_BYTES_V1, TIMED_OVN_REGISTRATION_RECORD_BYTES_V1,
    },
    tle_release::{
        PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1,
        ParliamentTimedOvnCastingContextArchiveV1, ParliamentTimedOvnCastingPhaseV1,
        ValidatedParliamentTimedOvnCastingContextArchiveV1,
    },
};
use iroha_crypto::{
    Hash, HashOf,
    timed_ovn::{TimedOvnChoiceV1, TimedOvnRegistrationSecretV1, TimedOvnRegistrationV1},
};
use iroha_data_model::{
    NetworkId,
    account::{AccountAddress, AccountId},
    block::BlockHeader,
    governance::types::{BallotAttemptId, parliament_ballot_participant_hash_v1},
};
use iroha_torii_shared::parliament_api::{
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_RESPONSE_BYTES_V1,
    ParliamentTimedOvnCastingProofResponseV1,
};
use libc::{c_int, c_uchar, c_ulong};
use rand::{TryCryptoRng, TryRngCore};
use zeroize::Zeroizing;

use super::{BridgeError, write_bytes_bridge};

/// Exact width of the caller-keystore-held root seed.
pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1: usize = 32;
/// Exact width of every caller-configured Parliament casting trust-anchor hash.
pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1: usize = 32;
/// Maximum canonical proof response accepted at the wallet boundary.
pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1: usize =
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_RESPONSE_BYTES_V1;

const REGISTRATION_RNG_DOMAIN_V1: &[u8] =
    b"iroha.connect.parliament.timed-ovn.registration-rng.v1\0";
const BALLOT_RNG_DOMAIN_V1: &[u8] = b"iroha.connect.parliament.timed-ovn.ballot-rng.v1\0";
const RNG_BLOCK_DOMAIN_V1: &[u8] = b"iroha.connect.parliament.timed-ovn.rng-block.v1\0";
const PUBLIC_ARCHIVE_NESTING_LIMIT_V1: usize = 64;
const PUBLIC_PROOF_NESTING_LIMIT_V1: usize = 128;
pub(super) const AUTHORITY_UTF8_MAX_BYTES_V1: usize = 8 * 1024;

#[derive(Debug)]
struct DeterministicRngExhausted;

impl core::fmt::Display for DeterministicRngExhausted {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("timed-OVN deterministic wallet RNG exhausted")
    }
}

impl std::error::Error for DeterministicRngExhausted {}

/// Zeroizing counter-mode BLAKE3 PRF used only after a keyed, contextual derivation.
struct KeyedBlake3Rng {
    key: Zeroizing<[u8; 32]>,
    block: Zeroizing<[u8; 32]>,
    block_cursor: usize,
    next_counter: u64,
}

impl KeyedBlake3Rng {
    fn derive(root_seed: &[u8; 32], context: &[u8]) -> Self {
        let key = Zeroizing::new(*blake3::keyed_hash(root_seed, context).as_bytes());
        Self {
            key,
            block: Zeroizing::new([0_u8; 32]),
            block_cursor: 32,
            next_counter: 0,
        }
    }

    fn refill(&mut self) -> Result<(), DeterministicRngExhausted> {
        let counter = self.next_counter;
        self.next_counter = self
            .next_counter
            .checked_add(1)
            .ok_or(DeterministicRngExhausted)?;
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(RNG_BLOCK_DOMAIN_V1);
        hasher.update(&counter.to_be_bytes());
        self.block.copy_from_slice(hasher.finalize().as_bytes());
        self.block_cursor = 0;
        Ok(())
    }
}

impl TryRngCore for KeyedBlake3Rng {
    type Error = DeterministicRngExhausted;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0_u8; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0_u8; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn try_fill_bytes(&mut self, mut destination: &mut [u8]) -> Result<(), Self::Error> {
        while !destination.is_empty() {
            if self.block_cursor == self.block.len() {
                self.refill()?;
            }
            let available = self.block.len() - self.block_cursor;
            let copied = available.min(destination.len());
            destination[..copied]
                .copy_from_slice(&self.block[self.block_cursor..self.block_cursor + copied]);
            self.block_cursor += copied;
            destination = &mut destination[copied..];
        }
        Ok(())
    }
}

impl TryCryptoRng for KeyedBlake3Rng {}

fn registration_rng(
    root_seed: &[u8; 32],
    session_digest: &[u8; 32],
    participant_hash: &[u8; 32],
) -> KeyedBlake3Rng {
    let mut context = Vec::with_capacity(REGISTRATION_RNG_DOMAIN_V1.len() + 64);
    context.extend_from_slice(REGISTRATION_RNG_DOMAIN_V1);
    context.extend_from_slice(session_digest);
    context.extend_from_slice(participant_hash);
    KeyedBlake3Rng::derive(root_seed, &context)
}

fn ballot_rng(
    root_seed: &[u8; 32],
    session_digest: &[u8; 32],
    participant_hash: &[u8; 32],
    survivor_root: &[u8; 32],
    release_identity_digest: &[u8; 32],
    choice: TimedOvnChoiceV1,
) -> KeyedBlake3Rng {
    let mut context = Vec::with_capacity(BALLOT_RNG_DOMAIN_V1.len() + 32 * 4 + 1);
    context.extend_from_slice(BALLOT_RNG_DOMAIN_V1);
    context.extend_from_slice(session_digest);
    context.extend_from_slice(participant_hash);
    context.extend_from_slice(survivor_root);
    context.extend_from_slice(release_identity_digest);
    context.push(choice as u8);
    KeyedBlake3Rng::derive(root_seed, &context)
}

fn participant_hash(
    context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authority: &AccountId,
) -> [u8; 32] {
    parliament_ballot_participant_hash_v1(
        BallotAttemptId::new(context.archive().session().ballot_attempt_id),
        authority,
    )
}

fn registration_record_from_seed(
    context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authority: &AccountId,
    root_seed: &[u8; 32],
) -> Result<Vec<u8>, BridgeError> {
    if context.archive().phase() != ParliamentTimedOvnCastingPhaseV1::Registered {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let session = context.timed_ovn_session();
    let participant_hash = participant_hash(context, authority);
    let mut rng = registration_rng(root_seed, &session.digest(), &participant_hash);
    let (_, registration) =
        TimedOvnRegistrationSecretV1::generate_with_rng(session, participant_hash, &mut rng)
            .map_err(|_| BridgeError::ParliamentTimedOvn)?;
    let record = registration.to_bytes();
    if record.len() != TIMED_OVN_REGISTRATION_RECORD_BYTES_V1 {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    for existing_record in context.archive().registration_records() {
        let existing = TimedOvnRegistrationV1::from_bytes(session, existing_record)
            .map_err(|_| BridgeError::ParliamentTimedOvn)?;
        if existing.participant_hash() == &participant_hash && existing_record != &record {
            return Err(BridgeError::ParliamentTimedOvn);
        }
    }
    Ok(record)
}

fn ballot_record_from_seed(
    context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authority: &AccountId,
    root_seed: &[u8; 32],
    choice: TimedOvnChoiceV1,
) -> Result<Vec<u8>, BridgeError> {
    if context.archive().phase() != ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let prepared = context
        .prepared_attempt()
        .ok_or(BridgeError::ParliamentTimedOvn)?;
    let session = prepared.registration_roster().session();
    let participant_hash = participant_hash(context, authority);
    let mut registration_rng = registration_rng(root_seed, &session.digest(), &participant_hash);
    let (secret, regenerated_registration) = TimedOvnRegistrationSecretV1::generate_with_rng(
        session,
        participant_hash,
        &mut registration_rng,
    )
    .map_err(|_| BridgeError::ParliamentTimedOvn)?;
    let committed_registration = prepared
        .registration_roster()
        .registrations()
        .binary_search_by_key(&participant_hash, |registration| {
            *registration.participant_hash()
        })
        .ok()
        .and_then(|index| prepared.registration_roster().registrations().get(index))
        .ok_or(BridgeError::ParliamentTimedOvn)?;
    if &regenerated_registration != committed_registration {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let survivor_roster = prepared.survivor_roster();
    let mut ballot_rng = ballot_rng(
        root_seed,
        &session.digest(),
        &participant_hash,
        survivor_roster.survivor_root(),
        survivor_roster.identity_digest(),
        choice,
    );
    let ballot = secret
        .cast_ballot_with_rng(survivor_roster, choice, &mut ballot_rng)
        .map_err(|_| BridgeError::ParliamentTimedOvn)?;
    let record = ballot.to_bytes();
    if record.len() != TIMED_OVN_BALLOT_RECORD_BYTES_V1 {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    Ok(record)
}

fn public_archive_decode_limits(encoded_len: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        encoded_len,
        encoded_len,
        encoded_len,
        encoded_len.saturating_mul(4),
        PUBLIC_ARCHIVE_NESTING_LIMIT_V1,
    )
}

fn public_proof_decode_limits(encoded_len: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        encoded_len,
        encoded_len,
        encoded_len,
        encoded_len.saturating_mul(4),
        PUBLIC_PROOF_NESTING_LIMIT_V1,
    )
}

fn decode_casting_context(
    bytes: &[u8],
) -> Result<ValidatedParliamentTimedOvnCastingContextArchiveV1, BridgeError> {
    if bytes.is_empty() || bytes.len() > PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1 {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let archive: ParliamentTimedOvnCastingContextArchiveV1 =
        norito::decode_canonical_with_limits(bytes, public_archive_decode_limits(bytes.len()))
            .map_err(|_| BridgeError::ParliamentTimedOvn)?;
    archive
        .validate_v1()
        .map_err(|_| BridgeError::ParliamentTimedOvn)
}

fn parse_wallet_authority(authority: &str) -> Result<AccountId, BridgeError> {
    AccountAddress::parse_encoded(authority, None)
        .and_then(|address| address.to_account_id())
        .map_err(|_| BridgeError::Authority)
}

pub(super) fn verified_casting_context_from_proof_v1(
    proof_response_bytes: &[u8],
    network_id: [u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1],
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: [u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1],
    expected_ballot_attempt_id: [u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1],
) -> Result<ValidatedParliamentTimedOvnCastingContextArchiveV1, BridgeError> {
    if proof_response_bytes.is_empty()
        || proof_response_bytes.len()
            > CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1
        || trusted_checkpoint_height == 0
        || network_id.iter().all(|byte| *byte == 0)
        || network_id[network_id.len() - 1] & 1 == 0
    {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let response: ParliamentTimedOvnCastingProofResponseV1 = norito::decode_canonical_with_limits(
        proof_response_bytes,
        public_proof_decode_limits(proof_response_bytes.len()),
    )
    .map_err(|_| BridgeError::ParliamentTimedOvn)?;
    // A promotion page never authorizes secret access. Callers must persist its
    // returned checkpoint and fetch a separately encoded terminal response.
    if response.more_available {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(network_id),
    ));
    let expected_ballot_attempt_id = BallotAttemptId::new(expected_ballot_attempt_id);
    let binding = response
        .verify_consensus_page_against(
            network_id,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
            expected_ballot_attempt_id,
        )
        .map_err(|_| BridgeError::ParliamentTimedOvn)?
        .ok_or(BridgeError::ParliamentTimedOvn)?;
    let archive_bytes = response
        .casting_context_archive
        .as_deref()
        .ok_or(BridgeError::ParliamentTimedOvn)?;
    let archive = decode_casting_context(archive_bytes)?;
    if !archive.matches_compact_binding_v1(binding) {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    Ok(archive)
}

pub(super) fn registration_from_verified_context_v1(
    casting_context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authority: &str,
    root_seed: &[u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1],
) -> Result<Vec<u8>, BridgeError> {
    if authority.is_empty()
        || authority.len() > AUTHORITY_UTF8_MAX_BYTES_V1
        || root_seed.iter().all(|byte| *byte == 0)
    {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let authority = parse_wallet_authority(authority)?;
    registration_record_from_seed(casting_context, &authority, root_seed)
}

pub(super) fn ballot_from_verified_context_v1(
    casting_context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authority: &str,
    root_seed: &[u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1],
    choice: u8,
) -> Result<Vec<u8>, BridgeError> {
    if authority.is_empty()
        || authority.len() > AUTHORITY_UTF8_MAX_BYTES_V1
        || root_seed.iter().all(|byte| *byte == 0)
    {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let choice = match choice {
        0 => TimedOvnChoiceV1::Aye,
        1 => TimedOvnChoiceV1::Nay,
        2 => TimedOvnChoiceV1::Abstain,
        _ => return Err(BridgeError::ParliamentTimedOvn),
    };
    let authority = parse_wallet_authority(authority)?;
    ballot_record_from_seed(casting_context, &authority, root_seed, choice)
}

unsafe fn input_bytes<'a>(
    input_ptr: *const c_uchar,
    input_len: c_ulong,
    maximum: usize,
) -> Result<&'a [u8], BridgeError> {
    if input_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let input_len = usize::try_from(input_len).map_err(|_| BridgeError::ParliamentTimedOvn)?;
    if input_len == 0 || input_len > maximum {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    Ok(unsafe { slice::from_raw_parts(input_ptr, input_len) })
}

unsafe fn trust_anchor_from_input(
    input_ptr: *const c_uchar,
    input_len: c_ulong,
) -> Result<[u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1], BridgeError> {
    let bytes = unsafe {
        input_bytes(
            input_ptr,
            input_len,
            CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1,
        )?
    };
    bytes
        .try_into()
        .map_err(|_| BridgeError::ParliamentTimedOvn)
}

unsafe fn authority_from_input(
    authority_ptr: *const c_char,
    authority_len: c_ulong,
) -> Result<AccountId, BridgeError> {
    let authority_bytes = unsafe {
        input_bytes(
            authority_ptr.cast::<c_uchar>(),
            authority_len,
            AUTHORITY_UTF8_MAX_BYTES_V1,
        )?
    };
    let authority = core::str::from_utf8(authority_bytes).map_err(|_| BridgeError::Utf8)?;
    parse_wallet_authority(authority)
}

unsafe fn seed_from_input(
    seed_ptr: *const c_uchar,
    seed_len: c_ulong,
) -> Result<Zeroizing<[u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1]>, BridgeError> {
    let seed_bytes = unsafe {
        input_bytes(
            seed_ptr,
            seed_len,
            CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1,
        )?
    };
    if seed_bytes.len() != CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    let mut seed = Zeroizing::new([0_u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1]);
    seed.copy_from_slice(seed_bytes);
    if seed.iter().all(|byte| *byte == 0) {
        return Err(BridgeError::ParliamentTimedOvn);
    }
    Ok(seed)
}

unsafe fn reset_output(
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> Result<(), BridgeError> {
    if out_ptr.is_null() || out_len.is_null() {
        return Err(BridgeError::NullPtr);
    }
    unsafe {
        *out_ptr = ptr::null_mut();
        *out_len = 0;
    }
    Ok(())
}

/// Verify one terminal consensus-authenticated casting response without reading a seed.
///
/// # Safety
///
/// Every non-null pointer must address the declared number of readable bytes
/// for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_parliament_timed_ovn_verify_casting_proof_v1(
    proof_response_norito_ptr: *const c_uchar,
    proof_response_norito_len: c_ulong,
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id_ptr: *const c_uchar,
    trusted_checkpoint_context_id_len: c_ulong,
    expected_ballot_attempt_id_ptr: *const c_uchar,
    expected_ballot_attempt_id_len: c_ulong,
) -> c_int {
    let result = (|| -> Result<(), BridgeError> {
        let proof_response = unsafe {
            input_bytes(
                proof_response_norito_ptr,
                proof_response_norito_len,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?
        };
        let network_id = unsafe { trust_anchor_from_input(network_id_ptr, network_id_len)? };
        let checkpoint_context = unsafe {
            trust_anchor_from_input(
                trusted_checkpoint_context_id_ptr,
                trusted_checkpoint_context_id_len,
            )?
        };
        let expected_ballot = unsafe {
            trust_anchor_from_input(
                expected_ballot_attempt_id_ptr,
                expected_ballot_attempt_id_len,
            )?
        };
        verified_casting_context_from_proof_v1(
            proof_response,
            network_id,
            trusted_checkpoint_height,
            checkpoint_context,
            expected_ballot,
        )?;
        Ok(())
    })();
    result.map_or_else(BridgeError::code, |()| 0)
}

/// Authenticate one terminal casting proof, then reconstruct one wallet's
/// secret and emit its canonical public registration.
///
/// The proof is canonical-decoded, finality/witness/membership verified, and
/// its Core archive replayed and rebound before `keystore_seed` is read. The
/// output is public and must be released with `connect_norito_free`.
///
/// # Safety
///
/// Every non-null pointer must address the declared number of readable or
/// writable bytes for the duration of this call. The two output slots must be
/// distinct and must not overlap any input storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_parliament_timed_ovn_registration_from_proof_v1(
    proof_response_norito_ptr: *const c_uchar,
    proof_response_norito_len: c_ulong,
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id_ptr: *const c_uchar,
    trusted_checkpoint_context_id_len: c_ulong,
    expected_ballot_attempt_id_ptr: *const c_uchar,
    expected_ballot_attempt_id_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    keystore_seed_ptr: *const c_uchar,
    keystore_seed_len: c_ulong,
    out_registration_ptr: *mut *mut c_uchar,
    out_registration_len: *mut c_ulong,
) -> c_int {
    let result = (|| -> Result<(), BridgeError> {
        unsafe { reset_output(out_registration_ptr, out_registration_len)? };
        let proof_response = unsafe {
            input_bytes(
                proof_response_norito_ptr,
                proof_response_norito_len,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?
        };
        let network_id = unsafe { trust_anchor_from_input(network_id_ptr, network_id_len)? };
        let checkpoint_context = unsafe {
            trust_anchor_from_input(
                trusted_checkpoint_context_id_ptr,
                trusted_checkpoint_context_id_len,
            )?
        };
        let expected_ballot = unsafe {
            trust_anchor_from_input(
                expected_ballot_attempt_id_ptr,
                expected_ballot_attempt_id_len,
            )?
        };
        let casting_context = verified_casting_context_from_proof_v1(
            proof_response,
            network_id,
            trusted_checkpoint_height,
            checkpoint_context,
            expected_ballot,
        )?;
        let authority = unsafe { authority_from_input(authority_ptr, authority_len)? };
        // Seed access is intentionally last, after every public proof and archive gate.
        let root_seed = unsafe { seed_from_input(keystore_seed_ptr, keystore_seed_len)? };
        let registration = registration_record_from_seed(&casting_context, &authority, &root_seed)?;
        unsafe { write_bytes_bridge(out_registration_ptr, out_registration_len, &registration) }
    })();
    result.map_or_else(BridgeError::code, |()| 0)
}

/// Authenticate one terminal casting proof, then reconstruct one wallet's
/// registered secret and emit a canonical masked ballot.
///
/// The context must be exactly `SurvivorsFrozen`; full public evidence and its
/// TLE transcript are replay-validated before any proof is produced. The
/// regenerated registration must equal the committed record byte-for-byte.
/// `choice` is `0` (Aye), `1` (Nay), or `2` (Abstain). The public output must
/// be released with `connect_norito_free`.
///
/// # Safety
///
/// Every non-null pointer must address the declared number of readable or
/// writable bytes for the duration of this call. The two output slots must be
/// distinct and must not overlap any input storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_parliament_timed_ovn_ballot_from_proof_v1(
    proof_response_norito_ptr: *const c_uchar,
    proof_response_norito_len: c_ulong,
    network_id_ptr: *const c_uchar,
    network_id_len: c_ulong,
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id_ptr: *const c_uchar,
    trusted_checkpoint_context_id_len: c_ulong,
    expected_ballot_attempt_id_ptr: *const c_uchar,
    expected_ballot_attempt_id_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    keystore_seed_ptr: *const c_uchar,
    keystore_seed_len: c_ulong,
    choice: u8,
    out_ballot_ptr: *mut *mut c_uchar,
    out_ballot_len: *mut c_ulong,
) -> c_int {
    let result = (|| -> Result<(), BridgeError> {
        unsafe { reset_output(out_ballot_ptr, out_ballot_len)? };
        let choice = match choice {
            0 => TimedOvnChoiceV1::Aye,
            1 => TimedOvnChoiceV1::Nay,
            2 => TimedOvnChoiceV1::Abstain,
            _ => return Err(BridgeError::ParliamentTimedOvn),
        };
        let proof_response = unsafe {
            input_bytes(
                proof_response_norito_ptr,
                proof_response_norito_len,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?
        };
        let network_id = unsafe { trust_anchor_from_input(network_id_ptr, network_id_len)? };
        let checkpoint_context = unsafe {
            trust_anchor_from_input(
                trusted_checkpoint_context_id_ptr,
                trusted_checkpoint_context_id_len,
            )?
        };
        let expected_ballot = unsafe {
            trust_anchor_from_input(
                expected_ballot_attempt_id_ptr,
                expected_ballot_attempt_id_len,
            )?
        };
        let casting_context = verified_casting_context_from_proof_v1(
            proof_response,
            network_id,
            trusted_checkpoint_height,
            checkpoint_context,
            expected_ballot,
        )?;
        let authority = unsafe { authority_from_input(authority_ptr, authority_len)? };
        // Seed access is intentionally last, after every public proof and archive gate.
        let root_seed = unsafe { seed_from_input(keystore_seed_ptr, keystore_seed_len)? };
        let ballot = ballot_record_from_seed(&casting_context, &authority, &root_seed, choice)?;
        unsafe { write_bytes_bridge(out_ballot_ptr, out_ballot_len, &ballot) }
    })();
    result.map_or_else(BridgeError::code, |()| 0)
}

#[cfg(test)]
mod tests {
    use iroha_core::{
        governance::timed_ovn::{
            TimedOvnLifecycleStateV1, TimedOvnSessionPublicV1, timed_ovn_parameter_hash_v1,
        },
        tle_release::{
            ParliamentTimedOvnCastingContextArchiveV1, ParliamentTimedOvnCastingPhaseV1,
            ValidatedTleKeySessionV1,
        },
    };
    use iroha_crypto::{
        Algorithm, Hash, HashOf, KeyPair, MerkleTree, Signature,
        threshold_bls::{
            AdaptiveThresholdBlsParameters, DasRenDealerSecret, ThresholdBlsSession,
            TleReleasePurpose,
        },
    };
    use iroha_data_model::{
        block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding, QuorumCertificate,
            ValidatorPower, finality::V2FinalityArtifact,
        },
        bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
        parliament_casting::{
            PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1,
            PARLIAMENT_TIMED_OVN_CASTING_WITNESS_SIBLINGS_V1,
            ParliamentTimedOvnCastingContextMembershipProofV1,
            ParliamentTimedOvnCastingSnapshotCommitmentV1, ParliamentTimedOvnCastingWitnessProofV1,
        },
        peer::PeerId,
    };
    use iroha_torii_shared::parliament_api::{
        PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1, ParliamentTimedOvnCastingProofResponseV1,
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use std::num::NonZeroU64;

    use super::*;

    fn binding(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn tle_fixture() -> ValidatedTleKeySessionV1 {
        let session =
            ThresholdBlsSession::<TleReleasePurpose>::new(binding(1), binding(2), binding(3), 4, 2)
                .expect("threshold session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut rng = StdRng::from_seed([31; 32]);
        let dealers = (1_u16..=3)
            .map(|index| {
                DasRenDealerSecret::generate_with_rng(&parameters, index, &mut rng)
                    .expect("dealer")
                    .1
            })
            .collect::<Vec<_>>();
        ValidatedTleKeySessionV1::from_qualified_dealers(session, &dealers, &[1, 2, 3], binding(4))
            .expect("TLE key session")
    }

    fn account(seed: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("account fixture key");
        AccountId::new(key.public_key().clone())
    }

    #[test]
    fn wallet_authority_uses_the_canonical_embedded_i105_discriminant() {
        let authority = account(50);
        let process_discriminant = iroha_data_model::account::address::chain_discriminant();
        let embedded_discriminant = if process_discriminant == 42 { 43 } else { 42 };
        let literal = authority
            .to_i105_for_discriminant(embedded_discriminant)
            .expect("foreign-discriminant canonical i105");

        assert_eq!(
            parse_wallet_authority(&literal).expect("embedded discriminant must be authoritative"),
            authority
        );

        let mut tampered = literal.into_bytes();
        let last = tampered.last_mut().expect("non-empty i105 literal");
        *last = if *last == b'1' { b'2' } else { b'1' };
        let tampered = String::from_utf8(tampered).expect("ASCII i105 literal");
        assert!(parse_wallet_authority(&tampered).is_err());
    }

    fn open_lifecycle(tle: &ValidatedTleKeySessionV1) -> TimedOvnLifecycleStateV1 {
        let session = TimedOvnSessionPublicV1 {
            network_id: binding(1),
            proposal_content_id: binding(10),
            governance_attempt_id: binding(11),
            body_instance_id: binding(12),
            ballot_attempt_id: binding(13),
            parameter_hash: timed_ovn_parameter_hash_v1(),
            tle_key_session_id: tle.public_state().key_session_id,
            tle_key_transcript_hash: tle.public_state().transcript_hash,
            tle_master_public_key: *tle.master_public_key().as_bytes(),
        };
        TimedOvnLifecycleStateV1::open_registration(session, 20, 40, tle)
            .expect("open registration")
    }

    fn casting_context(
        lifecycle: &TimedOvnLifecycleStateV1,
        tle: &ValidatedTleKeySessionV1,
    ) -> ValidatedParliamentTimedOvnCastingContextArchiveV1 {
        let (finalized_height, phase, survivors, release_identity) = match lifecycle {
            TimedOvnLifecycleStateV1::Registered(_) => {
                (25, ParliamentTimedOvnCastingPhaseV1::Registered, None, None)
            }
            TimedOvnLifecycleStateV1::RegistrationClosed(_) => (
                32,
                ParliamentTimedOvnCastingPhaseV1::RegistrationClosed,
                None,
                None,
            ),
            TimedOvnLifecycleStateV1::SurvivorsFrozen(frozen) => (
                36,
                ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen,
                Some(frozen.survivor_participant_hashes().to_vec()),
                Some(*frozen.release_identity()),
            ),
            TimedOvnLifecycleStateV1::Sealed(_) | TimedOvnLifecycleStateV1::Released(_) => {
                panic!("test context must remain cast-capable")
            }
        };
        ParliamentTimedOvnCastingContextArchiveV1::try_from_parts_v1(
            finalized_height,
            phase,
            *lifecycle.session(),
            lifecycle
                .registration_opened_at_finalized_height()
                .expect("cast-capable registration-open height"),
            lifecycle.target_finalized_height(),
            tle.public_state().clone(),
            lifecycle.registration_records().to_vec(),
            survivors,
            release_identity,
        )
        .expect("casting context archive")
        .validate_v1()
        .expect("validated casting context")
    }

    const REGISTRATION_CLOSE_HEIGHT: u64 = 30;
    const SURVIVOR_FREEZE_HEIGHT: u64 = 35;
    const COMMITMENT_CLOSE_HEIGHT: u64 = 39;

    #[derive(Clone)]
    struct CastingProofFixture {
        response: ParliamentTimedOvnCastingProofResponseV1,
        network_id: [u8; 32],
        checkpoint_height: u64,
        checkpoint_context_id: [u8; 32],
        ballot_attempt_id: [u8; 32],
    }

    impl CastingProofFixture {
        fn canonical_bytes(&self) -> Vec<u8> {
            norito::to_bytes(&self.response).expect("canonical casting proof response")
        }
    }

    fn witness_and_root(
        snapshot: &ParliamentTimedOvnCastingSnapshotCommitmentV1,
    ) -> (ParliamentTimedOvnCastingWitnessProofV1, Hash) {
        let value = norito::to_bytes(snapshot).expect("canonical casting snapshot");
        let witness = ParliamentTimedOvnCastingWitnessProofV1 {
            key: PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1.to_vec(),
            value,
            siblings: vec![Hash::new([]); PARLIAMENT_TIMED_OVN_CASTING_WITNESS_SIBLINGS_V1],
        };
        let path = Hash::new(&witness.key);
        let value_hash = Hash::new(&witness.value);
        let mut leaf = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf.push(0);
        leaf.extend_from_slice(path.as_ref());
        leaf.extend_from_slice(value_hash.as_ref());
        let mut root = Hash::new(leaf);
        for (level, sibling) in witness.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let right = path.as_ref()[path_bit / 8] & (1_u8 << (path_bit % 8)) != 0;
            let (left, right) = if right {
                (sibling, root)
            } else {
                (root, sibling)
            };
            let mut node = Vec::with_capacity(1 + 2 * Hash::LENGTH);
            node.push(1);
            node.extend_from_slice(left.as_ref());
            node.extend_from_slice(right.as_ref());
            root = Hash::new(node);
        }
        assert!(witness.verify(root));
        (witness, root)
    }

    fn finality_chain(
        network_id: NetworkId,
        tip_height: u64,
        tip_ordinary_writes_root: Hash,
    ) -> Vec<BridgeFinalityProof> {
        let mut keys = (0_u8..4)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![0xD0_u8.saturating_add(index); 32],
                    Algorithm::BlsNormal,
                )
                .expect("derive deterministic Parliament validator")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let quorum = DualQuorum::from_roster(&roster).expect("valid Parliament validator roster");
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive Parliament validator PoP")
            })
            .collect::<Vec<_>>();
        let mut proofs = Vec::with_capacity(usize::try_from(tip_height).expect("small test tip"));
        for height in 1..=tip_height {
            let parent = proofs
                .last()
                .map(|proof: &BridgeFinalityProof| &proof.finality_artifact);
            let mut timestamp = 1_900_000_000_000_u64 + height;
            let header = loop {
                let candidate = BlockHeader::new(
                    NonZeroU64::new(height).expect("nonzero Parliament proof height"),
                    parent.map(|artifact| artifact.block_hash),
                    None,
                    None,
                    timestamp,
                    0,
                );
                if height != tip_height || candidate.hash().as_ref()[31] & 1 == 1 {
                    break candidate;
                }
                timestamp = timestamp.checked_add(1).expect("test timestamp space");
            };
            let mut context = HeightContext {
                network_id,
                protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
                height,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: ConsensusMode::Npos,
                parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                quorum,
                roster: roster.clone(),
                nexus_amx_context_hash: Hash::new(b"Parliament casting proof test nexus"),
                execution_policy_hash: Hash::new(b"Parliament casting proof test policy"),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1_024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4_096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x5A; 32],
            };
            if height == 1 {
                while context.id().0.as_ref()[31] & 1 == 0 {
                    context.leader_seed[0] = context.leader_seed[0]
                        .checked_add(1)
                        .expect("canonical checkpoint fixture search");
                }
            }
            let subject = BlockSubject {
                parent_block_hash: header.prev_block_hash(),
                block_hash: header.hash(),
                payload_hash: Hash::new(b"Parliament casting proof test payload"),
            };
            let round = ConsensusRound {
                context_id: context.id(),
                height,
                view: 0,
            };
            let ordinary_writes_root = if height == tip_height {
                tip_ordinary_writes_root
            } else {
                Hash::new(height.to_be_bytes())
            };
            let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"Parliament casting parent state"),
                Hash::new(b"Parliament casting post state"),
                ordinary_writes_root,
                1,
                Hash::new(b"Parliament casting executed wire"),
            );
            let mut commit_qc = QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            };
            let preimage = commit_qc
                .signer_preimage(&context, 0)
                .expect("valid Parliament finality signer preimage");
            let signatures = commit_qc
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("validator index")].private_key(),
                        &preimage,
                    )
                    .expect("sign Parliament finality vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate Parliament finality votes");
            let artifact = V2FinalityArtifact::new(context, subject, commit_qc, pops.clone());
            artifact.verify().expect("Parliament finality artifact");
            proofs.push(BridgeFinalityProof {
                version: BRIDGE_FINALITY_PROOF_VERSION_V2,
                block_header: header,
                finality_artifact: artifact,
            });
        }
        proofs
    }

    fn casting_proof_fixture(
        context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
    ) -> CastingProofFixture {
        let binding = context
            .compact_binding_v1(
                REGISTRATION_CLOSE_HEIGHT,
                SURVIVOR_FREEZE_HEIGHT,
                COMMITMENT_CLOSE_HEIGHT,
            )
            .expect("compact casting binding");
        let snapshot = ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(
            binding.evaluated_height,
            std::slice::from_ref(&binding),
        )
        .expect("casting snapshot");
        let tree = MerkleTree::from_iter([HashOf::new(&binding)]);
        let membership = ParliamentTimedOvnCastingContextMembershipProofV1::new(
            tree.get_proof(0).expect("single casting leaf proof"),
        );
        let (witness, ordinary_writes_root) = witness_and_root(&snapshot);
        let network_id_bytes = binding.network_id;
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(network_id_bytes)),
        );
        let finality_chain =
            finality_chain(network_id, binding.evaluated_height, ordinary_writes_root);
        let first = finality_chain.first().expect("checkpoint proof");
        let tip = finality_chain.last().expect("evaluated proof");
        let checkpoint_context_id = *first.finality_artifact.context_id().0.as_ref();
        let evaluated_context_id = tip.finality_artifact.context_id();
        let evaluated_block_hash = hex::encode(tip.finality_artifact.block_hash.as_ref());
        let response = ParliamentTimedOvnCastingProofResponseV1 {
            version: PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1,
            casting_context_archive: Some(
                context
                    .archive()
                    .to_canonical_bytes_v1()
                    .expect("canonical casting archive"),
            ),
            casting_context_binding: Some(binding.clone()),
            context_membership_proof: Some(membership),
            casting_witness: Some(witness),
            finality_chain,
            evaluated_context_id,
            evaluated_block_height: binding.evaluated_height,
            evaluated_block_hash,
            observed_ledger_tip_height: binding.evaluated_height,
            more_available: false,
        };
        CastingProofFixture {
            response,
            network_id: network_id_bytes,
            checkpoint_height: 1,
            checkpoint_context_id,
            ballot_attempt_id: *binding.ballot_attempt_id.as_bytes(),
        }
    }

    fn call_registration_ffi(
        context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
        authority: &AccountId,
        seed: &[u8; 32],
    ) -> Result<Vec<u8>, c_int> {
        let fixture = casting_proof_fixture(context);
        call_registration_ffi_bytes(&fixture.canonical_bytes(), &fixture, authority, seed)
    }

    fn call_registration_ffi_bytes(
        proof_response: &[u8],
        anchor: &CastingProofFixture,
        authority: &AccountId,
        seed: &[u8; 32],
    ) -> Result<Vec<u8>, c_int> {
        let authority = authority.to_string();
        let mut output = ptr::null_mut();
        let mut output_len = 1;
        let status = unsafe {
            connect_norito_parliament_timed_ovn_registration_from_proof_v1(
                proof_response.as_ptr(),
                proof_response.len() as c_ulong,
                anchor.network_id.as_ptr(),
                anchor.network_id.len() as c_ulong,
                anchor.checkpoint_height,
                anchor.checkpoint_context_id.as_ptr(),
                anchor.checkpoint_context_id.len() as c_ulong,
                anchor.ballot_attempt_id.as_ptr(),
                anchor.ballot_attempt_id.len() as c_ulong,
                authority.as_ptr().cast::<c_char>(),
                authority.len() as c_ulong,
                seed.as_ptr(),
                seed.len() as c_ulong,
                &mut output,
                &mut output_len,
            )
        };
        if status != 0 {
            assert!(output.is_null());
            assert_eq!(output_len, 0);
            return Err(status);
        }
        assert!(!output.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(output, output_len as usize) }.to_vec();
        crate::connect_norito_free(output);
        Ok(bytes)
    }

    fn call_ballot_ffi(
        context: &ValidatedParliamentTimedOvnCastingContextArchiveV1,
        authority: &AccountId,
        seed: &[u8; 32],
        choice: u8,
    ) -> Result<Vec<u8>, c_int> {
        let fixture = casting_proof_fixture(context);
        let proof_response = fixture.canonical_bytes();
        let authority = authority.to_string();
        let mut output = ptr::null_mut();
        let mut output_len = 1;
        let status = unsafe {
            connect_norito_parliament_timed_ovn_ballot_from_proof_v1(
                proof_response.as_ptr(),
                proof_response.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.checkpoint_height,
                fixture.checkpoint_context_id.as_ptr(),
                fixture.checkpoint_context_id.len() as c_ulong,
                fixture.ballot_attempt_id.as_ptr(),
                fixture.ballot_attempt_id.len() as c_ulong,
                authority.as_ptr().cast::<c_char>(),
                authority.len() as c_ulong,
                seed.as_ptr(),
                seed.len() as c_ulong,
                choice,
                &mut output,
                &mut output_len,
            )
        };
        if status != 0 {
            assert!(output.is_null());
            assert_eq!(output_len, 0);
            return Err(status);
        }
        assert!(!output.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(output, output_len as usize) }.to_vec();
        crate::connect_norito_free(output);
        Ok(bytes)
    }

    #[test]
    fn keystore_seed_reconstructs_registration_and_survivor_bound_ballot() {
        let tle = tle_fixture();
        let mut lifecycle = open_lifecycle(&tle);
        let mut voters = [
            (account(51), [61_u8; 32]),
            (account(52), [62_u8; 32]),
            (account(53), [63_u8; 32]),
        ]
        .into_iter()
        .map(|(authority, seed)| {
            let hash = participant_hash(&casting_context(&lifecycle, &tle), &authority);
            (hash, authority, seed)
        })
        .collect::<Vec<_>>();
        voters.sort_by_key(|(hash, _, _)| *hash);

        let first_context = casting_context(&lifecycle, &tle);
        let ffi_registration = call_registration_ffi(&first_context, &voters[0].1, &voters[0].2)
            .expect("native registration");
        assert_eq!(
            ffi_registration.len(),
            TIMED_OVN_REGISTRATION_RECORD_BYTES_V1
        );
        assert_eq!(
            ffi_registration,
            registration_record_from_seed(&first_context, &voters[0].1, &voters[0].2)
                .expect("direct registration")
        );

        for (hash, authority, seed) in &voters {
            let context = casting_context(&lifecycle, &tle);
            let first =
                registration_record_from_seed(&context, authority, seed).expect("registration");
            let repeated = registration_record_from_seed(&context, authority, seed)
                .expect("deterministic registration");
            assert_eq!(first, repeated);
            lifecycle = lifecycle
                .register_participant(*hash, first, &tle)
                .expect("register participant");
        }
        assert!(
            registration_record_from_seed(
                &casting_context(&lifecycle, &tle),
                &voters[0].1,
                &[99; 32],
            )
            .is_err()
        );
        lifecycle = lifecycle
            .close_registration(&tle)
            .expect("close registration")
            .freeze_survivors(&tle)
            .expect("freeze survivors");
        let frozen_context = casting_context(&lifecycle, &tle);

        let choices = [
            TimedOvnChoiceV1::Aye,
            TimedOvnChoiceV1::Nay,
            TimedOvnChoiceV1::Abstain,
        ];
        let ballot_records = voters
            .iter()
            .zip(choices)
            .map(|((_, authority, seed), choice)| {
                let first = ballot_record_from_seed(&frozen_context, authority, seed, choice)
                    .expect("masked ballot");
                let repeated = ballot_record_from_seed(&frozen_context, authority, seed, choice)
                    .expect("deterministic masked ballot");
                assert_eq!(first, repeated);
                first
            })
            .collect::<Vec<_>>();
        let ffi_ballot = call_ballot_ffi(&frozen_context, &voters[0].1, &voters[0].2, 0)
            .expect("native masked ballot");
        assert_eq!(ffi_ballot.len(), TIMED_OVN_BALLOT_RECORD_BYTES_V1);
        assert_eq!(ffi_ballot, ballot_records[0]);
        assert_eq!(
            call_ballot_ffi(&frozen_context, &voters[0].1, &[99; 32], 0),
            Err(BridgeError::ParliamentTimedOvn.code())
        );
        let TimedOvnLifecycleStateV1::SurvivorsFrozen(frozen) = &lifecycle else {
            panic!("expected frozen survivor state")
        };
        frozen
            .validate(&tle)
            .expect("validated frozen state")
            .admit_ballot_corpus(&ballot_records)
            .expect("admit complete public ballot corpus");
    }

    #[test]
    fn phase_seed_and_choice_boundaries_fail_closed() {
        let tle = tle_fixture();
        let lifecycle = open_lifecycle(&tle);
        let authority = account(71);
        let seed = [72_u8; 32];
        let registered_context = casting_context(&lifecycle, &tle);
        assert!(
            ballot_record_from_seed(
                &registered_context,
                &authority,
                &seed,
                TimedOvnChoiceV1::Aye,
            )
            .is_err()
        );
        assert_eq!(
            call_ballot_ffi(&registered_context, &authority, &seed, 0),
            Err(BridgeError::ParliamentTimedOvn.code())
        );
        assert_eq!(
            call_ballot_ffi(&registered_context, &authority, &seed, 3),
            Err(BridgeError::ParliamentTimedOvn.code())
        );

        let closed_context = casting_context(
            &lifecycle
                .clone()
                .close_registration(&tle)
                .expect("close registration"),
            &tle,
        );
        assert_eq!(
            call_registration_ffi(&closed_context, &authority, &seed),
            Err(BridgeError::ParliamentTimedOvn.code())
        );
        assert_eq!(
            call_ballot_ffi(&closed_context, &authority, &seed, 0),
            Err(BridgeError::ParliamentTimedOvn.code())
        );

        assert_eq!(
            call_registration_ffi(&registered_context, &authority, &[0_u8; 32]),
            Err(BridgeError::ParliamentTimedOvn.code())
        );
    }

    #[test]
    fn casting_proof_rejects_fake_chain_wrong_anchors_intermediate_and_archive_tampering() {
        let tle = tle_fixture();
        let lifecycle = open_lifecycle(&tle);
        let context = casting_context(&lifecycle, &tle);
        let fixture = casting_proof_fixture(&context);
        let verify = |bytes: &[u8], network, checkpoint_context, ballot| {
            verified_casting_context_from_proof_v1(
                bytes,
                network,
                fixture.checkpoint_height,
                checkpoint_context,
                ballot,
            )
        };

        assert!(
            verify(
                &fixture.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_ok()
        );

        assert!(
            verify(
                &fixture.canonical_bytes(),
                [0_u8; 32],
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );
        let mut normalized_network_alias = fixture.network_id;
        normalized_network_alias[31] &= !1;
        assert_ne!(normalized_network_alias, fixture.network_id);
        assert!(
            verify(
                &fixture.canonical_bytes(),
                normalized_network_alias,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut malformed = fixture.canonical_bytes();
        malformed.push(0);
        assert!(
            verify(
                &malformed,
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut fake_chain = fixture.clone();
        fake_chain
            .response
            .finality_chain
            .last_mut()
            .expect("finality tip")
            .finality_artifact
            .commit_qc
            .aggregate_signature[0] ^= 0x80;
        assert!(
            verify(
                &fake_chain.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut wrong_network = fixture.network_id;
        wrong_network[0] ^= 0x40;
        assert!(
            verify(
                &fixture.canonical_bytes(),
                wrong_network,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );
        let mut wrong_context = fixture.checkpoint_context_id;
        wrong_context[0] ^= 0x40;
        assert!(
            verify(
                &fixture.canonical_bytes(),
                fixture.network_id,
                wrong_context,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );
        let mut wrong_ballot = fixture.ballot_attempt_id;
        wrong_ballot[0] ^= 0x40;
        assert!(
            verify(
                &fixture.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                wrong_ballot,
            )
            .is_err()
        );

        let mut intermediate = fixture.clone();
        intermediate.response.more_available = true;
        intermediate.response.observed_ledger_tip_height = intermediate
            .response
            .evaluated_block_height
            .checked_add(1)
            .expect("test tip height");
        intermediate.response.casting_context_archive = None;
        intermediate.response.casting_context_binding = None;
        intermediate.response.context_membership_proof = None;
        intermediate.response.casting_witness = None;
        assert!(
            verify(
                &intermediate.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut binding_tampering = fixture.clone();
        binding_tampering
            .response
            .casting_context_binding
            .as_mut()
            .expect("terminal casting binding")
            .parameter_hash[0] ^= 0x40;
        assert!(
            verify(
                &binding_tampering.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut membership_tampering = fixture.clone();
        let bound_context = membership_tampering
            .response
            .casting_context_binding
            .as_ref()
            .expect("terminal casting binding");
        let alternate_tree =
            MerkleTree::from_iter([HashOf::new(bound_context), HashOf::new(bound_context)]);
        membership_tampering.response.context_membership_proof =
            Some(ParliamentTimedOvnCastingContextMembershipProofV1::new(
                alternate_tree
                    .get_proof(0)
                    .expect("two-leaf alternate membership proof"),
            ));
        assert!(
            verify(
                &membership_tampering.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let mut witness_tampering = fixture.clone();
        witness_tampering
            .response
            .casting_witness
            .as_mut()
            .expect("terminal casting witness")
            .siblings[0] = Hash::new(b"tampered Parliament casting witness sibling");
        assert!(
            verify(
                &witness_tampering.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );

        let alternate_authority = account(75);
        let alternate_seed = [76_u8; 32];
        let alternate_hash = participant_hash(&context, &alternate_authority);
        let alternate_record =
            registration_record_from_seed(&context, &alternate_authority, &alternate_seed)
                .expect("alternate registration");
        let alternate_lifecycle = lifecycle
            .register_participant(alternate_hash, alternate_record, &tle)
            .expect("alternate registered lifecycle");
        let alternate_context = casting_context(&alternate_lifecycle, &tle);
        let mut archive_substitution = fixture.clone();
        archive_substitution.response.casting_context_archive = Some(
            alternate_context
                .archive()
                .to_canonical_bytes_v1()
                .expect("alternate canonical archive"),
        );
        assert!(
            verify(
                &archive_substitution.canonical_bytes(),
                fixture.network_id,
                fixture.checkpoint_context_id,
                fixture.ballot_attempt_id,
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_proof_is_rejected_before_either_seed_pointer_is_read() {
        let tle = tle_fixture();
        let lifecycle = open_lifecycle(&tle);
        let context = casting_context(&lifecycle, &tle);
        let fixture = casting_proof_fixture(&context);
        let mut malformed = fixture.canonical_bytes();
        malformed.push(0);
        let authority = account(77).to_string();
        let mut output = ptr::null_mut();
        let mut output_len = 99;
        let status = unsafe {
            connect_norito_parliament_timed_ovn_registration_from_proof_v1(
                malformed.as_ptr(),
                malformed.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.checkpoint_height,
                fixture.checkpoint_context_id.as_ptr(),
                fixture.checkpoint_context_id.len() as c_ulong,
                fixture.ballot_attempt_id.as_ptr(),
                fixture.ballot_attempt_id.len() as c_ulong,
                authority.as_ptr().cast::<c_char>(),
                authority.len() as c_ulong,
                ptr::null(),
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 as c_ulong,
                &mut output,
                &mut output_len,
            )
        };
        assert_eq!(status, BridgeError::ParliamentTimedOvn.code());
        assert!(output.is_null());
        assert_eq!(output_len, 0);

        output = ptr::null_mut();
        output_len = 99;
        let status = unsafe {
            connect_norito_parliament_timed_ovn_ballot_from_proof_v1(
                malformed.as_ptr(),
                malformed.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.checkpoint_height,
                fixture.checkpoint_context_id.as_ptr(),
                fixture.checkpoint_context_id.len() as c_ulong,
                fixture.ballot_attempt_id.as_ptr(),
                fixture.ballot_attempt_id.len() as c_ulong,
                authority.as_ptr().cast::<c_char>(),
                authority.len() as c_ulong,
                ptr::null(),
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 as c_ulong,
                0,
                &mut output,
                &mut output_len,
            )
        };
        assert_eq!(status, BridgeError::ParliamentTimedOvn.code());
        assert!(output.is_null());
        assert_eq!(output_len, 0);
    }

    #[test]
    fn malformed_proof_and_pointer_contract_fail_closed() {
        let tle = tle_fixture();
        let lifecycle = open_lifecycle(&tle);
        let authority = account(73);
        let seed = [74_u8; 32];
        let context = casting_context(&lifecycle, &tle);
        let fixture = casting_proof_fixture(&context);
        let canonical = fixture.canonical_bytes();

        let mut tampered = canonical.clone();
        let last = tampered.last_mut().expect("non-empty archive");
        *last ^= 0x80;
        assert_eq!(
            call_registration_ffi_bytes(&tampered, &fixture, &authority, &seed),
            Err(BridgeError::ParliamentTimedOvn.code())
        );

        let mut noncanonical = canonical.clone();
        noncanonical.push(0);
        assert_eq!(
            call_registration_ffi_bytes(&noncanonical, &fixture, &authority, &seed),
            Err(BridgeError::ParliamentTimedOvn.code())
        );

        let authority_string = authority.to_string();
        let mut output = ptr::dangling_mut::<c_uchar>();
        let mut output_len = 7;
        let status = unsafe {
            connect_norito_parliament_timed_ovn_registration_from_proof_v1(
                ptr::null(),
                canonical.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.checkpoint_height,
                fixture.checkpoint_context_id.as_ptr(),
                fixture.checkpoint_context_id.len() as c_ulong,
                fixture.ballot_attempt_id.as_ptr(),
                fixture.ballot_attempt_id.len() as c_ulong,
                authority_string.as_ptr().cast::<c_char>(),
                authority_string.len() as c_ulong,
                seed.as_ptr(),
                seed.len() as c_ulong,
                &mut output,
                &mut output_len,
            )
        };
        assert_eq!(status, BridgeError::NullPtr.code());
        assert!(output.is_null());
        assert_eq!(output_len, 0);

        let status = unsafe {
            connect_norito_parliament_timed_ovn_registration_from_proof_v1(
                canonical.as_ptr(),
                canonical.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.checkpoint_height,
                fixture.checkpoint_context_id.as_ptr(),
                fixture.checkpoint_context_id.len() as c_ulong,
                fixture.ballot_attempt_id.as_ptr(),
                fixture.ballot_attempt_id.len() as c_ulong,
                authority_string.as_ptr().cast::<c_char>(),
                authority_string.len() as c_ulong,
                seed.as_ptr(),
                seed.len() as c_ulong,
                ptr::null_mut(),
                &mut output_len,
            )
        };
        assert_eq!(status, BridgeError::NullPtr.code());
    }

    #[test]
    fn keyed_rng_is_deterministic_domain_separated_and_nonzero() {
        let seed = [81_u8; 32];
        let mut first = registration_rng(&seed, &binding(82), &binding(83));
        let mut repeated = registration_rng(&seed, &binding(82), &binding(83));
        let mut different = registration_rng(&seed, &binding(82), &binding(84));
        let mut different_session = registration_rng(&seed, &binding(85), &binding(83));
        let mut first_bytes = [0_u8; 96];
        let mut repeated_bytes = [0_u8; 96];
        let mut different_bytes = [0_u8; 96];
        let mut different_session_bytes = [0_u8; 96];
        first.try_fill_bytes(&mut first_bytes).expect("PRF stream");
        repeated
            .try_fill_bytes(&mut repeated_bytes)
            .expect("repeated PRF stream");
        different
            .try_fill_bytes(&mut different_bytes)
            .expect("separate PRF stream");
        different_session
            .try_fill_bytes(&mut different_session_bytes)
            .expect("separate session PRF stream");
        assert_eq!(first_bytes, repeated_bytes);
        assert_ne!(first_bytes, different_bytes);
        assert_ne!(first_bytes, different_session_bytes);
        assert!(first_bytes.iter().any(|byte| *byte != 0));

        let mut aye = ballot_rng(
            &seed,
            &binding(82),
            &binding(83),
            &binding(86),
            &binding(87),
            TimedOvnChoiceV1::Aye,
        );
        let mut nay = ballot_rng(
            &seed,
            &binding(82),
            &binding(83),
            &binding(86),
            &binding(87),
            TimedOvnChoiceV1::Nay,
        );
        let mut retry = ballot_rng(
            &seed,
            &binding(82),
            &binding(83),
            &binding(88),
            &binding(89),
            TimedOvnChoiceV1::Aye,
        );
        let mut aye_bytes = [0_u8; 64];
        let mut nay_bytes = [0_u8; 64];
        let mut retry_bytes = [0_u8; 64];
        aye.try_fill_bytes(&mut aye_bytes).expect("Aye PRF stream");
        nay.try_fill_bytes(&mut nay_bytes).expect("Nay PRF stream");
        retry
            .try_fill_bytes(&mut retry_bytes)
            .expect("retry PRF stream");
        assert_ne!(aye_bytes, nay_bytes);
        assert_ne!(aye_bytes, retry_bytes);
    }
}
