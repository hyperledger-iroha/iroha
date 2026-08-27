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
use iroha_config::parameters::actual::Sumeragi as SumeragiConfig;
#[cfg(test)]
use iroha_crypto::HashOf;
pub use iroha_data_model::block::consensus::{
    CertPhase, ConsensusBlockHeader, ConsensusGenesisModeParams, ConsensusGenesisParams, Evidence,
    ExecKv, ExecWitness, ExecWitnessMsg, Height, LaneBlockCertificateV1, LaneBlockDescriptorV1,
    LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1, LaneBlockVoteBodyV1,
    NposGenesisParams, Proposal, Qc, QcAggregate, QcRef, QcVote, ValidatorIndex, View,
    default_chain_order_hash,
};
#[cfg(test)]
pub use iroha_data_model::block::consensus::{VrfCommit, VrfReveal};
/// Live consensus protocol revision.
pub const PROTO_VERSION: u32 = iroha_data_model::block::consensus_v2::PROTOCOL_VERSION as u32;
/// Permissioned Sumeragi v2 handshake and signing-domain tag.
pub const PERMISSIONED_TAG: &str = iroha_data_model::block::consensus_v2::PERMISSIONED_TAG;
/// NPoS Sumeragi v2 handshake and signing-domain tag.
pub const NPOS_TAG: &str = iroha_data_model::block::consensus_v2::NPOS_TAG;
/// Commit-certificate phase (prepare/commit/new-view).
pub type Phase = CertPhase;
/// Runtime adapter vote used for certificate aggregation.
pub type Vote = QcVote;
/// Reference to a QC header carried in hints.
pub type QcHeaderRef = QcRef;
use crate::state::{StateView, WorldReadOnly};
use iroha_data_model::parameter::system::SumeragiNposParameters;
use iroha_data_model::prelude::*;
/// Count the number of validators encoded into a QC signer bitmap.
pub fn qc_signer_count(qc: &Qc) -> usize {
    qc.aggregate
        .signers_bitmap
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum()
}
/// Build the canonical preimage for a QC vote signature under the given chain and mode tag.
pub fn vote_preimage(network_id: &NetworkId, mode_tag: &str, v: &Vote) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 * 4 + 8 * 6 + 3);
    let domain = consensus_domain(network_id, "Vote", b"v1", mode_tag);
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
#[cfg(test)]
pub fn vrf_commit_preimage(network_id: &NetworkId, mode_tag: &str, c: &VrfCommit) -> Vec<u8> {
    vrf_commit_preimage_fields(network_id, mode_tag, c.epoch, c.signer, &c.commitment)
}
/// Build the canonical preimage for a versioned-v2 VRF commitment.
#[cfg(test)]
pub fn v2_vrf_commit_preimage(
    network_id: &NetworkId,
    mode_tag: &str,
    commit: &iroha_data_model::block::consensus_v2::VrfCommit,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32);
    let domain = consensus_domain(network_id, "VrfCommit", b"v2", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&commit.epoch.to_be_bytes());
    out.extend_from_slice(&commit.signer.to_be_bytes());
    out.extend_from_slice(&commit.commitment);
    out
}
#[cfg(test)]
fn vrf_commit_preimage_fields(
    network_id: &NetworkId,
    mode_tag: &str,
    epoch: u64,
    signer: u32,
    commitment: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32);
    let domain = consensus_domain(network_id, "VrfCommit", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&epoch.to_be_bytes());
    out.extend_from_slice(&signer.to_be_bytes());
    out.extend_from_slice(commitment);
    out
}
/// Build the canonical preimage for a VRF reveal signature under the given chain and mode tag.
#[cfg(test)]
pub fn vrf_reveal_preimage(network_id: &NetworkId, mode_tag: &str, r: &VrfReveal) -> Vec<u8> {
    vrf_reveal_preimage_fields(network_id, mode_tag, r.epoch, r.signer, &r.reveal)
}
/// Build the canonical preimage for a versioned-v2 VRF reveal.
#[cfg(test)]
pub fn v2_vrf_reveal_preimage(
    network_id: &NetworkId,
    mode_tag: &str,
    reveal: &iroha_data_model::block::consensus_v2::VrfReveal,
) -> Vec<u8> {
    let proof_len = u64::try_from(reveal.vrf_proof.len()).unwrap_or(u64::MAX);
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32 + 8 + reveal.vrf_proof.len());
    let domain = consensus_domain(network_id, "VrfReveal", b"v2", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&reveal.epoch.to_be_bytes());
    out.extend_from_slice(&reveal.signer.to_be_bytes());
    out.extend_from_slice(&reveal.reveal);
    out.extend_from_slice(&proof_len.to_be_bytes());
    out.extend_from_slice(&reveal.vrf_proof);
    out
}
#[cfg(test)]
fn vrf_reveal_preimage_fields(
    network_id: &NetworkId,
    mode_tag: &str,
    epoch: u64,
    signer: u32,
    reveal: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 8 + 4 + 32);
    let domain = consensus_domain(network_id, "VrfReveal", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&epoch.to_be_bytes());
    out.extend_from_slice(&signer.to_be_bytes());
    out.extend_from_slice(reveal);
    out
}
/// Canonical preimage helpers for BLS signing (same-message across signers).
#[cfg(feature = "bls")]
pub mod bls_preimage {
    use super::*;
    /// Build the canonical preimage for a Vote signature under the given chain and mode tag.
    pub fn vote(network_id: &NetworkId, mode_tag: &str, v: &Vote) -> Vec<u8> {
        super::vote_preimage(network_id, mode_tag, v)
    }
    /// Build the canonical preimage for a VRF commit signature.
    #[cfg(test)]
    pub fn vrf_commit(network_id: &NetworkId, mode_tag: &str, c: &VrfCommit) -> Vec<u8> {
        super::vrf_commit_preimage(network_id, mode_tag, c)
    }
    /// Build the canonical preimage for a VRF reveal signature.
    #[cfg(test)]
    pub fn vrf_reveal(network_id: &NetworkId, mode_tag: &str, r: &VrfReveal) -> Vec<u8> {
        super::vrf_reveal_preimage(network_id, mode_tag, r)
    }
}
/// Domain separation helper for signable payloads.
/// Returns a 32‑byte Blake2b digest of the domain preimage.
pub fn consensus_domain(
    network_id: &NetworkId,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, b"iroha-sumeragi-consensus/v1");
    iroha_crypto::blake2::digest::Update::update(&mut hasher, network_id.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &PROTO_VERSION.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, message_type_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, extra);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}
/// Compute the genesis-embedded v2 consensus-parameters fingerprint.
///
/// The projection deliberately omits v1 collectors, phase-specific and
/// adaptive timeouts, the global-RBC switch, and any local fallback. Mode,
/// cadence, block bound, signed DA/Nexus context,
/// and the genesis-selected NPoS election inputs are canonical Norito fields.
/// Mutable shared adapter settings are committed separately by
/// [`SumeragiConfig::v2_config`].
pub fn compute_consensus_parameters_fingerprint(
    params: &ConsensusGenesisParams,
) -> Result<[u8; 32], String> {
    iroha_data_model::block::consensus_v2::fingerprint::compute(params)
}
/// Build the exact first-release carrier for consensus-genesis parameters.
///
/// Runtime handshakes, genesis metadata generation, and startup validation all
/// pass this carrier through the canonical v2 fingerprint projection.
///
/// # Errors
/// Returns an error when NPoS mode lacks its signed election parameters.
pub fn consensus_genesis_params_from_parameters(
    mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    params: &iroha_data_model::parameter::Parameters,
    v2_context: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
) -> Result<ConsensusGenesisParams, &'static str> {
    let sumeragi = params.sumeragi();
    let block = params.block();
    let npos_payload = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);
    let mode = match mode {
        iroha_data_model::block::consensus_v2::ConsensusMode::Npos => {
            let npos = npos_payload.ok_or("NPoS genesis requires `sumeragi_npos_parameters`")?;
            ConsensusGenesisModeParams::Npos(NposGenesisParams {
                epoch_length_blocks: npos.epoch_length_blocks(),
                epoch_seed: npos.epoch_seed(),
                vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                max_validators: npos.max_validators(),
                min_self_bond: npos.min_self_bond().clone(),
                min_nomination_bond: npos.min_nomination_bond().clone(),
                max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                seat_band_pct: npos.seat_band_pct(),
                max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                finality_margin_blocks: npos.finality_margin_blocks(),
                evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                activation_lag_blocks: npos.activation_lag_blocks(),
                slashing_delay_blocks: npos.slashing_delay_blocks(),
            })
        }
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned => {
            if npos_payload.is_some() {
                return Err("permissioned genesis must omit `sumeragi_npos_parameters`");
            }
            ConsensusGenesisModeParams::Permissioned
        }
    };
    Ok(ConsensusGenesisParams {
        block_cadence_ms: sumeragi.block_cadence_ms(),
        block_max_transactions: block.max_transactions(),
        mode,
        protocol_version: u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION),
        v2_context,
    })
}
/// Derive consensus handshake capabilities (mode tag, BLS domain, fingerprint) from a world
/// snapshot, committed height, and local configuration.
#[allow(clippy::too_many_lines)]
pub fn compute_consensus_handshake_caps_from_world(
    world: &impl WorldReadOnly,
    _height: u64,
    sumeragi_config: &SumeragiConfig,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    signed_v2_context: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
) -> Result<(String, String, iroha_p2p::ConsensusHandshakeCaps), String> {
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
    let canon = consensus_genesis_params_from_parameters(frozen_mode, s_params, signed_v2_context)
        .map_err(str::to_owned)?;
    let fingerprint = compute_consensus_parameters_fingerprint(&canon)?;
    let mut config_caps = *config_caps;
    config_caps.execution_policy_hash = signed_v2_context.execution_policy_hash;
    config_caps.v2_config_fingerprint = sumeragi_config
        .v2_config(s_params.sumeragi().block_cadence(), frozen_mode)
        .map_err(|error| error.to_string())?
        .fingerprint()
        .into();
    Ok((
        mode_tag.clone(),
        bls_domain,
        iroha_p2p::ConsensusHandshakeCaps {
            mode: frozen_mode,
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
    sumeragi_config: &SumeragiConfig,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    signed_v2_context: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
) -> Result<(String, String, iroha_p2p::ConsensusHandshakeCaps), String> {
    let height = u64::try_from(view.height()).unwrap_or(u64::MAX);
    compute_consensus_handshake_caps_from_world(
        view.world(),
        height,
        sumeragi_config,
        config_caps,
        frozen_mode,
        signed_v2_context,
    )
}
/// Handshake gate structure for p2p checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeGate {
    /// Local exact genesis-derived network id.
    pub network_id: NetworkId,
    /// Local runtime mode tag.
    pub mode_tag: String,
    /// Local protocol version.
    pub proto_version: u32,
    /// Deterministic genesis-embedded consensus-parameters fingerprint.
    pub parameters_fingerprint: [u8; 32],
}
impl HandshakeGate {
    /// Build a local handshake gate from an exact network id and parameters fingerprint.
    pub fn local(network_id: NetworkId, parameters_fingerprint: [u8; 32], mode_tag: &str) -> Self {
        Self {
            network_id,
            mode_tag: mode_tag.to_string(),
            proto_version: PROTO_VERSION,
            parameters_fingerprint,
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
        peer_network_id: &NetworkId,
        peer_mode_tag: &str,
        peer_proto: u32,
        peer_parameters_fingerprint: &[u8; 32],
    ) -> Result<(), String> {
        if peer_network_id != &self.network_id {
            return Err(format!(
                "handshake rejected: expected network_id `{}`, got `{}`",
                self.network_id, peer_network_id
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
        if peer_parameters_fingerprint != &self.parameters_fingerprint {
            let expected_hex = hex::encode(self.parameters_fingerprint);
            let got_hex = hex::encode(peer_parameters_fingerprint);
            return Err(format!(
                "handshake rejected: consensus-parameters fingerprint mismatch (expected {expected_hex}, got {got_hex})"
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
    fn test_network_id(seed: &str) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new(seed.as_bytes()),
        ))
    }
    fn checked_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("Sumeragi consensus fixture BLS key generation should succeed")
    }
    fn sample_validator_set(count: usize) -> Vec<PeerId> {
        (0..count)
            .map(|_| PeerId::new(checked_bls_keypair().public_key().clone()))
            .collect()
    }
    fn permissioned_genesis_params() -> ConsensusGenesisParams {
        ConsensusGenesisParams {
            block_cadence_ms: core::num::NonZeroU64::new(1_000)
                .expect("test cadence must be non-zero"),
            block_max_transactions: core::num::NonZeroU64::new(1_024)
                .expect("test block bound must be non-zero"),
            mode: ConsensusGenesisModeParams::Permissioned,
            protocol_version: PROTO_VERSION,
            v2_context:
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
        }
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
        let cid_a = test_network_id("iroha:test:A");
        let cid_b = test_network_id("iroha:test:B");
        let d1 = consensus_domain(&cid_a, "Vote", b"x", PERMISSIONED_TAG);
        let d2 = consensus_domain(&cid_b, "Vote", b"x", PERMISSIONED_TAG);
        assert_ne!(d1, d2);
    }
    #[test]
    fn preimages_use_current_domain_tags() {
        let chain = test_network_id("iroha:test:preimage-tags");
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
        let vote_preimage = vote_preimage(&chain, PERMISSIONED_TAG, &vote);
        assert_eq!(
            &vote_preimage[..32],
            &consensus_domain(&chain, "Vote", b"v1", PERMISSIONED_TAG)
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
    }
    #[test]
    fn vote_preimage_matches_formal_layout_and_excludes_signature_material() {
        let chain = test_network_id("iroha:test:classic-vote-preimage-layout");
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
                &test_network_id("iroha:test:classic-vote-other-chain"),
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
        let chain = test_network_id("iroha:test:classic-vrf-preimage-layout");
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
        let mut v2_commit = iroha_data_model::block::consensus_v2::VrfCommit {
            epoch: commit.epoch,
            commitment: commit.commitment,
            signer: commit.signer,
            bls_sig: vec![0xFE],
        };
        let mut expected_v2_commit = Vec::new();
        expected_v2_commit.extend_from_slice(&consensus_domain(
            &chain,
            "VrfCommit",
            b"v2",
            PERMISSIONED_TAG,
        ));
        expected_v2_commit.extend_from_slice(&v2_commit.epoch.to_be_bytes());
        expected_v2_commit.extend_from_slice(&v2_commit.signer.to_be_bytes());
        expected_v2_commit.extend_from_slice(&v2_commit.commitment);
        assert_eq!(
            v2_vrf_commit_preimage(&chain, PERMISSIONED_TAG, &v2_commit),
            expected_v2_commit,
            "the versioned carrier must bind its own domain and canonical fields",
        );
        commit.bls_sig = vec![0x10, 0x11, 0x12];
        v2_commit.bls_sig = vec![0x13, 0x14];
        assert_eq!(
            vrf_commit_preimage(&chain, PERMISSIONED_TAG, &commit),
            expected_commit,
            "VRF commit signatures must stay outside the commit preimage"
        );
        assert_eq!(
            v2_vrf_commit_preimage(&chain, PERMISSIONED_TAG, &v2_commit),
            expected_v2_commit,
            "versioned VRF commit signatures must stay outside the commit preimage",
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
        let mut v2_reveal = iroha_data_model::block::consensus_v2::VrfReveal {
            epoch: reveal.epoch,
            reveal: reveal.reveal,
            signer: reveal.signer,
            vrf_proof: vec![0x31, 0x32],
            bls_sig: vec![0xFD],
        };
        let mut expected_v2_reveal = Vec::new();
        expected_v2_reveal.extend_from_slice(&consensus_domain(
            &chain,
            "VrfReveal",
            b"v2",
            PERMISSIONED_TAG,
        ));
        expected_v2_reveal.extend_from_slice(&v2_reveal.epoch.to_be_bytes());
        expected_v2_reveal.extend_from_slice(&v2_reveal.signer.to_be_bytes());
        expected_v2_reveal.extend_from_slice(&v2_reveal.reveal);
        expected_v2_reveal.extend_from_slice(
            &u64::try_from(v2_reveal.vrf_proof.len())
                .unwrap()
                .to_be_bytes(),
        );
        expected_v2_reveal.extend_from_slice(&v2_reveal.vrf_proof);
        assert_eq!(
            v2_vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &v2_reveal),
            expected_v2_reveal,
            "the versioned carrier must bind its proof and canonical fields",
        );
        assert_ne!(
            expected_commit, expected_reveal,
            "VRF commit and reveal preimages must remain type-separated"
        );
        reveal.bls_sig = vec![0x20, 0x21, 0x22];
        v2_reveal.bls_sig = vec![0x23, 0x24];
        assert_eq!(
            vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &reveal),
            expected_reveal,
            "VRF reveal signatures must stay outside the reveal preimage"
        );
        assert_eq!(
            v2_vrf_reveal_preimage(&chain, PERMISSIONED_TAG, &v2_reveal),
            expected_v2_reveal,
            "versioned VRF reveal signatures must stay outside the reveal preimage",
        );
    }
    #[test]
    fn vote_preimage_binds_chain_order() {
        let chain = test_network_id("iroha:test:chain-order-binding");
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
    fn handshake_gate_rejects_same_name_same_config_different_genesis() {
        let display_name = iroha_data_model::ChainId::from("shared-display-name");
        let chain = test_network_id("iroha:test:genesis-a");
        let fp = [9u8; 32];
        let gate = HandshakeGate::local(chain.clone(), fp, PERMISSIONED_TAG);
        // OK path
        assert!(
            gate.validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &fp)
                .is_ok()
        );
        // Mismatch
        let other = test_network_id("iroha:test:genesis-b");
        let err = gate
            .validate_peer(&other, PERMISSIONED_TAG, PROTO_VERSION, &fp)
            .expect_err("chain id mismatch must be rejected");
        assert!(err.starts_with("handshake rejected: expected network_id `"));
        assert!(err.contains(&chain.to_string()));
        assert!(err.contains(&other.to_string()));
        assert_eq!(display_name.as_str(), "shared-display-name");
    }
    #[test]
    fn handshake_fingerprint_changes_with_mode() {
        let chain = test_network_id("iroha:test:cutover");
        let permissioned_params = permissioned_genesis_params();
        let npos_params = ConsensusGenesisParams {
            mode: ConsensusGenesisModeParams::Npos(NposGenesisParams {
                epoch_length_blocks: core::num::NonZeroU64::new(3_600)
                    .expect("test epoch length must be non-zero"),
                epoch_seed: [7; 32],
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                max_validators: 31,
                min_self_bond: 1_u64.into(),
                min_nomination_bond: 1_u64.into(),
                max_nominator_concentration_pct: 25,
                seat_band_pct: 5,
                max_entity_correlation_pct: 25,
                finality_margin_blocks: 8,
                evidence_horizon_blocks: 50,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 9,
            }),
            ..permissioned_params
        };
        let fp_permissioned = compute_consensus_parameters_fingerprint(&permissioned_params)
            .expect("permissioned fixture must fingerprint");
        let fp_npos = compute_consensus_parameters_fingerprint(&npos_params)
            .expect("NPoS fixture must fingerprint");
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
    fn canonical_fingerprint_binds_v2_protocol_and_context() {
        let params = permissioned_genesis_params();
        let baseline = compute_consensus_parameters_fingerprint(&params)
            .expect("canonical fixture must fingerprint");
        let mut changed_context = params.clone();
        changed_context.v2_context.nexus_amx_context_hash[0] ^= 1;
        let changed_context = compute_consensus_parameters_fingerprint(&changed_context)
            .expect("changed signed context must remain valid");
        let changed_protocol = ConsensusGenesisParams {
            protocol_version: PROTO_VERSION.saturating_add(1),
            ..params
        };
        assert_ne!(baseline, changed_context);
        assert!(compute_consensus_parameters_fingerprint(&changed_protocol).is_err());
    }
    #[test]
    fn canonical_v2_npos_fingerprint_binds_election_seed() {
        let mut p = ConsensusGenesisParams {
            mode: ConsensusGenesisModeParams::Npos(NposGenesisParams {
                epoch_length_blocks: core::num::NonZeroU64::new(3_600)
                    .expect("test epoch length must be non-zero"),
                epoch_seed: [0u8; 32],
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                max_validators: 31,
                min_self_bond: 1_000_u64.into(),
                min_nomination_bond: 1_u64.into(),
                max_nominator_concentration_pct: 40,
                seat_band_pct: 15,
                max_entity_correlation_pct: 25,
                finality_margin_blocks: 8,
                evidence_horizon_blocks: 7_200,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 9,
            }),
            ..permissioned_genesis_params()
        };
        let ConsensusGenesisModeParams::Npos(npos) = &mut p.mode else {
            unreachable!("fixture must use NPoS")
        };
        npos.epoch_seed = [7; 32];
        let a =
            compute_consensus_parameters_fingerprint(&p).expect("NPoS fixture must fingerprint");
        let ConsensusGenesisModeParams::Npos(npos) = &mut p.mode else {
            unreachable!("fixture must use NPoS")
        };
        npos.epoch_seed[0] ^= 1;
        let election_seed_changed = compute_consensus_parameters_fingerprint(&p)
            .expect("changed NPoS fixture must fingerprint");
        assert_ne!(a, election_seed_changed);
    }
    #[test]
    fn handshake_gate_rejects_on_fingerprint_mismatch() {
        let chain = test_network_id("iroha:test:hshake");
        let p1 = permissioned_genesis_params();
        let mut p2 = p1.clone();
        p2.v2_context.nexus_amx_context_hash[0] ^= 1;
        let f1 = compute_consensus_parameters_fingerprint(&p1)
            .expect("baseline carrier must fingerprint");
        let f2 = compute_consensus_parameters_fingerprint(&p2)
            .expect("changed carrier must fingerprint");
        let gate = HandshakeGate::local(chain.clone(), f1, PERMISSIONED_TAG);
        assert!(
            gate.validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f1)
                .is_ok()
        );
        let err = gate
            .validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f2)
            .expect_err("signed Nexus/AMX context mismatch must be rejected");
        let expected_hex = hex::encode(f1);
        let got_hex = hex::encode(f2);
        assert_eq!(
            err,
            format!(
                "handshake rejected: consensus-parameters fingerprint mismatch (expected {expected_hex}, got {got_hex})"
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
