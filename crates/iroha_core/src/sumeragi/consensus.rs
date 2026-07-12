//! Sumeragi core message types and helpers.
//!
//! This module defines canonical, Norito-encoded types for QC voting
//! (prepare/commit/new-view), evidence, and consensus helpers.
//! It is used by the consensus adapters and related tooling.
//!
//! Mode separation (permissioned vs `NPoS`) is runtime-selectable via config/WSV.
//! Build artifacts no longer hard‑code consensus mode; peers validate mode
//! at handshake time using a mode tag provided by the running node.

// BLS signatures are mandatory; consensus code must not be built without the `bls` feature.
#[cfg(not(feature = "bls"))]
compile_error!(
    "The `bls` feature is mandatory for iroha_core consensus; rebuild with `--features bls`"
);

use std::time::Duration;

use iroha_config::parameters::actual::{Common as CommonConfig, Sumeragi as SumeragiConfig};
#[cfg(test)]
use iroha_crypto::HashOf;
pub use iroha_data_model::block::consensus::{
    BlockSubject, CertPhase, Certificate, ConsensusBlockHeader, ConsensusGenesisParams, Evidence,
    EvidenceKind, EvidencePayload, ExecKv, ExecWitness, ExecWitnessMsg, Height,
    LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1,
    LaneBlockVoteBodyV1, NposGenesisParams, PayloadRequest, PayloadResponse, Proposal, Qc,
    QcAggregate, QcRef, QcVote, QuorumPolicy, RbcChunk, RbcChunkRequest, RbcDeliver, RbcInit,
    RbcInitRequest, RbcReady, RbcReadySignature, Reconfig, RoundId, ValidatorIndex, ValidatorSetId,
    View, VrfCommit, VrfReveal, default_chain_order_hash,
};
use norito::codec::Encode;

/// Live consensus protocol revision. Legacy v1 types above are archival only.
pub const PROTO_VERSION: u32 = iroha_data_model::block::consensus_v2::PROTOCOL_VERSION as u32;
/// Permissioned Sumeragi v2 handshake and signing-domain tag.
pub const PERMISSIONED_TAG: &str = iroha_data_model::block::consensus_v2::PERMISSIONED_TAG;
/// NPoS Sumeragi v2 handshake and signing-domain tag.
pub const NPOS_TAG: &str = iroha_data_model::block::consensus_v2::NPOS_TAG;
/// Legacy v1 protocol revision retained for archival validation tools.
pub const LEGACY_PROTO_VERSION: u32 = iroha_data_model::block::consensus::PROTO_VERSION;
/// Legacy permissioned tag retained for archival validation tools.
pub const LEGACY_PERMISSIONED_TAG: &str = iroha_data_model::block::consensus::PERMISSIONED_TAG;
/// Legacy NPoS tag retained for archival validation tools.
pub const LEGACY_NPOS_TAG: &str = iroha_data_model::block::consensus::NPOS_TAG;

/// Commit-certificate phase (prepare/commit/new-view).
pub type Phase = CertPhase;
/// Runtime adapter vote used for certificate aggregation.
pub type Vote = QcVote;
/// Reference to a QC header carried in hints.
pub type QcHeaderRef = QcRef;
use iroha_data_model::parameter::system::SumeragiNposParameters;
use iroha_data_model::prelude::*;

use crate::state::{StateView, WorldReadOnly};

/// Count the number of validators encoded into a QC signer bitmap.
pub fn qc_signer_count(qc: &Qc) -> usize {
    qc.aggregate
        .signers_bitmap
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum()
}

#[cfg(feature = "sumeragi-multiproof")]
pub use iroha_data_model::block::consensus::{BlockMultiproof, ReadNode, TxReadSpan, WriteEntry};

/// Build the canonical preimage for a QC vote signature under the given chain and mode tag.
pub fn vote_preimage(chain_id: &ChainId, mode_tag: &str, v: &Vote) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 * 4 + 8 * 6 + 3);
    let domain = consensus_domain(chain_id, "Vote", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(v.block_hash.as_ref().as_ref());
    out.extend_from_slice(v.parent_state_root.as_ref());
    out.extend_from_slice(v.post_state_root.as_ref());
    out.extend_from_slice(&v.height.to_be_bytes());
    out.extend_from_slice(&v.view.to_be_bytes());
    out.extend_from_slice(&v.epoch.to_be_bytes());
    out.extend_from_slice(v.chain_order_hash.as_ref());
    out.extend_from_slice(&v.rechain_seq.to_be_bytes());
    out.push(v.phase as u8);
    match v.highest_qc {
        Some(highest_qc) => {
            out.push(1);
            out.extend_from_slice(&highest_qc.height.to_be_bytes());
            out.extend_from_slice(&highest_qc.view.to_be_bytes());
            out.extend_from_slice(&highest_qc.epoch.to_be_bytes());
            out.extend_from_slice(highest_qc.subject_block_hash.as_ref().as_ref());
            out.push(highest_qc.phase as u8);
        }
        None => out.push(0),
    }
    out
}

/// Build the canonical preimage for a VRF commit signature under the given chain and mode tag.
pub fn vrf_commit_preimage(chain_id: &ChainId, mode_tag: &str, c: &VrfCommit) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32);
    let domain = consensus_domain(chain_id, "VrfCommit", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&c.epoch.to_be_bytes());
    out.extend_from_slice(&c.signer.to_be_bytes());
    out.extend_from_slice(&c.commitment);
    out
}

/// Build the canonical preimage for a VRF reveal signature under the given chain and mode tag.
pub fn vrf_reveal_preimage(chain_id: &ChainId, mode_tag: &str, r: &VrfReveal) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32);
    let domain = consensus_domain(chain_id, "VrfReveal", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&r.epoch.to_be_bytes());
    out.extend_from_slice(&r.signer.to_be_bytes());
    out.extend_from_slice(&r.reveal);
    out
}

/// Canonical preimage helpers for BLS signing (same-message across signers).
#[cfg(feature = "bls")]
pub mod bls_preimage {
    use super::*;

    /// Build the canonical preimage for a Vote signature under the given chain and mode tag.
    pub fn vote(chain_id: &ChainId, mode_tag: &str, v: &Vote) -> Vec<u8> {
        super::vote_preimage(chain_id, mode_tag, v)
    }

    /// Build the canonical preimage for a VRF commit signature.
    pub fn vrf_commit(chain_id: &ChainId, mode_tag: &str, c: &VrfCommit) -> Vec<u8> {
        super::vrf_commit_preimage(chain_id, mode_tag, c)
    }

    /// Build the canonical preimage for a VRF reveal signature.
    pub fn vrf_reveal(chain_id: &ChainId, mode_tag: &str, r: &VrfReveal) -> Vec<u8> {
        super::vrf_reveal_preimage(chain_id, mode_tag, r)
    }
}

/// Domain separation helper for signable payloads.
/// Returns a 32‑byte Blake2b digest of the domain preimage.
pub fn consensus_domain(
    chain_id: &ChainId,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, b"iroha-sumeragi-consensus/v1");
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &PROTO_VERSION.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, message_type_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, extra);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

/// Deterministic computation of the consensus fingerprint required by genesis.
/// `blake2b32(MODE_TAG || canonical_json(consensus_params) || bls_domain || proto_versions || chain_id)`
pub fn compute_consensus_fingerprint(
    chain_id: &ChainId,
    consensus_params_json: &str,
    bls_domain: &str,
    proto_versions: &[u32],
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, consensus_params_json.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, bls_domain.as_bytes());
    for v in proto_versions {
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &v.to_be_bytes());
    }
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

/// Compute the live v2 handshake fingerprint from only frozen protocol inputs.
///
/// The projection deliberately omits v1 collectors, phase-specific and
/// adaptive timeouts, the global-RBC switch, and any local fallback. Mode,
/// cadence, the single round timeout, block bound, signed DA/Nexus context,
/// and the genesis-selected NPoS election inputs are canonical Norito fields.
/// Mutable shared adapter settings are committed separately by
/// [`SumeragiConfig::v2_config`].
pub fn compute_consensus_fingerprint_from_params(
    chain_id: &ChainId,
    params: &ConsensusGenesisParams,
    mode_tag: &str,
) -> [u8; 32] {
    let mode = if mode_tag == NPOS_TAG {
        iroha_data_model::block::consensus_v2::ConsensusMode::Npos
    } else {
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned
    };
    iroha_data_model::block::consensus_v2::fingerprint::compute(chain_id, mode, params)
}

/// Compute the retired full-parameter fingerprint for decode/archive tooling.
///
/// This preserves the former preimage exactly. Live v2 peer admission must use
/// [`compute_consensus_fingerprint_from_params`] instead.
pub fn compute_legacy_consensus_fingerprint_from_params(
    chain_id: &ChainId,
    params: &ConsensusGenesisParams,
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &PROTO_VERSION.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &params.encode());
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

/// Build the compatibility carrier for consensus-genesis parameters.
///
/// Runtime handshakes, genesis metadata generation, and startup validation all
/// pass this carrier through the canonical v2 fingerprint projection. Retired
/// fields remain populated only so archival v1 structures keep decoding.
pub fn consensus_genesis_params_from_parameters(
    chain_id: &ChainId,
    mode_tag: &str,
    bls_domain: impl Into<String>,
    params: &iroha_data_model::parameter::Parameters,
    sumeragi_config: &SumeragiConfig,
) -> ConsensusGenesisParams {
    let sumeragi = params.sumeragi();
    let block = params.block();
    let use_npos = mode_tag == NPOS_TAG;

    let npos_payload = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);

    let (npos_params, epoch_length_blocks) = if use_npos {
        let round_timeout_ms = u64::try_from(sumeragi_config.round_timeout.as_millis())
            .expect("Sumeragi round timeout exceeds supported millisecond range");

        match npos_payload {
            Some(npos) => (
                Some(NposGenesisParams {
                    block_time_ms: sumeragi.block_time_ms,
                    timeout_propose_ms: round_timeout_ms,
                    timeout_prevote_ms: round_timeout_ms,
                    timeout_precommit_ms: round_timeout_ms,
                    timeout_commit_ms: round_timeout_ms,
                    timeout_da_ms: round_timeout_ms,
                    timeout_aggregator_ms: round_timeout_ms,
                    k_aggregators: npos.k_aggregators(),
                    redundant_send_r: npos.redundant_send_r(),
                    epoch_seed: npos.epoch_seed(),
                    vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                    vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                    max_validators: npos.max_validators(),
                    min_self_bond: npos.min_self_bond(),
                    min_nomination_bond: npos.min_nomination_bond(),
                    max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                    seat_band_pct: npos.seat_band_pct(),
                    max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                    finality_margin_blocks: npos.finality_margin_blocks(),
                    evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                    activation_lag_blocks: npos.activation_lag_blocks(),
                    slashing_delay_blocks: npos.slashing_delay_blocks(),
                }),
                npos.epoch_length_blocks().max(1),
            ),
            None => {
                let npos_cfg = &sumeragi_config.npos;
                (
                    Some(NposGenesisParams {
                        block_time_ms: sumeragi.block_time_ms,
                        timeout_propose_ms: round_timeout_ms,
                        timeout_prevote_ms: round_timeout_ms,
                        timeout_precommit_ms: round_timeout_ms,
                        timeout_commit_ms: round_timeout_ms,
                        timeout_da_ms: round_timeout_ms,
                        timeout_aggregator_ms: round_timeout_ms,
                        k_aggregators: 0,
                        redundant_send_r: 0,
                        epoch_seed: super::chain_epoch_seed(chain_id),
                        vrf_commit_window_blocks: npos_cfg.vrf.commit_window_blocks,
                        vrf_reveal_window_blocks: npos_cfg.vrf.reveal_window_blocks,
                        max_validators: npos_cfg.election.max_validators,
                        min_self_bond: npos_cfg.election.min_self_bond,
                        min_nomination_bond: npos_cfg.election.min_nomination_bond,
                        max_nominator_concentration_pct: npos_cfg
                            .election
                            .max_nominator_concentration_pct,
                        seat_band_pct: npos_cfg.election.seat_band_pct,
                        max_entity_correlation_pct: npos_cfg.election.max_entity_correlation_pct,
                        finality_margin_blocks: npos_cfg.election.finality_margin_blocks,
                        evidence_horizon_blocks: npos_cfg.reconfig.evidence_horizon_blocks,
                        activation_lag_blocks: npos_cfg.reconfig.activation_lag_blocks,
                        slashing_delay_blocks: npos_cfg.reconfig.slashing_delay_blocks,
                    }),
                    npos_cfg.epoch_length_blocks.max(1),
                )
            }
        }
    } else {
        (None, 0)
    };

    ConsensusGenesisParams {
        block_time_ms: sumeragi.block_time_ms,
        commit_time_ms: sumeragi.commit_time_ms,
        min_finality_ms: sumeragi.min_finality_ms,
        max_clock_drift_ms: sumeragi.max_clock_drift_ms,
        collectors_k: sumeragi.collectors_k,
        redundant_send_r: sumeragi.collectors_redundant_send_r,
        block_max_transactions: block.max_transactions().get(),
        da_enabled: sumeragi.da_enabled,
        epoch_length_blocks,
        bls_domain: bls_domain.into(),
        npos: npos_params,
        protocol_version: u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION),
        round_timeout_ms: u64::try_from(sumeragi_config.round_timeout.as_millis())
            .expect("Sumeragi round timeout exceeds supported millisecond range"),
        // Only a signed genesis metadata entry may populate this. Callers which
        // verify a genesis attach the decoded value before fingerprinting;
        // runtime config is deliberately not a source for height contexts.
        v2_context: None,
    }
}

/// Derive consensus handshake capabilities (mode tag, BLS domain, fingerprint) from a world
/// snapshot, committed height, and local configuration.
#[allow(clippy::too_many_lines)]
pub fn compute_consensus_handshake_caps_from_world(
    world: &impl WorldReadOnly,
    _height: u64,
    common_config: &CommonConfig,
    sumeragi_config: &SumeragiConfig,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
) -> Result<
    (String, String, iroha_p2p::ConsensusHandshakeCaps),
    iroha_config::parameters::actual::SumeragiV2ConfigError,
> {
    let s_params = world.parameters();
    let (mode_tag, bls_domain) = match frozen_mode {
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned => (
            PERMISSIONED_TAG.to_string(),
            iroha_data_model::block::consensus_v2::PERMISSIONED_BLS_DOMAIN.to_string(),
        ),
        iroha_data_model::block::consensus_v2::ConsensusMode::Npos => (
            NPOS_TAG.to_string(),
            iroha_data_model::block::consensus_v2::NPOS_BLS_DOMAIN.to_string(),
        ),
    };
    let canon = consensus_genesis_params_from_parameters(
        &common_config.chain,
        &mode_tag,
        bls_domain.clone(),
        s_params,
        sumeragi_config,
    );
    let fingerprint =
        compute_consensus_fingerprint_from_params(&common_config.chain, &canon, &mode_tag);
    let mut config_caps = *config_caps;
    config_caps.v2_config_fingerprint = sumeragi_config
        .v2_config(
            Duration::from_millis(s_params.sumeragi().block_time_ms()),
            frozen_mode,
        )?
        .fingerprint()
        .into();

    Ok((
        mode_tag.clone(),
        bls_domain,
        iroha_p2p::ConsensusHandshakeCaps {
            mode_tag,
            proto_version: PROTO_VERSION,
            consensus_fingerprint: fingerprint,
            config: config_caps,
        },
    ))
}

/// Derive consensus handshake capabilities (mode tag, BLS domain, fingerprint) from the current
/// state view and configuration.
pub fn compute_consensus_handshake_caps_from_view(
    view: &StateView<'_>,
    common_config: &CommonConfig,
    sumeragi_config: &SumeragiConfig,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
) -> Result<
    (String, String, iroha_p2p::ConsensusHandshakeCaps),
    iroha_config::parameters::actual::SumeragiV2ConfigError,
> {
    let height = u64::try_from(view.height()).unwrap_or(u64::MAX);
    compute_consensus_handshake_caps_from_world(
        view.world(),
        height,
        common_config,
        sumeragi_config,
        config_caps,
        frozen_mode,
    )
}

/// Handshake gate structure for p2p checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeGate {
    /// Local chain id.
    pub chain_id: ChainId,
    /// Local runtime mode tag.
    pub mode_tag: String,
    /// Local protocol version.
    pub proto_version: u32,
    /// Deterministic consensus fingerprint.
    pub consensus_fingerprint: [u8; 32],
}

/// Build canonical preimage for signing an RBC READY message.
pub fn rbc_ready_preimage(chain_id: &ChainId, mode_tag: &str, ready: &RbcReady) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 4 + 32 + 32);
    let domain = consensus_domain(chain_id, "RbcReady", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(ready.block_hash.as_ref().as_ref());
    out.extend_from_slice(&ready.height.to_be_bytes());
    out.extend_from_slice(&ready.view.to_be_bytes());
    out.extend_from_slice(&ready.epoch.to_be_bytes());
    out.extend_from_slice(ready.roster_hash.as_ref());
    out.extend_from_slice(ready.chunk_root.as_ref());
    out.extend_from_slice(&ready.sender.to_be_bytes());
    out
}

/// Build canonical preimage for signing an RBC DELIVER message.
pub fn rbc_deliver_preimage(chain_id: &ChainId, mode_tag: &str, deliver: &RbcDeliver) -> Vec<u8> {
    let ready_bytes = deliver
        .ready_signatures
        .iter()
        .map(|entry| {
            std::mem::size_of::<u32>()
                .saturating_add(std::mem::size_of::<u32>())
                .saturating_add(entry.signature.len())
        })
        .sum::<usize>();
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 4 + 32 + 32 + 4 + ready_bytes);
    let domain = consensus_domain(chain_id, "RbcDeliver", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(deliver.block_hash.as_ref().as_ref());
    out.extend_from_slice(&deliver.height.to_be_bytes());
    out.extend_from_slice(&deliver.view.to_be_bytes());
    out.extend_from_slice(&deliver.epoch.to_be_bytes());
    out.extend_from_slice(deliver.roster_hash.as_ref());
    out.extend_from_slice(deliver.chunk_root.as_ref());
    out.extend_from_slice(&deliver.sender.to_be_bytes());
    let ready_len = u32::try_from(deliver.ready_signatures.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&ready_len.to_be_bytes());
    for entry in &deliver.ready_signatures {
        out.extend_from_slice(&entry.sender.to_be_bytes());
        let sig_len = u32::try_from(entry.signature.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&sig_len.to_be_bytes());
        out.extend_from_slice(&entry.signature);
    }
    out
}

impl HandshakeGate {
    /// Build a local handshake gate tuple from chain id and consensus fingerprint.
    pub fn local(chain_id: ChainId, consensus_fingerprint: [u8; 32], mode_tag: &str) -> Self {
        Self {
            chain_id,
            mode_tag: mode_tag.to_string(),
            proto_version: PROTO_VERSION,
            consensus_fingerprint,
        }
    }

    /// Validate a peer handshake tuple. Returns Ok(()) on exact match; Err otherwise.
    /// Validate peer parameters from the handshake.
    ///
    /// # Errors
    /// Returns a descriptive string explaining the mismatch between the local
    /// configuration and the values reported by a peer.
    pub fn validate_peer(
        &self,
        peer_chain: &ChainId,
        peer_mode_tag: &str,
        peer_proto: u32,
        peer_fingerprint: &[u8; 32],
    ) -> Result<(), String> {
        if peer_chain != &self.chain_id {
            return Err(format!(
                "handshake rejected: expected chain_id `{}`, got `{}`",
                self.chain_id, peer_chain
            ));
        }
        if peer_mode_tag != self.mode_tag {
            return Err(format!(
                "handshake rejected: expected mode tag `{}`, got `{peer_mode_tag}`",
                self.mode_tag
            ));
        }
        if peer_proto != self.proto_version {
            return Err(format!(
                "handshake rejected: expected proto version {}, got {}",
                self.proto_version, peer_proto
            ));
        }
        if peer_fingerprint != &self.consensus_fingerprint {
            let expected_hex = hex::encode(self.consensus_fingerprint);
            let got_hex = hex::encode(peer_fingerprint);
            return Err(format!(
                "handshake rejected: consensus fingerprint mismatch (expected {expected_hex}, got {got_hex})"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1;

    fn checked_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("Sumeragi consensus fixture BLS key generation should succeed")
    }

    fn sample_validator_set(count: usize) -> Vec<PeerId> {
        (0..count)
            .map(|_| PeerId::new(checked_bls_keypair().public_key().clone()))
            .collect()
    }

    fn qc_with_raw_signers_bitmap(signers_bitmap: Vec<u8>) -> Qc {
        let validator_set = Vec::<PeerId>::new();
        let validator_set_hash = HashOf::new(&validator_set);
        Qc {
            phase: Phase::Commit,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [3u8; 32],
            )),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap,
                bls_aggregate_signature: vec![1],
            },
        }
    }

    #[test]
    fn qc_roundtrip_encode_decode() {
        let validator_set = sample_validator_set(16);
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0u8; 32],
            )),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 10,
            view: 7,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0xAA, 0x01],
                bls_aggregate_signature: vec![1, 2, 3],
            },
        };
        let bytes = qc.encode();
        let dec = Qc::decode(&mut &bytes[..]).expect("decode qc");
        assert_eq!(qc, dec);
    }

    #[test]
    fn qc_signer_count_formal_gate_matrix() {
        let cases: [(&str, &[u8], usize); 12] = [
            ("empty", &[], 0),
            ("zero_byte", &[0], 0),
            ("low_bit", &[1], 1),
            ("high_bit", &[128], 1),
            ("full_byte", &[255], 8),
            ("two_sparse", &[3, 5], 4),
            ("three_sparse", &[1, 2, 4], 3),
            ("padding_bits", &[240], 4),
            ("alternating_pair", &[170, 85], 8),
            ("two_full_bytes", &[255, 255], 16),
            ("three_zero_bytes", &[0, 0, 0], 0),
            ("mixed_three", &[15, 0, 240], 8),
        ];

        for (name, bitmap, expected) in cases {
            let qc = qc_with_raw_signers_bitmap(bitmap.to_vec());
            assert_eq!(qc_signer_count(&qc), expected, "{name}");
            assert!(
                expected <= bitmap.len().saturating_mul(8),
                "{name} count must fit inside the bitmap width"
            );
        }
    }

    #[test]
    fn qc_signer_count_counts_bits() {
        let validator_set = sample_validator_set(16);
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [1u8; 32],
            )),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0b1010_0101, 0b0000_0011],
                bls_aggregate_signature: vec![1, 2, 3],
            },
        };
        assert_eq!(qc_signer_count(&qc), 6);
    }

    #[test]
    fn qc_signer_count_empty_bitmap() {
        let validator_set = sample_validator_set(0);
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [2u8; 32],
            )),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: vec![9],
            },
        };
        assert_eq!(qc_signer_count(&qc), 0);
    }

    #[test]
    fn domain_depends_on_all_fields() {
        let cid_a = ChainId::from("iroha:test:A");
        let cid_b = ChainId::from("iroha:test:B");
        let d1 = consensus_domain(&cid_a, "Vote", b"x", PERMISSIONED_TAG);
        let d2 = consensus_domain(&cid_b, "Vote", b"x", PERMISSIONED_TAG);
        assert_ne!(d1, d2);
    }

    #[test]
    fn preimages_use_current_domain_tags() {
        let chain = ChainId::from("iroha:test:preimage-tags");
        let block_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([7u8; 32]));
        let roster_hash = iroha_crypto::Hash::prehashed([3u8; 32]);
        let chunk_root = iroha_crypto::Hash::prehashed([4u8; 32]);

        let vote = Vote {
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([1u8; 32]),
            post_state_root: iroha_crypto::Hash::prehashed([2u8; 32]),
            height: 11,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            phase: Phase::Prepare,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let vote_preimage = vote_preimage(&chain, PERMISSIONED_TAG, &vote);
        assert_eq!(
            &vote_preimage[..32],
            &consensus_domain(&chain, "Vote", b"v1", PERMISSIONED_TAG)
        );
        assert_ne!(
            &vote_preimage[..32],
            &consensus_domain(&chain, "Vote", b"legacy-v2", PERMISSIONED_TAG)
        );

        let vrf_commit = VrfCommit {
            epoch: 0,
            commitment: [0xA1; 32],
            signer: 3,
            bls_sig: Vec::new(),
        };
        let vrf_commit_preimage = vrf_commit_preimage(&chain, PERMISSIONED_TAG, &vrf_commit);
        assert_eq!(
            &vrf_commit_preimage[..32],
            &consensus_domain(&chain, "VrfCommit", b"v1", PERMISSIONED_TAG)
        );
        let vrf_reveal = VrfReveal {
            epoch: 0,
            reveal: [0xB2; 32],
            signer: 3,
            bls_sig: Vec::new(),
        };
        let vrf_reveal_preimage = vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &vrf_reveal);
        assert_eq!(
            &vrf_reveal_preimage[..32],
            &consensus_domain(&chain, "VrfReveal", b"v1", PERMISSIONED_TAG)
        );

        let ready = RbcReady {
            block_hash,
            height: 11,
            view: 2,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 1,
            signature: vec![0xAA],
        };
        let ready_preimage = rbc_ready_preimage(&chain, PERMISSIONED_TAG, &ready);
        assert_eq!(
            &ready_preimage[..32],
            &consensus_domain(&chain, "RbcReady", b"v1", PERMISSIONED_TAG)
        );
        assert_ne!(
            &ready_preimage[..32],
            &consensus_domain(&chain, "RbcReady", b"v2", PERMISSIONED_TAG)
        );

        let deliver = RbcDeliver {
            block_hash,
            height: 11,
            view: 2,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 1,
            signature: vec![0xBB],
            ready_signatures: Vec::new(),
        };
        let deliver_preimage = rbc_deliver_preimage(&chain, PERMISSIONED_TAG, &deliver);
        assert_eq!(
            &deliver_preimage[..32],
            &consensus_domain(&chain, "RbcDeliver", b"v1", PERMISSIONED_TAG)
        );
        assert_ne!(
            &deliver_preimage[..32],
            &consensus_domain(&chain, "RbcDeliver", b"v2", PERMISSIONED_TAG)
        );
    }

    #[test]
    fn vote_preimage_matches_formal_layout_and_excludes_signature_material() {
        let chain = ChainId::from("iroha:test:classic-vote-preimage-layout");
        let block_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x11; 32]));
        let parent_state_root = iroha_crypto::Hash::prehashed([0x12; 32]);
        let post_state_root = iroha_crypto::Hash::prehashed([0x13; 32]);
        let chain_order_hash = iroha_crypto::Hash::prehashed([0x14; 32]);
        let highest_block_hash =
            HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x15; 32]));
        let mut vote = Vote {
            block_hash,
            parent_state_root,
            post_state_root,
            height: 0x0102_0304_0506_0708,
            view: 0x1112_1314_1516_1718,
            epoch: 0x2122_2324_2526_2728,
            chain_order_hash,
            rechain_seq: 0x3132_3334_3536_3738,
            phase: Phase::Commit,
            highest_qc: None,
            signer: 0x4142_4344,
            bls_sig: vec![0xAA, 0xBB, 0xCC],
        };

        let mut expected_without_highest = Vec::new();
        expected_without_highest.extend_from_slice(&consensus_domain(
            &chain,
            "Vote",
            b"v1",
            PERMISSIONED_TAG,
        ));
        expected_without_highest.extend_from_slice(vote.block_hash.as_ref().as_ref());
        expected_without_highest.extend_from_slice(vote.parent_state_root.as_ref());
        expected_without_highest.extend_from_slice(vote.post_state_root.as_ref());
        expected_without_highest.extend_from_slice(&vote.height.to_be_bytes());
        expected_without_highest.extend_from_slice(&vote.view.to_be_bytes());
        expected_without_highest.extend_from_slice(&vote.epoch.to_be_bytes());
        expected_without_highest.extend_from_slice(vote.chain_order_hash.as_ref());
        expected_without_highest.extend_from_slice(&vote.rechain_seq.to_be_bytes());
        expected_without_highest.push(vote.phase as u8);
        expected_without_highest.push(0);

        assert_eq!(
            vote_preimage(&chain, PERMISSIONED_TAG, &vote),
            expected_without_highest
        );
        assert_ne!(
            vote_preimage(
                &ChainId::from("iroha:test:classic-vote-other-chain"),
                PERMISSIONED_TAG,
                &vote
            ),
            expected_without_highest,
            "chain id must be bound through the consensus domain"
        );
        assert_ne!(
            vote_preimage(&chain, NPOS_TAG, &vote),
            expected_without_highest,
            "mode tag must be bound through the consensus domain"
        );

        vote.signer = 0x5152_5354;
        vote.bls_sig = vec![0xDD, 0xEE, 0xFF, 0x00];
        assert_eq!(
            vote_preimage(&chain, PERMISSIONED_TAG, &vote),
            expected_without_highest,
            "mutable signer transport fields must stay outside the vote preimage"
        );

        vote.highest_qc = Some(QcRef {
            height: 0x6162_6364_6566_6768,
            view: 0x7172_7374_7576_7778,
            epoch: 0x8182_8384_8586_8788,
            subject_block_hash: highest_block_hash,
            phase: Phase::Prepare,
        });

        let mut expected_with_highest = expected_without_highest;
        *expected_with_highest
            .last_mut()
            .expect("highest flag should be present") = 1;
        let highest = vote.highest_qc.expect("highest qc");
        expected_with_highest.extend_from_slice(&highest.height.to_be_bytes());
        expected_with_highest.extend_from_slice(&highest.view.to_be_bytes());
        expected_with_highest.extend_from_slice(&highest.epoch.to_be_bytes());
        expected_with_highest.extend_from_slice(highest.subject_block_hash.as_ref().as_ref());
        expected_with_highest.push(highest.phase as u8);

        assert_eq!(
            vote_preimage(&chain, PERMISSIONED_TAG, &vote),
            expected_with_highest
        );
    }

    #[test]
    fn vrf_preimages_match_formal_layout_and_exclude_signatures() {
        let chain = ChainId::from("iroha:test:classic-vrf-preimage-layout");
        let mut commit = VrfCommit {
            epoch: 0x0102_0304_0506_0708,
            commitment: [0x21; 32],
            signer: 0x3132_3334,
            bls_sig: vec![0xAA, 0xBB],
        };
        let mut reveal = VrfReveal {
            epoch: 0x1112_1314_1516_1718,
            reveal: [0x41; 32],
            signer: 0x5152_5354,
            bls_sig: vec![0xCC, 0xDD],
        };

        let mut expected_commit = Vec::new();
        expected_commit.extend_from_slice(&consensus_domain(
            &chain,
            "VrfCommit",
            b"v1",
            PERMISSIONED_TAG,
        ));
        expected_commit.extend_from_slice(&commit.epoch.to_be_bytes());
        expected_commit.extend_from_slice(&commit.signer.to_be_bytes());
        expected_commit.extend_from_slice(&commit.commitment);
        assert_eq!(
            vrf_commit_preimage(&chain, PERMISSIONED_TAG, &commit),
            expected_commit
        );

        commit.bls_sig = vec![0x10, 0x11, 0x12];
        assert_eq!(
            vrf_commit_preimage(&chain, PERMISSIONED_TAG, &commit),
            expected_commit,
            "VRF commit signatures must stay outside the commit preimage"
        );

        let mut expected_reveal = Vec::new();
        expected_reveal.extend_from_slice(&consensus_domain(
            &chain,
            "VrfReveal",
            b"v1",
            PERMISSIONED_TAG,
        ));
        expected_reveal.extend_from_slice(&reveal.epoch.to_be_bytes());
        expected_reveal.extend_from_slice(&reveal.signer.to_be_bytes());
        expected_reveal.extend_from_slice(&reveal.reveal);
        assert_eq!(
            vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &reveal),
            expected_reveal
        );
        assert_ne!(
            expected_commit, expected_reveal,
            "VRF commit and reveal preimages must remain type-separated"
        );

        reveal.bls_sig = vec![0x20, 0x21, 0x22];
        assert_eq!(
            vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &reveal),
            expected_reveal,
            "VRF reveal signatures must stay outside the reveal preimage"
        );
    }

    #[test]
    fn rbc_ready_preimage_matches_formal_layout_and_excludes_signature() {
        let chain = ChainId::from("iroha:test:rbc-ready-preimage-layout");
        let block_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x31; 32]));
        let roster_hash = iroha_crypto::Hash::prehashed([0x32; 32]);
        let chunk_root = iroha_crypto::Hash::prehashed([0x33; 32]);
        let mut ready = RbcReady {
            block_hash,
            height: 0x0102_0304_0506_0708,
            view: 0x1112_1314_1516_1718,
            epoch: 0x2122_2324_2526_2728,
            roster_hash,
            chunk_root,
            sender: 0x4142_4344,
            signature: vec![0xAA, 0xBB, 0xCC],
        };

        let mut expected = Vec::new();
        expected.extend_from_slice(&consensus_domain(
            &chain,
            "RbcReady",
            b"v1",
            PERMISSIONED_TAG,
        ));
        expected.extend_from_slice(ready.block_hash.as_ref().as_ref());
        expected.extend_from_slice(&ready.height.to_be_bytes());
        expected.extend_from_slice(&ready.view.to_be_bytes());
        expected.extend_from_slice(&ready.epoch.to_be_bytes());
        expected.extend_from_slice(ready.roster_hash.as_ref());
        expected.extend_from_slice(ready.chunk_root.as_ref());
        expected.extend_from_slice(&ready.sender.to_be_bytes());

        assert_eq!(
            rbc_ready_preimage(&chain, PERMISSIONED_TAG, &ready),
            expected
        );

        ready.signature = vec![0xDD, 0xEE, 0xFF, 0x00];
        assert_eq!(
            rbc_ready_preimage(&chain, PERMISSIONED_TAG, &ready),
            expected
        );
    }

    #[test]
    fn rbc_deliver_preimage_matches_formal_layout_and_excludes_signature() {
        let chain = ChainId::from("iroha:test:rbc-deliver-preimage-layout");
        let block_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x41; 32]));
        let roster_hash = iroha_crypto::Hash::prehashed([0x42; 32]);
        let chunk_root = iroha_crypto::Hash::prehashed([0x43; 32]);
        let mut deliver = RbcDeliver {
            block_hash,
            height: 0x0102_0304_0506_0708,
            view: 0x1112_1314_1516_1718,
            epoch: 0x2122_2324_2526_2728,
            roster_hash,
            chunk_root,
            sender: 0x5152_5354,
            signature: vec![0xAA, 0xBB, 0xCC],
            ready_signatures: vec![
                RbcReadySignature {
                    sender: 0x6162_6364,
                    signature: vec![0x10, 0x11, 0x12],
                },
                RbcReadySignature {
                    sender: 0x7172_7374,
                    signature: vec![0x20, 0x21],
                },
            ],
        };

        let mut expected = Vec::new();
        expected.extend_from_slice(&consensus_domain(
            &chain,
            "RbcDeliver",
            b"v1",
            PERMISSIONED_TAG,
        ));
        expected.extend_from_slice(deliver.block_hash.as_ref().as_ref());
        expected.extend_from_slice(&deliver.height.to_be_bytes());
        expected.extend_from_slice(&deliver.view.to_be_bytes());
        expected.extend_from_slice(&deliver.epoch.to_be_bytes());
        expected.extend_from_slice(deliver.roster_hash.as_ref());
        expected.extend_from_slice(deliver.chunk_root.as_ref());
        expected.extend_from_slice(&deliver.sender.to_be_bytes());
        expected.extend_from_slice(&2_u32.to_be_bytes());
        expected.extend_from_slice(&0x6162_6364_u32.to_be_bytes());
        expected.extend_from_slice(&3_u32.to_be_bytes());
        expected.extend_from_slice(&[0x10, 0x11, 0x12]);
        expected.extend_from_slice(&0x7172_7374_u32.to_be_bytes());
        expected.extend_from_slice(&2_u32.to_be_bytes());
        expected.extend_from_slice(&[0x20, 0x21]);

        assert_eq!(
            rbc_deliver_preimage(&chain, PERMISSIONED_TAG, &deliver),
            expected
        );

        deliver.signature = vec![0xDD, 0xEE, 0xFF, 0x00];
        assert_eq!(
            rbc_deliver_preimage(&chain, PERMISSIONED_TAG, &deliver),
            expected
        );

        deliver.ready_signatures.swap(0, 1);
        assert_ne!(
            rbc_deliver_preimage(&chain, PERMISSIONED_TAG, &deliver),
            expected
        );
    }

    #[test]
    fn vote_preimage_binds_chain_order() {
        let chain = ChainId::from("iroha:test:chain-order-binding");
        let block_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([7u8; 32]));
        let vote = Vote {
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([1u8; 32]),
            post_state_root: iroha_crypto::Hash::prehashed([2u8; 32]),
            height: 11,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            phase: Phase::Prepare,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let base = vote_preimage(&chain, PERMISSIONED_TAG, &vote);

        let mut changed_order = vote.clone();
        changed_order.chain_order_hash = iroha_crypto::Hash::new(b"alternate-chain-order");
        assert_ne!(
            base,
            vote_preimage(&chain, PERMISSIONED_TAG, &changed_order)
        );

        let mut changed_seq = vote;
        changed_seq.rechain_seq = 1;
        assert_ne!(base, vote_preimage(&chain, PERMISSIONED_TAG, &changed_seq));
    }

    #[test]
    fn handshake_gate_validates_mismatches() {
        let chain = ChainId::from("iroha:test:gate");
        let fp = [9u8; 32];
        let gate = HandshakeGate::local(chain.clone(), fp, PERMISSIONED_TAG);
        // OK path
        assert!(
            gate.validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &fp)
                .is_ok()
        );
        // Mismatch
        let other = ChainId::from("iroha:test:other");
        let err = gate
            .validate_peer(&other, PERMISSIONED_TAG, PROTO_VERSION, &fp)
            .expect_err("chain id mismatch must be rejected");
        assert_eq!(
            err,
            "handshake rejected: expected chain_id `iroha:test:gate`, got `iroha:test:other`"
        );
    }

    #[test]
    fn handshake_fingerprint_changes_with_mode() {
        let chain = ChainId::from("iroha:test:cutover");
        let permissioned_params = ConsensusGenesisParams {
            block_time_ms: 1000,
            commit_time_ms: 1000,
            min_finality_ms: 100,
            max_clock_drift_ms: 500,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v2".to_string(),
            npos: None,
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };
        let npos_params = ConsensusGenesisParams {
            block_time_ms: 1000,
            commit_time_ms: 1000,
            min_finality_ms: 100,
            max_clock_drift_ms: 500,
            collectors_k: 3,
            redundant_send_r: 2,
            block_max_transactions: 1024,
            da_enabled: true,
            epoch_length_blocks: 10,
            bls_domain: "bls-iroha2:npos-sumeragi:v2".to_string(),
            npos: Some(NposGenesisParams {
                block_time_ms: 1000,
                timeout_propose_ms: 300,
                timeout_prevote_ms: 300,
                timeout_precommit_ms: 250,
                timeout_commit_ms: 200,
                timeout_da_ms: 300,
                timeout_aggregator_ms: 120,
                k_aggregators: 3,
                redundant_send_r: 2,
                epoch_seed: [0u8; 32],
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                max_validators: 32,
                min_self_bond: 1,
                min_nomination_bond: 1,
                max_nominator_concentration_pct: 25,
                seat_band_pct: 5,
                max_entity_correlation_pct: 25,
                finality_margin_blocks: 8,
                evidence_horizon_blocks: 50,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 9,
            }),
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };

        let fp_permissioned = compute_consensus_fingerprint_from_params(
            &chain,
            &permissioned_params,
            PERMISSIONED_TAG,
        );
        let fp_npos = compute_consensus_fingerprint_from_params(&chain, &npos_params, NPOS_TAG);

        assert_ne!(
            fp_permissioned, fp_npos,
            "fingerprints must differ across mode tags/domains"
        );

        let gate_permissioned =
            HandshakeGate::local(chain.clone(), fp_permissioned, PERMISSIONED_TAG);
        assert!(
            gate_permissioned
                .validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &fp_permissioned)
                .is_ok(),
            "permissioned gate should accept matching tag/fingerprint"
        );
        assert!(
            gate_permissioned
                .validate_peer(&chain, NPOS_TAG, PROTO_VERSION, &fp_npos)
                .is_err(),
            "permissioned gate should reject NPoS tag/fingerprint"
        );

        let gate_npos = HandshakeGate::local(chain.clone(), fp_npos, NPOS_TAG);
        assert!(
            gate_npos
                .validate_peer(&chain, NPOS_TAG, PROTO_VERSION, &fp_npos)
                .is_ok(),
            "npos gate should accept matching tag/fingerprint"
        );
        assert!(
            gate_npos
                .validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &fp_permissioned)
                .is_err(),
            "npos gate should reject permissioned tag/fingerprint"
        );
    }

    #[test]
    fn compute_fingerprint_stable() {
        let chain = ChainId::from("iroha:test:fp");
        let a = compute_consensus_fingerprint(
            &chain,
            "{}",
            "bls-iroha2:test",
            &[PROTO_VERSION],
            PERMISSIONED_TAG,
        );
        let b = compute_consensus_fingerprint(
            &chain,
            "{}",
            "bls-iroha2:test",
            &[PROTO_VERSION],
            PERMISSIONED_TAG,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn canonical_v2_fingerprint_ignores_retired_v1_fields() {
        let chain = ChainId::from("iroha:test:fpcanon");
        let mut p = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            min_finality_ms: 100,
            max_clock_drift_ms: 1000,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v2".to_string(),
            npos: None,
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };
        let a = compute_consensus_fingerprint_from_params(&chain, &p, PERMISSIONED_TAG);
        p.commit_time_ms += 1;
        p.min_finality_ms += 1;
        p.max_clock_drift_ms += 1;
        p.collectors_k = 2;
        p.redundant_send_r = 2;
        p.da_enabled = true;
        p.bls_domain = "retired-arbitrary-domain".to_owned();
        let b = compute_consensus_fingerprint_from_params(&chain, &p, PERMISSIONED_TAG);
        assert_eq!(a, b);

        let legacy = compute_legacy_consensus_fingerprint_from_params(&chain, &p, PERMISSIONED_TAG);
        let mut original = p;
        original.collectors_k = 1;
        assert_ne!(
            legacy,
            compute_legacy_consensus_fingerprint_from_params(&chain, &original, PERMISSIONED_TAG,),
            "archive-only full projection must retain the old field set",
        );
    }

    #[test]
    fn canonical_fingerprint_binds_v2_protocol_and_round_timeout() {
        let chain = ChainId::from("iroha:test:fpcanon-v2-timeout");
        let params = ConsensusGenesisParams {
            block_time_ms: 1_000,
            commit_time_ms: 1_000,
            min_finality_ms: 1_000,
            max_clock_drift_ms: 1_000,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1_024,
            da_enabled: true,
            epoch_length_blocks: 0,
            bls_domain: iroha_data_model::block::consensus_v2::PERMISSIONED_BLS_DOMAIN.to_owned(),
            npos: None,
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };
        let baseline = compute_consensus_fingerprint_from_params(&chain, &params, PERMISSIONED_TAG);
        let changed_timeout = compute_consensus_fingerprint_from_params(
            &chain,
            &ConsensusGenesisParams {
                round_timeout_ms: 10_001,
                ..params.clone()
            },
            PERMISSIONED_TAG,
        );
        let changed_protocol = compute_consensus_fingerprint_from_params(
            &chain,
            &ConsensusGenesisParams {
                protocol_version: PROTO_VERSION.saturating_add(1),
                ..params
            },
            PERMISSIONED_TAG,
        );

        assert_ne!(baseline, changed_timeout);
        assert_ne!(baseline, changed_protocol);
    }

    #[test]
    fn canonical_v2_npos_fingerprint_ignores_phase_timeouts_but_binds_election_seed() {
        let chain = ChainId::from("iroha:test:fpcanon-npos");
        let mut p = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            min_finality_ms: 100,
            max_clock_drift_ms: 1000,
            collectors_k: 3,
            redundant_send_r: 2,
            block_max_transactions: 1024,
            da_enabled: true,
            epoch_length_blocks: 3600,
            bls_domain: "bls-iroha2:npos-sumeragi:v2".to_string(),
            npos: Some(NposGenesisParams {
                block_time_ms: 1_000,
                timeout_propose_ms: 250,
                timeout_prevote_ms: 250,
                timeout_precommit_ms: 250,
                timeout_commit_ms: 250,
                timeout_da_ms: 300,
                timeout_aggregator_ms: 150,
                k_aggregators: 3,
                redundant_send_r: 2,
                epoch_seed: [0u8; 32],
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                max_validators: 128,
                min_self_bond: 1_000,
                min_nomination_bond: 1,
                max_nominator_concentration_pct: 40,
                seat_band_pct: 15,
                max_entity_correlation_pct: 25,
                finality_margin_blocks: 8,
                evidence_horizon_blocks: 7_200,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 9,
            }),
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };
        let a = compute_consensus_fingerprint_from_params(&chain, &p, NPOS_TAG);
        let npos = p.npos.as_mut().expect("NPoS parameters");
        npos.block_time_ms += 1;
        npos.timeout_propose_ms += 1;
        npos.timeout_prevote_ms += 1;
        npos.timeout_precommit_ms += 1;
        npos.timeout_commit_ms += 1;
        npos.timeout_da_ms += 1;
        npos.timeout_aggregator_ms += 1;
        npos.k_aggregators += 1;
        npos.redundant_send_r += 1;
        let retired_fields_changed =
            compute_consensus_fingerprint_from_params(&chain, &p, NPOS_TAG);
        assert_eq!(a, retired_fields_changed);

        p.npos.as_mut().expect("NPoS parameters").epoch_seed[0] = 1;
        let election_seed_changed = compute_consensus_fingerprint_from_params(&chain, &p, NPOS_TAG);
        assert_ne!(a, election_seed_changed);
    }

    #[test]
    fn handshake_gate_rejects_on_fingerprint_mismatch() {
        let chain = ChainId::from("iroha:test:hshake");
        let p1 = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            min_finality_ms: 100,
            max_clock_drift_ms: 1000,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v2".to_string(),
            npos: None,
            protocol_version: PROTO_VERSION,
            round_timeout_ms: 10_000,
            v2_context: Some(
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            ),
        };
        let mut p2 = p1.clone();
        p2.v2_context
            .as_mut()
            .expect("signed v2 context")
            .nexus_amx_context_hash[0] ^= 1;
        let f1 = compute_consensus_fingerprint_from_params(&chain, &p1, PERMISSIONED_TAG);
        let f2 = compute_consensus_fingerprint_from_params(&chain, &p2, PERMISSIONED_TAG);
        let gate = HandshakeGate::local(chain.clone(), f1, PERMISSIONED_TAG);
        assert!(
            gate.validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f1)
                .is_ok()
        );

        let retired_only = ConsensusGenesisParams {
            commit_time_ms: p1.commit_time_ms + 1,
            min_finality_ms: p1.min_finality_ms + 1,
            collectors_k: p1.collectors_k + 1,
            redundant_send_r: p1.redundant_send_r + 1,
            da_enabled: !p1.da_enabled,
            ..p1.clone()
        };
        let retired_only_fingerprint =
            compute_consensus_fingerprint_from_params(&chain, &retired_only, PERMISSIONED_TAG);
        assert_eq!(f1, retired_only_fingerprint);
        assert!(
            gate.validate_peer(
                &chain,
                PERMISSIONED_TAG,
                PROTO_VERSION,
                &retired_only_fingerprint,
            )
            .is_ok(),
            "retired fields must not partition live v2 admission",
        );

        let err = gate
            .validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f2)
            .expect_err("signed Nexus/AMX context mismatch must be rejected");
        let expected_hex = hex::encode(f1);
        let got_hex = hex::encode(f2);
        assert_eq!(
            err,
            format!(
                "handshake rejected: consensus fingerprint mismatch (expected {expected_hex}, got {got_hex})"
            )
        );
    }

    #[test]
    fn exec_witness_roundtrip_codec() {
        // Build a small witness with two reads and one write
        let w = ExecWitness {
            reads: vec![
                ExecKv {
                    key: b"key:read:1".to_vec(),
                    value: b"value-pre-1".to_vec(),
                },
                ExecKv {
                    key: b"key:read:2".to_vec(),
                    value: b"value-pre-2".to_vec(),
                },
            ],
            writes: vec![ExecKv {
                key: b"key:write:1".to_vec(),
                value: b"value-post-1".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let bytes = w.encode();
        let dec = ExecWitness::decode(&mut &bytes[..]).expect("decode witness");
        assert_eq!(w, dec);
    }
}
