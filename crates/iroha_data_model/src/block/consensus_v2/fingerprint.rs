//! Canonical Sumeragi v2 consensus-parameters fingerprint projection.

use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_primitives::numeric::Quantity;
use norito::codec::Encode;

use super::{ConsensusMode, SumeragiV2GenesisContextParameters};
use crate::block::consensus::{ConsensusGenesisModeParams, ConsensusGenesisParams};

const DOMAIN: &[u8] = b"iroha:sumeragi:v2:consensus-parameters-fingerprint:v1\0";

/// Version of the canonical v2 consensus-parameters projection.
pub const FORMAT_VERSION: u16 = 1;

#[derive(Encode)]
struct ConsensusParametersFingerprintInput {
    format_version: u16,
    protocol_version: u32,
    mode: ConsensusMode,
    block_cadence_ms: core::num::NonZeroU64,
    block_max_transactions: core::num::NonZeroU64,
    context: SumeragiV2GenesisContextParameters,
    npos: Option<NposGenesisFingerprintInput>,
}

#[derive(Encode)]
struct NposGenesisFingerprintInput {
    epoch_length_blocks: core::num::NonZeroU64,
    epoch_seed: [u8; 32],
    vrf_commit_window_blocks: u64,
    vrf_reveal_window_blocks: u64,
    max_validators: u32,
    min_self_bond: Quantity,
    min_nomination_bond: Quantity,
    max_nominator_concentration_pct: u8,
    seat_band_pct: u8,
    max_entity_correlation_pct: u8,
    finality_margin_blocks: u64,
    evidence_horizon_blocks: u64,
    activation_lag_blocks: u64,
    slashing_delay_blocks: u64,
}

/// Compute the deterministic v2 consensus-parameters fingerprint.
///
/// Only frozen v2 inputs are representable in the encoded projection. Legacy
/// collectors, per-phase/adaptive timers, the global-RBC enable flag, the BLS
/// domain string, and node-local fallbacks are deliberately discarded. Live
/// startup therefore cannot fingerprint input that omits its v2 context. Exact
/// network identity is deliberately not part of this genesis-embedded value:
/// runtime handshakes authenticate a separate required `NetworkId`, avoiding a
/// self-reference through the genesis block hash.
///
/// # Errors
///
/// Returns an error when the signed genesis parameters violate the first-release
/// consensus invariants.
#[must_use = "a rejected genesis carrier must not be fingerprinted"]
pub fn compute(params: &ConsensusGenesisParams) -> Result<[u8; 32], String> {
    params.validate()?;
    let (mode, npos) = match &params.mode {
        ConsensusGenesisModeParams::Permissioned => (ConsensusMode::Permissioned, None),
        ConsensusGenesisModeParams::Npos(npos) => (
            ConsensusMode::Npos,
            Some(NposGenesisFingerprintInput {
                epoch_length_blocks: npos.epoch_length_blocks,
                epoch_seed: npos.epoch_seed,
                vrf_commit_window_blocks: npos.vrf_commit_window_blocks,
                vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks,
                max_validators: npos.max_validators,
                min_self_bond: npos.min_self_bond.clone(),
                min_nomination_bond: npos.min_nomination_bond.clone(),
                max_nominator_concentration_pct: npos.max_nominator_concentration_pct,
                seat_band_pct: npos.seat_band_pct,
                max_entity_correlation_pct: npos.max_entity_correlation_pct,
                finality_margin_blocks: npos.finality_margin_blocks,
                evidence_horizon_blocks: npos.evidence_horizon_blocks,
                activation_lag_blocks: npos.activation_lag_blocks,
                slashing_delay_blocks: npos.slashing_delay_blocks,
            }),
        ),
    };
    let projection = ConsensusParametersFingerprintInput {
        format_version: FORMAT_VERSION,
        protocol_version: params.protocol_version,
        mode,
        block_cadence_ms: params.block_cadence_ms,
        block_max_transactions: params.block_max_transactions,
        context: params.v2_context,
        npos,
    };

    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, DOMAIN);
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &projection.encode());
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest[..32]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissioned_params() -> ConsensusGenesisParams {
        ConsensusGenesisParams {
            block_cadence_ms: core::num::NonZeroU64::new(1_000).unwrap(),
            block_max_transactions: core::num::NonZeroU64::new(512).unwrap(),
            mode: ConsensusGenesisModeParams::Permissioned,
            protocol_version: u32::from(super::super::PROTOCOL_VERSION),
            v2_context: SumeragiV2GenesisContextParameters::recommended(),
        }
    }

    fn npos_params() -> ConsensusGenesisParams {
        let mut params = permissioned_params();
        params.mode =
            ConsensusGenesisModeParams::Npos(crate::block::consensus::NposGenesisParams {
                epoch_length_blocks: core::num::NonZeroU64::new(3_600).unwrap(),
                epoch_seed: [7; 32],
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                max_validators: 31,
                min_self_bond: 1_000_u64.into(),
                min_nomination_bond: 1_u64.into(),
                max_nominator_concentration_pct: 25,
                seat_band_pct: 5,
                max_entity_correlation_pct: 25,
                finality_margin_blocks: 8,
                evidence_horizon_blocks: 7_200,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 259_200,
            });
        params
    }

    #[test]
    fn signed_cadence_changes_live_fingerprint() {
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.block_cadence_ms = core::num::NonZeroU64::new(1_001).unwrap();

        assert_ne!(compute(&baseline).unwrap(), compute(&changed).unwrap(),);
    }

    #[test]
    fn signed_context_mismatch_changes_live_fingerprint() {
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.v2_context.nexus_amx_context_hash[0] ^= 1;

        assert_ne!(compute(&baseline).unwrap(), compute(&changed).unwrap(),);
    }

    #[test]
    fn signed_execution_policy_mismatch_changes_live_fingerprint() {
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.v2_context.execution_policy_hash[0] ^= 1;

        assert_ne!(compute(&baseline).unwrap(), compute(&changed).unwrap(),);
    }

    #[test]
    fn npos_election_input_changes_live_fingerprint() {
        let baseline = npos_params();
        let mut changed = baseline.clone();
        let ConsensusGenesisModeParams::Npos(npos) = &mut changed.mode else {
            unreachable!()
        };
        npos.epoch_seed[0] ^= 1;
        assert_ne!(compute(&baseline).unwrap(), compute(&changed).unwrap(),);
    }

    #[test]
    fn all_zero_npos_seed_is_rejected_before_hashing() {
        let mut params = npos_params();
        let ConsensusGenesisModeParams::Npos(npos) = &mut params.mode else {
            unreachable!()
        };
        npos.epoch_seed = [0; 32];
        assert!(compute(&params).is_err());
    }

    #[test]
    fn unsupported_protocol_is_rejected_before_hashing() {
        let mut params = permissioned_params();
        params.protocol_version += 1;
        let error = compute(&params).expect_err("unsupported wire revision must fail closed");
        assert!(error.contains("unsupported consensus protocol version"));
    }

    #[test]
    fn invalid_data_availability_context_is_rejected_before_hashing() {
        let mut params = permissioned_params();
        params.v2_context.da_layout.chunk_size_bytes = 0;
        let error = compute(&params).expect_err("zero DA chunk size must fail closed");
        assert!(error.contains("invalid Sumeragi v2 genesis context"));
    }

    #[test]
    fn zero_execution_policy_context_is_rejected_before_hashing() {
        let mut params = permissioned_params();
        params.v2_context.execution_policy_hash = [0; 32];
        let error = compute(&params).expect_err("zero execution-policy hash must fail closed");
        assert!(error.contains("invalid Sumeragi v2 genesis context"));
    }

    #[test]
    fn npos_windows_outside_epoch_are_rejected_before_hashing() {
        let mut params = npos_params();
        let ConsensusGenesisModeParams::Npos(npos) = &mut params.mode else {
            unreachable!()
        };
        npos.vrf_commit_window_blocks = 3_599;
        npos.vrf_reveal_window_blocks = 2;
        let error =
            compute(&params).expect_err("VRF windows outside the signed epoch must fail closed");
        assert!(error.contains("close before the epoch boundary"));
    }

    #[test]
    fn npos_percentage_above_one_hundred_is_rejected_before_hashing() {
        let mut params = npos_params();
        let ConsensusGenesisModeParams::Npos(npos) = &mut params.mode else {
            unreachable!()
        };
        npos.max_entity_correlation_pct = 101;
        let error =
            compute(&params).expect_err("invalid signed election percentages must fail closed");
        assert!(error.contains("percentages"));
    }

    #[test]
    fn genesis_embedded_fingerprint_is_deterministic_without_genesis_hash_input() {
        let params = permissioned_params();
        let first = compute(&params).expect("valid parameters fingerprint");
        let second = compute(&params).expect("same valid parameters fingerprint");
        assert_eq!(first, second);
    }
}
