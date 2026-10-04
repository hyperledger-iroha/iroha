//! Canonical public beacon framing shared by execution and authenticated proof readers.
use crate::NetworkId;
use crate::consensus::{FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1};
use iroha_crypto::{
    Hash,
    threshold_bls::{BeaconPurpose, ThresholdBlsError, ThresholdBlsSignature},
};
/// Exact domain of the fixed-width finalized pulse signing preimage.
pub const GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.pulse-payload.v1\0";
/// Exact byte length of the canonical fixed-width beacon signing payload.
pub const GLOBAL_BEACON_PULSE_PAYLOAD_LEN_V1: usize =
    GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 9 + 8 * 4;
const GLOBAL_BEACON_PULSE_ID_DOMAIN_V1: &[u8] = b"iroha.global-threshold-beacon.pulse-id.v1\0";
const GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1: u64 = 0;

/// Invalid public beacon shape; threshold verification still requires the authenticated session.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum BeaconPulseShapeError {
    /// A required native consensus identity is absent or inert.
    #[error("beacon pulse native context mismatch")]
    PulseContextMismatch,
    /// Unsupported pulse layout.
    #[error("unsupported beacon version {actual}")]
    UnsupportedVersion {
        /// Advertised layout version.
        actual: u16,
    },
    /// A pulse uses a noncanonical round.
    #[error("noncanonical beacon round")]
    NonCanonicalRound,
    /// A required public binding is zero.
    #[error("zero beacon binding")]
    ZeroPulse,
    /// Compressed threshold signature is malformed.
    #[error(transparent)]
    Signature(#[from] ThresholdBlsError),
    /// Identifier does not hash the exact public payload, signature and seed.
    #[error("beacon pulse identifier mismatch")]
    PulseIdMismatch,
}
fn zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&byte| byte == 0)
}
/// Validate inert/replay bindings, canonical signature encoding and the exact public identifier.
/// This does not verify a threshold signature against its authenticated session.
///
/// # Errors
/// A malformed version, round, binding, signature or identifier.
pub fn validate_beacon_pulse_shape(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
) -> Result<(), BeaconPulseShapeError> {
    pulse
        .context
        .validate()
        .map_err(|_| BeaconPulseShapeError::PulseContextMismatch)?;
    if pulse.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1 {
        return Err(BeaconPulseShapeError::UnsupportedVersion {
            actual: pulse.version,
        });
    }
    if pulse.round != GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1 {
        return Err(BeaconPulseShapeError::NonCanonicalRound);
    }
    if pulse.height == 0
        || zero(&pulse.pulse_id)
        || zero(&pulse.seed)
        || zero(&pulse.roster_hash)
        || zero(&pulse.transcript_hash)
        || zero(pulse.finalized_chain_anchor.block_hash.as_ref())
    {
        return Err(BeaconPulseShapeError::ZeroPulse);
    }
    ThresholdBlsSignature::<BeaconPurpose>::from_bytes(pulse.session_id, &pulse.signature)?;
    if global_threshold_beacon_pulse_id_v1(pulse, pulse.seed) != pulse.pulse_id {
        return Err(BeaconPulseShapeError::PulseIdMismatch);
    }
    Ok(())
}

/// Build the exact fully consuming payload signed by a finalized beacon pulse.
///
/// The signature, derived seed, and derived pulse ID are omitted because each
/// depends on this payload. All integer fields are encoded big-endian and every
/// remaining field has a fixed width. No earlier pulse identifier or seed is
/// included: every `(session, height, fixed round, finalized parent)` slot is
/// independently unique. The complete native instance, epoch, authenticated epoch context,
/// parent consensus hash and parent result are signed without a view number, so
/// skipping an optional governance slot cannot alter
/// a later mandatory `NPoS` pulse.
#[must_use]
pub fn global_threshold_beacon_pulse_payload_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
) -> [u8; GLOBAL_BEACON_PULSE_PAYLOAD_LEN_V1] {
    let mut payload = [0; GLOBAL_BEACON_PULSE_PAYLOAD_LEN_V1];
    let mut offset = 0;
    for field in [
        GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1,
        &pulse.version.to_be_bytes(),
        pulse.network_id.as_bytes(),
        &pulse.session_id,
        &pulse.roster_hash,
        &pulse.transcript_hash,
        &pulse.context.instance,
        &pulse.context.epoch.to_be_bytes(),
        &pulse.context.epoch_context_id,
        &pulse.context.parent_consensus_hash,
        &pulse.context.parent_result,
        &pulse.height.to_be_bytes(),
        &pulse.round.to_be_bytes(),
        &pulse.finalized_chain_anchor.height.to_be_bytes(),
        pulse.finalized_chain_anchor.block_hash.as_ref(),
    ] {
        let end = offset + field.len();
        payload[offset..end].copy_from_slice(field);
        offset = end;
    }
    debug_assert_eq!(offset, payload.len());
    payload
}

/// Derive the canonical pulse identifier after final-signature verification.
#[must_use]
pub fn global_threshold_beacon_pulse_id_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    verified_seed: [u8; 32],
) -> [u8; 32] {
    let payload = global_threshold_beacon_pulse_payload_v1(pulse);
    *Hash::new_from_chunks(&[
        GLOBAL_BEACON_PULSE_ID_DOMAIN_V1,
        &u32::try_from(payload.len())
            .expect("fixed-size beacon pulse payload")
            .to_be_bytes(),
        &payload,
        &pulse.signature,
        &verified_seed,
    ])
    .as_ref()
}

const GLOBAL_BEACON_NPOS_SUCCESSOR_SEED_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.npos-successor-seed.v1\0";
/// Derive the `NPoS` successor seed from one already-verified global beacon pulse.
///
/// The dedicated domain prevents a pulse seed consumed by Parliament or another
/// protocol from being reused as the raw `NPoS` PRF key. The target boundary and
/// successor epoch are explicit even though the pulse identifier already binds
/// its signed position; this makes accidental cross-epoch reuse impossible at
/// the consensus call site.
#[must_use]
pub fn global_threshold_beacon_npos_successor_seed_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    boundary_height: u64,
    successor_epoch: u64,
) -> [u8; 32] {
    let boundary_height = boundary_height.to_be_bytes();
    let successor_epoch = successor_epoch.to_be_bytes();
    *Hash::new_from_chunks(&[
        GLOBAL_BEACON_NPOS_SUCCESSOR_SEED_DOMAIN_V1,
        pulse.network_id.as_bytes(),
        pulse.session_id.as_slice(),
        pulse.pulse_id.as_slice(),
        pulse.seed.as_slice(),
        pulse.height.to_be_bytes().as_slice(),
        pulse.finalized_chain_anchor.height.to_be_bytes().as_slice(),
        boundary_height.as_slice(),
        successor_epoch.as_slice(),
    ])
    .as_ref()
}

/// Derive the domain-separated E+2 election seed from its authenticated selection pulse.
///
/// # Errors
/// Foreign network, overflowing target epoch or a zero digest.
pub fn election_seed(
    network: NetworkId,
    selection_epoch: u64,
    pulse: &crate::consensus::FinalizedGlobalThresholdBeaconPulseV1,
) -> Result<[u8; 32], String> {
    if pulse.network_id != network {
        return Err("election pulse belongs to another network".into());
    }
    let target_epoch = selection_epoch
        .checked_add(2)
        .ok_or("target epoch overflows")?;
    let seed = Hash::new_from_chunks(&[
        b"iroha:validator-election:v1",
        &[0],
        network.as_bytes(),
        &selection_epoch.to_le_bytes(),
        &target_epoch.to_le_bytes(),
        &pulse.pulse_id,
        &pulse.seed,
    ])
    .into();
    if seed == [0; 32] {
        return Err("election entropy is zero".into());
    }
    Ok(seed)
}

#[cfg(test)]
mod tests;
