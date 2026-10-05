//! Bounded programmed-memory initializers.
//!
//! The fixed-width reduction has at most `2^-247` bias per lane for an ideal
//! uniform stream (RFC 9380 section 5's reduction bound). The actual stream is a
//! keyed BLAKE3 XOF: security additionally depends on the primitive and the secret's
//! entropy. This is not an implementation of RFC 9380 `hash_to_field`.
//!
//! [`derive_lanes_v1`] is the canonical V1 initializer. It depends on the
//! program key, the stable function identity and the associated-data digest,
//! and on no policy or encryption key. [`derive_residues`] is the diagnostic
//! BFV backend's initializer, which binds the superseded policy hash.

use super::{Hash, RAM_LFE_SECRET_MAX_BYTES, RamLfeError, validate_secret};
use norito::codec::Encode;
use subtle::{Choice, ConditionallySelectable};
use zeroize::Zeroizing;

pub(super) const LANES: usize = 32;
const BYTES_PER_LANE: usize = 32;
const CONTEXT: &str = "iroha.ram_lfe.bfv_program.initial_state.v1";
pub(super) const CONTEXT_V1: &str = "iroha.ram_lfe.v1.initial_state";

/// Maximum associated-data length for programmed RAM-LFE evaluation.
pub const RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES: usize = 512;

/// Exact first-release initializer and secret-commitment contract.
pub const BFV_PROGRAM_INITIALIZER_DESCRIPTOR: &[u8] = b"iroha.ram_lfe.bfv_program.initializer.v1;policy-secret=blake3-derive-key(context=iroha.ram_lfe.policy_secret.v1,schema=iroha_crypto::ram_lfe::PolicySecretInputV1,canonical-norito-v1-flags2(backend,secret),raw32);hidden-program=Hash(iroha.ram_lfe.bfv_program.digest.v1||blake3-derive-key(context=iroha.ram_lfe.bfv_program.secret_tape.v1,canonical-norito-v1-flags2(schema=iroha_crypto::ram_lfe::HiddenRamFheProgramV1,fields=version,register_count,memory_lane_count,tape,slots=1..256*48bytes,words=6*u64le,opcodes=0..10,unused-words=zero),raw32));initializer=blake3-derive-key-xof;context=iroha.ram_lfe.bfv_program.initial_state.v1;frame=canonical-norito-v1-flags2;schema=iroha_crypto::ram_lfe::ProgramInitializationInputV1;fields=initializer_descriptor_hash,policy_hash,secret,associated_data;secret-bytes=1..4096;associated-data-bytes=0..512;stream-bytes=1024;lanes=32;bytes-per-lane=32;lane-order=ascending-contiguous;integer=unsigned-big-endian;modulus=257;reduction=32-fixed-byte-folds(x=byte+257-r,conditional-subtract257);ciphertext-ring-degree=64;state-c0-first=residue;state-other-coefficients=0;registers=4-zero-ciphertexts";

/// Return the digest of the compiled initializer contract published in every profile.
#[must_use]
pub fn bfv_program_initializer_descriptor_hash() -> Hash {
    Hash::new(BFV_PROGRAM_INITIALIZER_DESCRIPTOR)
}

// Borrowed fields stream directly to the hasher; no secret preimage Vec is created.
#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::ProgramInitializationInputV1",
    frame = "iroha_crypto::ram_lfe::ProgramInitializationInputV1"
)]
struct ProgramInitializationInputV1<'a> {
    initializer_descriptor_hash: Hash,
    policy_hash: Hash,
    secret: &'a [u8],
    associated_data: &'a [u8],
}

pub(super) fn validate_associated_data(bytes: &[u8]) -> Result<(), RamLfeError> {
    if bytes.len() > RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES {
        return Err(RamLfeError::Bfv(format!(
            "programmed BFV associated data exceeds {RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES} bytes"
        )));
    }
    Ok(())
}

// TODO(R.12): retire this policy-hash-bound initializer with the diagnostic
// BFV programmed backend. `derive_lanes_v1` is the canonical replacement.
pub(super) fn derive_residues(
    secret: &[u8],
    policy_hash: Hash,
    associated_data: &[u8],
) -> Result<Zeroizing<[u64; LANES]>, RamLfeError> {
    validate_secret(secret)?;
    validate_associated_data(associated_data)?;
    debug_assert!(secret.len() <= RAM_LFE_SECRET_MAX_BYTES);
    let input = ProgramInitializationInputV1 {
        initializer_descriptor_hash: bfv_program_initializer_descriptor_hash(),
        policy_hash,
        secret,
        associated_data,
    };
    let mut lanes = Zeroizing::new([0_u16; LANES]);
    expand_lanes(CONTEXT, &input, &mut lanes)?;
    let mut residues = Zeroizing::new([0_u64; LANES]);
    for (result, lane) in residues.iter_mut().zip(lanes.iter()) {
        *result = u64::from(*lane);
    }
    Ok(residues)
}

super::canonical::commitment_frame! {
    // The program key is borrowed and streams into the clearing hash state.
    struct RamLfeInitializationInputV1<'a> as "iroha_crypto::ram_lfe::RamLfeInitializationInputV1" {
        function_identity: Hash,
        associated_data_hash: Hash,
        program_key: &'a [u8],
    }
}

/// Field order of the canonical V1 initializer frame.
#[cfg(test)]
pub(super) const INITIALIZATION_FRAME: (&str, &[&str]) = (
    RamLfeInitializationInputV1::FRAME,
    RamLfeInitializationInputV1::FIELDS,
);

/// Derive the canonical V1 state lanes for one execution into the caller's owner.
///
/// Every execution derives its lanes again from these three inputs. Nothing an
/// earlier execution stored is an input, and no policy or encryption key is.
/// The lanes are written in place, so they never exist outside `lanes` and the
/// clearing buffers of this function.
pub(super) fn derive_lanes_v1(
    program_key: &[u8; 32],
    function_identity: Hash,
    associated_data_hash: Hash,
    lanes: &mut [u16; LANES],
) -> Result<(), RamLfeError> {
    expand_lanes(
        CONTEXT_V1,
        &RamLfeInitializationInputV1 {
            function_identity,
            associated_data_hash,
            program_key,
        },
        lanes,
    )
}

/// Expand one canonical private frame into 32 residues modulo 257, in place.
fn expand_lanes<T: norito::NoritoSerialize>(
    context: &'static str,
    input: &T,
    lanes: &mut [u16; LANES],
) -> Result<(), RamLfeError> {
    let mut hasher = Zeroizing::new(blake3::Hasher::new_derive_key(context));
    norito::core::write_canonical_to_writer(input, &mut *hasher)
        .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))?;
    let mut reader = Zeroizing::new(hasher.finalize_xof());
    let mut bytes = Zeroizing::new([0_u8; LANES * BYTES_PER_LANE]);
    reader.fill(bytes.as_mut());
    for (result, lane) in lanes.iter_mut().zip(bytes.chunks_exact(BYTES_PER_LANE)) {
        *result = reduce_lane(lane);
    }
    Ok(())
}

fn reduce_byte(residue: u16, byte: u8) -> u16 {
    // 256 == -1 (mod 257). For residue<=256, x is in 1..=512.
    let x = u16::from(byte) + 257 - residue;
    let difference = x.wrapping_sub(257);
    // Under the stated bounds, the high bit is exactly the subtraction borrow.
    let subtract = Choice::from(1 ^ (difference.to_be_bytes()[0] >> 7));
    u16::conditional_select(&x, &difference, subtract)
}

fn reduce_lane(bytes: &[u8]) -> u16 {
    let mut residue = Zeroizing::new(0_u16);
    for &byte in bytes {
        *residue = reduce_byte(*residue, byte);
    }
    *residue
}

#[cfg(test)]
#[path = "initialization_tests.rs"]
mod tests;
