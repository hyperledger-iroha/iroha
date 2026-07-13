//! Canonical Sumeragi v2 genesis/handshake fingerprint projection.

use iroha_crypto::blake2::{Blake2b512, Digest as _};
use norito::codec::Encode;

use super::{ConsensusMode, SumeragiV2GenesisContextParameters};
use crate::{
    ChainId,
    block::consensus::{ConsensusGenesisModeParams, ConsensusGenesisParams},
};

const DOMAIN: &[u8] = b"iroha:sumeragi:v2:genesis-fingerprint\0";

/// Version of the canonical v2 genesis-fingerprint projection.
pub const FORMAT_VERSION: u16 = 1;

#[derive(Encode)]
struct GenesisFingerprintInput {
    format_version: u16,
    protocol_version: u32,
    chain_id: ChainId,
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
    min_self_bond: u64,
    min_nomination_bond: u64,
    max_nominator_concentration_pct: u8,
    seat_band_pct: u8,
    max_entity_correlation_pct: u8,
    finality_margin_blocks: u64,
    evidence_horizon_blocks: u64,
    activation_lag_blocks: u64,
    slashing_delay_blocks: u64,
}

/// Compute the live v2 genesis/handshake fingerprint.
///
/// Only frozen v2 inputs are representable in the encoded projection. Legacy
/// collectors, per-phase/adaptive timers, the global-RBC enable flag, the BLS
/// domain string, and node-local fallbacks are deliberately discarded. Live
/// startup therefore cannot fingerprint input that omits its v2 context.
#[must_use = "a rejected genesis carrier must not be fingerprinted"]
pub fn compute(chain_id: &ChainId, params: &ConsensusGenesisParams) -> Result<[u8; 32], String> {
    params.validate()?;
    let (mode, npos) = match params.mode {
        ConsensusGenesisModeParams::Permissioned => (ConsensusMode::Permissioned, None),
        ConsensusGenesisModeParams::Npos(npos) => (
            ConsensusMode::Npos,
            Some(NposGenesisFingerprintInput {
                epoch_length_blocks: npos.epoch_length_blocks,
                epoch_seed: npos.epoch_seed,
                vrf_commit_window_blocks: npos.vrf_commit_window_blocks,
                vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks,
                max_validators: npos.max_validators,
                min_self_bond: npos.min_self_bond,
                min_nomination_bond: npos.min_nomination_bond,
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
    let projection = GenesisFingerprintInput {
        format_version: FORMAT_VERSION,
        protocol_version: params.protocol_version,
        chain_id: chain_id.clone(),
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
                max_validators: 128,
                min_self_bond: 1_000,
                min_nomination_bond: 1,
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
        let chain = ChainId::from("fingerprint-cadence");
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.block_cadence_ms = core::num::NonZeroU64::new(1_001).unwrap();

        assert_ne!(
            compute(&chain, &baseline).unwrap(),
            compute(&chain, &changed).unwrap(),
        );
    }

    #[test]
    fn signed_context_mismatch_changes_live_fingerprint() {
        let chain = ChainId::from("fingerprint-context-mismatch");
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.v2_context.nexus_amx_context_hash[0] ^= 1;

        assert_ne!(
            compute(&chain, &baseline).unwrap(),
            compute(&chain, &changed).unwrap(),
        );
    }

    #[test]
    fn npos_election_input_changes_live_fingerprint() {
        let chain = ChainId::from("fingerprint-npos-election");
        let baseline = npos_params();
        let mut changed = baseline.clone();
        let ConsensusGenesisModeParams::Npos(npos) = &mut changed.mode else {
            unreachable!()
        };
        npos.epoch_seed[0] ^= 1;
        assert_ne!(
            compute(&chain, &baseline).unwrap(),
            compute(&chain, &changed).unwrap(),
        );
    }

    #[test]
    fn all_zero_npos_seed_is_rejected_before_hashing() {
        let chain = ChainId::from("fingerprint-zero-seed");
        let mut params = npos_params();
        let ConsensusGenesisModeParams::Npos(npos) = &mut params.mode else {
            unreachable!()
        };
        npos.epoch_seed = [0; 32];
        assert!(compute(&chain, &params).is_err());
    }

    #[test]
    fn unsupported_protocol_is_rejected_before_hashing() {
        let mut params = permissioned_params();
        params.protocol_version += 1;
        let error = compute(&ChainId::from("unsupported-protocol"), &params)
            .expect_err("unsupported wire revision must fail closed");
        assert!(error.contains("unsupported consensus protocol version"));
    }

    #[test]
    fn invalid_data_availability_context_is_rejected_before_hashing() {
        let mut params = permissioned_params();
        params.v2_context.da_layout.chunk_size_bytes = 0;
        let error = compute(&ChainId::from("invalid-da-layout"), &params)
            .expect_err("zero DA chunk size must fail closed");
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
        let error = compute(&ChainId::from("invalid-vrf-windows"), &params)
            .expect_err("VRF windows outside the signed epoch must fail closed");
        assert!(error.contains("fit within the epoch"));
    }

    #[test]
    fn npos_percentage_above_one_hundred_is_rejected_before_hashing() {
        let mut params = npos_params();
        let ConsensusGenesisModeParams::Npos(npos) = &mut params.mode else {
            unreachable!()
        };
        npos.max_entity_correlation_pct = 101;
        let error = compute(&ChainId::from("invalid-election-percentage"), &params)
            .expect_err("invalid signed election percentages must fail closed");
        assert!(error.contains("percentages"));
    }
}
