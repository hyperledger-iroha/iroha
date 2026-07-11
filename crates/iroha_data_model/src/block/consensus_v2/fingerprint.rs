//! Canonical Sumeragi v2 genesis/handshake fingerprint projection.

use iroha_crypto::blake2::{Blake2b512, Digest as _};
use norito::codec::Encode;

use super::{ConsensusMode, SumeragiV2GenesisContextParameters};
use crate::{ChainId, block::consensus::ConsensusGenesisParams};

const DOMAIN: &[u8] = b"iroha:sumeragi:v2:genesis-fingerprint\0";

/// Version of the canonical v2 genesis-fingerprint projection.
pub const FORMAT_VERSION: u16 = 1;

#[derive(Encode)]
struct GenesisFingerprintInput {
    format_version: u16,
    protocol_version: u32,
    chain_id: ChainId,
    mode: ConsensusMode,
    block_cadence_ms: u64,
    round_timeout_ms: u64,
    block_max_transactions: u64,
    context: Option<SumeragiV2GenesisContextParameters>,
    npos: Option<NposGenesisFingerprintInput>,
}

#[derive(Encode)]
struct NposGenesisFingerprintInput {
    epoch_length_blocks: u64,
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
/// startup separately requires `params.v2_context` to be present and signed;
/// retaining `Option` here lets archival tooling fingerprint malformed input
/// without inventing a fallback context.
#[must_use]
pub fn compute(
    chain_id: &ChainId,
    mode: ConsensusMode,
    params: &ConsensusGenesisParams,
) -> [u8; 32] {
    let npos = if mode == ConsensusMode::Npos {
        params
            .npos
            .as_ref()
            .map(|npos| NposGenesisFingerprintInput {
                epoch_length_blocks: params.epoch_length_blocks,
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
            })
    } else {
        None
    };
    let projection = GenesisFingerprintInput {
        format_version: FORMAT_VERSION,
        protocol_version: params.protocol_version,
        chain_id: chain_id.clone(),
        mode,
        block_cadence_ms: params.block_time_ms,
        round_timeout_ms: params.round_timeout_ms,
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissioned_params() -> ConsensusGenesisParams {
        ConsensusGenesisParams {
            block_time_ms: 1_000,
            commit_time_ms: 3_000,
            min_finality_ms: 100,
            max_clock_drift_ms: 500,
            collectors_k: 3,
            redundant_send_r: 2,
            block_max_transactions: 512,
            da_enabled: true,
            epoch_length_blocks: 0,
            bls_domain: super::super::PERMISSIONED_BLS_DOMAIN.to_owned(),
            npos: None,
            protocol_version: u32::from(super::super::PROTOCOL_VERSION),
            round_timeout_ms: 10_000,
            v2_context: Some(SumeragiV2GenesisContextParameters::recommended()),
        }
    }

    #[test]
    fn retired_fields_are_not_representable_in_live_fingerprint() {
        let chain = ChainId::from("fingerprint-retired-fields");
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed.commit_time_ms += 1;
        changed.min_finality_ms += 1;
        changed.max_clock_drift_ms += 1;
        changed.collectors_k += 1;
        changed.redundant_send_r += 1;
        changed.da_enabled = !changed.da_enabled;
        changed.bls_domain = "legacy-arbitrary-domain".to_owned();

        assert_eq!(
            compute(&chain, ConsensusMode::Permissioned, &baseline),
            compute(&chain, ConsensusMode::Permissioned, &changed),
        );
    }

    #[test]
    fn signed_context_mismatch_changes_live_fingerprint() {
        let chain = ChainId::from("fingerprint-context-mismatch");
        let baseline = permissioned_params();
        let mut changed = baseline.clone();
        changed
            .v2_context
            .as_mut()
            .expect("signed v2 context")
            .nexus_amx_context_hash[0] ^= 1;

        assert_ne!(
            compute(&chain, ConsensusMode::Permissioned, &baseline),
            compute(&chain, ConsensusMode::Permissioned, &changed),
        );
    }
}
