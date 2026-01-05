//! Sumeragi core message types and helpers.
//!
//! This module defines canonical, Norito-encoded types for commit-certificate
//! voting (prepare/commit/new-view), execution attestations, and evidence.
//! It is used by the actor and related tooling.
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

use iroha_config::parameters::actual::{
    Common as CommonConfig, ConsensusMode, Sumeragi as SumeragiConfig,
};
#[cfg(test)]
use iroha_crypto::HashOf;
pub use iroha_data_model::block::consensus::{
    CertPhase, CommitAggregate, CommitCertificate, CommitCertificateRef, CommitVote,
    ConsensusBlockHeader, ConsensusGenesisParams, Evidence, EvidenceKind, EvidencePayload, ExecKv,
    ExecVote, ExecWitness, ExecWitnessMsg, ExecutionQC, Height, NPOS_TAG, NposGenesisParams,
    PERMISSIONED_TAG, PROTO_VERSION, Proposal, RbcChunk, RbcDeliver, RbcInit, RbcReady, Reconfig,
    ValidatorIndex, View, VrfCommit, VrfReveal,
};

// Transitional aliases to reduce churn while the QC terminology is removed.
/// Commit-certificate phase (prepare/commit/new-view).
pub type Phase = CertPhase;
/// Commit vote used for certificate aggregation.
pub type Vote = CommitVote;
/// Commit certificate representing quorum-signed consensus.
pub type Qc = CommitCertificate;
/// Reference to a commit certificate header carried in hints.
pub type QcHeaderRef = CommitCertificateRef;
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

/// Build the canonical preimage for a commit-vote signature under the given chain and mode tag.
pub fn vote_preimage(chain_id: &ChainId, mode_tag: &str, v: &Vote) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 1);
    let domain = consensus_domain(chain_id, "Vote", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(v.block_hash.as_ref().as_ref());
    out.extend_from_slice(&v.height.to_be_bytes());
    out.extend_from_slice(&v.view.to_be_bytes());
    out.extend_from_slice(&v.epoch.to_be_bytes());
    out.push(v.phase as u8);
    out
}

/// Canonical preimage helpers for BLS signing (same-message across signers).
#[cfg(feature = "bls")]
pub mod bls_preimage {
    use super::*;

    fn write_u64(buf: &mut Vec<u8>, v: u64) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Build the canonical preimage for a Vote signature under the given chain and mode tag.
    pub fn vote(chain_id: &ChainId, mode_tag: &str, v: &Vote) -> Vec<u8> {
        super::vote_preimage(chain_id, mode_tag, v)
    }

    /// Build the canonical preimage for an [`ExecVote`] signature.
    pub fn exec_vote(chain_id: &ChainId, mode_tag: &str, v: &ExecVote) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 32 * 3 + 8 * 3);
        let domain = consensus_domain(chain_id, "ExecVote", b"v1", mode_tag);
        out.extend_from_slice(&domain);
        out.extend_from_slice(v.block_hash.as_ref().as_ref());
        out.extend_from_slice(v.parent_state_root.as_ref());
        out.extend_from_slice(v.post_state_root.as_ref());
        write_u64(&mut out, v.height);
        write_u64(&mut out, v.view);
        write_u64(&mut out, v.epoch);
        out
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
    iroha_crypto::blake2::digest::Update::update(&mut hasher, b"iroha2-consensus/v1");
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

/// Deterministic computation of the consensus fingerprint from canonical params.
///
/// Layout in the preimage:
/// `MODE_TAG || PROTO_VERSION || chain_id || Norito(ConsensusGenesisParams)`
///
/// This replaces the interim JSON used during scaffolding. The previous helper
/// remains for transition, but call sites should migrate to this function.
pub fn compute_consensus_fingerprint_from_params(
    chain_id: &ChainId,
    params: &ConsensusGenesisParams,
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    use norito::codec::Encode as _;
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &PROTO_VERSION.to_be_bytes());
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    let bytes = params.encode();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &bytes);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

/// Derive consensus handshake capabilities (mode tag, BLS domain, fingerprint) from the current state view and configuration.
pub fn compute_consensus_handshake_caps_from_view(
    view: &StateView<'_>,
    common_config: &CommonConfig,
    sumeragi_config: &SumeragiConfig,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
) -> (String, String, iroha_p2p::ConsensusHandshakeCaps) {
    let s_params = view.world().parameters();
    let sumeragi = s_params.sumeragi();
    let block = s_params.block();
    let effective_mode =
        crate::sumeragi::effective_consensus_mode(view, sumeragi_config.consensus_mode);
    let (mode_tag, bls_domain) = match effective_mode {
        ConsensusMode::Permissioned => (
            PERMISSIONED_TAG.to_string(),
            "bls-iroha2:permissioned-sumeragi:v1".to_string(),
        ),
        ConsensusMode::Npos => (
            NPOS_TAG.to_string(),
            "bls-iroha2:npos-sumeragi:v1".to_string(),
        ),
    };
    let epoch_length_blocks = if matches!(effective_mode, ConsensusMode::Npos) {
        sumeragi_config.epoch_length_blocks.max(1)
    } else {
        0
    };
    let npos_params = if matches!(effective_mode, ConsensusMode::Npos) {
        let npos_cfg = &sumeragi_config.npos;
        let duration_ms = |d: Duration| -> u64 {
            let ms = d.as_millis();
            u64::try_from(ms).expect("NPoS timeout exceeds supported millisecond range")
        };
        Some(NposGenesisParams {
            block_time_ms: duration_ms(npos_cfg.block_time),
            timeout_propose_ms: duration_ms(npos_cfg.timeouts.propose),
            timeout_prevote_ms: duration_ms(npos_cfg.timeouts.prevote),
            timeout_precommit_ms: duration_ms(npos_cfg.timeouts.precommit),
            timeout_commit_ms: duration_ms(npos_cfg.timeouts.commit),
            timeout_da_ms: duration_ms(npos_cfg.timeouts.da),
            timeout_aggregator_ms: duration_ms(npos_cfg.timeouts.aggregator),
            k_aggregators: u16::try_from(npos_cfg.k_aggregators)
                .expect("npos.k_aggregators must fit into u16"),
            redundant_send_r: npos_cfg.redundant_send_r,
            vrf_commit_window_blocks: npos_cfg.vrf.commit_window_blocks,
            vrf_reveal_window_blocks: npos_cfg.vrf.reveal_window_blocks,
            min_self_bond: npos_cfg.election.min_self_bond,
            max_nominator_concentration_pct: npos_cfg.election.max_nominator_concentration_pct,
            seat_band_pct: npos_cfg.election.seat_band_pct,
            max_entity_correlation_pct: npos_cfg.election.max_entity_correlation_pct,
            evidence_horizon_blocks: npos_cfg.reconfig.evidence_horizon_blocks,
            activation_lag_blocks: npos_cfg.reconfig.activation_lag_blocks,
        })
    } else {
        None
    };

    let canon = ConsensusGenesisParams {
        block_time_ms: sumeragi.block_time_ms,
        commit_time_ms: sumeragi.commit_time_ms,
        max_clock_drift_ms: sumeragi.max_clock_drift_ms,
        collectors_k: sumeragi.collectors_k,
        redundant_send_r: sumeragi.collectors_redundant_send_r,
        block_max_transactions: block.max_transactions().get(),
        da_enabled: sumeragi_config.da_enabled,
        epoch_length_blocks,
        bls_domain: bls_domain.clone(),
        npos: npos_params,
    };
    let fingerprint =
        compute_consensus_fingerprint_from_params(&common_config.chain, &canon, &mode_tag);

    (
        mode_tag.clone(),
        bls_domain,
        iroha_p2p::ConsensusHandshakeCaps {
            mode_tag,
            proto_version: PROTO_VERSION,
            consensus_fingerprint: fingerprint,
            config: *config_caps,
        },
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
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 4 + 32);
    let domain = consensus_domain(chain_id, "RbcReady", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(ready.block_hash.as_ref().as_ref());
    out.extend_from_slice(&ready.height.to_be_bytes());
    out.extend_from_slice(&ready.view.to_be_bytes());
    out.extend_from_slice(&ready.epoch.to_be_bytes());
    out.extend_from_slice(ready.chunk_root.as_ref());
    out.extend_from_slice(&ready.sender.to_be_bytes());
    out
}

/// Build canonical preimage for signing an RBC DELIVER message.
pub fn rbc_deliver_preimage(chain_id: &ChainId, mode_tag: &str, deliver: &RbcDeliver) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 4 + 32);
    let domain = consensus_domain(chain_id, "RbcDeliver", b"v1", mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(deliver.block_hash.as_ref().as_ref());
    out.extend_from_slice(&deliver.height.to_be_bytes());
    out.extend_from_slice(&deliver.view.to_be_bytes());
    out.extend_from_slice(&deliver.epoch.to_be_bytes());
    out.extend_from_slice(deliver.chunk_root.as_ref());
    out.extend_from_slice(&deliver.sender.to_be_bytes());
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

    fn sample_validator_set(count: usize) -> Vec<PeerId> {
        (0..count)
            .map(|_| {
                PeerId::new(
                    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect()
    }

    #[test]
    fn qc_roundtrip_encode_decode() {
        let validator_set = sample_validator_set(16);
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0u8; 32],
            )),
            height: 10,
            view: 7,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_cert: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: CommitAggregate {
                signers_bitmap: vec![0xAA, 0x01],
                bls_aggregate_signature: vec![1, 2, 3],
            },
        };
        let bytes = qc.encode();
        let dec = Qc::decode(&mut &bytes[..]).expect("decode qc");
        assert_eq!(qc, dec);
    }

    #[test]
    fn qc_signer_count_counts_bits() {
        let validator_set = sample_validator_set(16);
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [1u8; 32],
            )),
            height: 2,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_cert: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: CommitAggregate {
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
            height: 2,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_cert: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: CommitAggregate {
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
            max_clock_drift_ms: 500,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v1".to_string(),
            npos: None,
        };
        let npos_params = ConsensusGenesisParams {
            block_time_ms: 1000,
            commit_time_ms: 1000,
            max_clock_drift_ms: 500,
            collectors_k: 3,
            redundant_send_r: 2,
            block_max_transactions: 1024,
            da_enabled: true,
            epoch_length_blocks: 10,
            bls_domain: "bls-iroha2:npos-sumeragi:v1".to_string(),
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
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                min_self_bond: 1,
                max_nominator_concentration_pct: 25,
                seat_band_pct: 5,
                max_entity_correlation_pct: 25,
                evidence_horizon_blocks: 50,
                activation_lag_blocks: 1,
            }),
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
    fn canonical_fingerprint_changes_on_param_diff() {
        let chain = ChainId::from("iroha:test:fpcanon");
        let mut p = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            max_clock_drift_ms: 1000,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v1".to_string(),
            npos: None,
        };
        let a = compute_consensus_fingerprint_from_params(&chain, &p, PERMISSIONED_TAG);
        p.collectors_k = 2; // change a single field
        let b = compute_consensus_fingerprint_from_params(&chain, &p, PERMISSIONED_TAG);
        assert_ne!(a, b);
    }

    #[test]
    fn canonical_fingerprint_changes_on_npos_param_diff() {
        let chain = ChainId::from("iroha:test:fpcanon-npos");
        let mut p = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            max_clock_drift_ms: 1000,
            collectors_k: 3,
            redundant_send_r: 2,
            block_max_transactions: 1024,
            da_enabled: true,
            epoch_length_blocks: 3600,
            bls_domain: "bls-iroha2:npos-sumeragi:v1".to_string(),
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
                vrf_commit_window_blocks: 100,
                vrf_reveal_window_blocks: 40,
                min_self_bond: 1_000,
                max_nominator_concentration_pct: 40,
                seat_band_pct: 15,
                max_entity_correlation_pct: 25,
                evidence_horizon_blocks: 7_200,
                activation_lag_blocks: 1,
            }),
        };
        let a = compute_consensus_fingerprint_from_params(&chain, &p, NPOS_TAG);
        if let Some(npos) = p.npos.as_mut() {
            npos.timeout_commit_ms += 1;
        }
        let b = compute_consensus_fingerprint_from_params(&chain, &p, NPOS_TAG);
        assert_ne!(a, b);
    }

    #[test]
    fn handshake_gate_rejects_on_fingerprint_mismatch() {
        let chain = ChainId::from("iroha:test:hshake");
        let p1 = ConsensusGenesisParams {
            block_time_ms: 2000,
            commit_time_ms: 4000,
            max_clock_drift_ms: 1000,
            collectors_k: 1,
            redundant_send_r: 1,
            block_max_transactions: 1024,
            da_enabled: false,
            epoch_length_blocks: 0,
            bls_domain: "bls-iroha2:permissioned-sumeragi:v1".to_string(),
            npos: None,
        };
        let p2 = ConsensusGenesisParams {
            collectors_k: 2,
            ..p1.clone()
        };
        let f1 = compute_consensus_fingerprint_from_params(&chain, &p1, PERMISSIONED_TAG);
        let f2 = compute_consensus_fingerprint_from_params(&chain, &p2, PERMISSIONED_TAG);
        let gate = HandshakeGate::local(chain.clone(), f1, PERMISSIONED_TAG);
        assert!(
            gate.validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f1)
                .is_ok()
        );
        let err = gate
            .validate_peer(&chain, PERMISSIONED_TAG, PROTO_VERSION, &f2)
            .expect_err("fingerprint mismatch must be rejected");
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
