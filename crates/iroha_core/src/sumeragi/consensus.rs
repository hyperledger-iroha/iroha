//! Sumeragi bootstrap helpers.
//!
//! Global consensus messages, signatures, and quorum certificates live only
//! in [`iroha_data_model::block::consensus_v2`]. This module deliberately does
//! not expose the retired bitmap-QC or v1 vote-signing helpers.
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
use iroha_data_model::block::consensus::{
    ConsensusGenesisModeParams, ConsensusGenesisParams, NposGenesisParams,
};
pub use iroha_data_model::block::consensus::{
    Evidence, ExecKv, ExecWitness, LaneBlockProposalV1, ValidatorIndex,
};
/// Live consensus protocol revision.
pub const PROTO_VERSION: u32 = iroha_data_model::block::consensus_v2::PROTOCOL_VERSION as u32;
/// Permissioned Sumeragi v2 handshake and signing-domain tag.
pub const PERMISSIONED_TAG: &str = iroha_data_model::block::consensus_v2::PERMISSIONED_TAG;
/// NPoS Sumeragi v2 handshake and signing-domain tag.
pub const NPOS_TAG: &str = iroha_data_model::block::consensus_v2::NPOS_TAG;
use crate::state::{StateView, WorldReadOnly};
use iroha_data_model::parameter::system::SumeragiNposParameters;
use iroha_data_model::prelude::*;
/// Compute the genesis-embedded v2 consensus-parameters fingerprint.
///
/// Mode, cadence, block bound, signed DA/Nexus context, and the
/// genesis-selected NPoS election inputs are the complete canonical Norito
/// projection for the first release.
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
    let canon =
        consensus_genesis_params_from_parameters(frozen_mode, s_params, signed_v2_context.clone())
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
    fn test_network_id(seed: &str) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new(seed.as_bytes()),
        ))
    }
    fn permissioned_genesis_params() -> ConsensusGenesisParams {
        ConsensusGenesisParams {
            block_cadence_ms: core::num::NonZeroU64::new(1_000)
                .expect("test cadence must be non-zero"),
            block_max_transactions: core::num::NonZeroU64::new(1_024)
                .expect("test block bound must be non-zero"),
            mode: ConsensusGenesisModeParams::Permissioned,
            protocol_version: PROTO_VERSION,
            v2_context: crate::kagemusha_v1_test_fixtures::genesis_context_parameters(),
        }
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
            ..permissioned_params.clone()
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
