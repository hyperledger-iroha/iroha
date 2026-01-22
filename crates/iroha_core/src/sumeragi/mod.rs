//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use eyre::Result;
use iroha_config::parameters::{
    actual::{
        Common as CommonConfig, ConsensusMode, Sumeragi as SumeragiConfig, SumeragiNpos,
        SumeragiNposTimeouts,
    },
    defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    consensus::{ValidatorElectionParameters, VrfEpochRecord},
    merge::MergeCommitteeSignature,
    nexus::LaneRelayEnvelope,
    parameter::system::SumeragiConsensusMode,
    peer::PeerId,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, spawn_os_thread_as_future};
use iroha_genesis::GenesisBlock;
use mv::storage::StorageReadOnly;
use norito::codec::Encode as _;

use crate::{
    kura::BlockCount,
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};

/// Stack size reserved for the consensus worker thread to handle deep recursion safely.
const SUMERAGI_STACK_SIZE_BYTES: usize = 64 * 1024 * 1024;
const WORKER_WAKE_CHANNEL_CAP: usize = 1;

/// Build the initial validator topology from trusted peers.
/// Enforces BLS-normal keys and, when configured with a complete `PoP` map, valid `PoP` entries.
/// Observers are not included; this helper filters the validator set only.
pub fn filter_validators_from_trusted(
    tp: &iroha_config::parameters::actual::TrustedPeers,
) -> Vec<PeerId> {
    let mut baseline: BTreeSet<PeerId> = BTreeSet::new();
    let iter = std::iter::once(tp.myself.clone()).chain(tp.others.clone());
    for peer in iter {
        let pk = peer.id().public_key();
        if pk.algorithm() != Algorithm::BlsNormal {
            iroha_logger::warn!(?pk, "excluding peer: validator identity must be BLS-normal");
            continue;
        }
        baseline.insert(PeerId::new(pk.clone()));
    }

    let mut out = if tp.pops.is_empty() {
        baseline.clone()
    } else {
        let missing = baseline
            .iter()
            .filter(|peer_id| !tp.pops.contains_key(peer_id.public_key()))
            .count();
        if missing > 0 {
            iroha_logger::warn!(
                missing,
                baseline = baseline.len(),
                pops = tp.pops.len(),
                "PoP map incomplete for trusted peers; skipping PoP filtering"
            );
            baseline.clone()
        } else {
            let mut filtered: BTreeSet<PeerId> = BTreeSet::new();
            for peer_id in &baseline {
                let pk = peer_id.public_key();
                let Some(pop) = tp.pops.get(pk) else {
                    iroha_logger::warn!(?pk, "missing PoP; excluding peer from consensus");
                    continue;
                };
                if let Err(e) = iroha_crypto::bls_normal_pop_verify(pk, pop) {
                    iroha_logger::warn!(?pk, ?e, "invalid PoP; excluding peer from consensus");
                    continue;
                }
                filtered.insert(peer_id.clone());
            }
            let baseline_len = baseline.len();
            let needed = if baseline_len > 3 {
                ((baseline_len.saturating_sub(1)) / 3) * 2 + 1
            } else {
                baseline_len
            };
            if filtered.len() < needed {
                iroha_logger::warn!(
                    filtered = filtered.len(),
                    baseline = baseline_len,
                    needed,
                    pops = tp.pops.len(),
                    "PoP filtering produced sub-quorum roster; falling back to BLS baseline"
                );
                baseline.clone()
            } else {
                filtered
            }
        }
    };

    // If the explicit peer roster was empty but the configuration still includes PoP
    // records, fall back to those so we do not silently collapse into a single-node
    // topology when addresses were omitted.
    if out.is_empty() && !tp.pops.is_empty() {
        iroha_logger::warn!(
            roster_peers = tp.others.len().saturating_add(1),
            pops = tp.pops.len(),
            "validator roster resolved empty from trusted peers; falling back to PoP map"
        );
        for (bls_pk, pop) in &tp.pops {
            if let Err(e) = iroha_crypto::bls_normal_pop_verify(bls_pk, pop) {
                iroha_logger::warn!(?bls_pk, ?e, "invalid PoP; excluding peer from consensus");
                continue;
            }
            out.insert(PeerId::new(bls_pk.clone()));
        }
        if out.is_empty() {
            iroha_logger::warn!(
                pops = tp.pops.len(),
                "validator roster still empty after PoP fallback"
            );
        }
    }

    iroha_logger::info!(
        validators = out.len(),
        configured_peers = tp.others.len().saturating_add(1),
        pops = tp.pops.len(),
        "resolved validator roster from trusted peers"
    );

    out.into_iter().collect()
}

/// Derive the effective consensus mode for a specific height using on-chain parameters.
pub fn effective_consensus_mode_for_height(
    view: &StateView<'_>,
    height: u64,
    fallback: ConsensusMode,
) -> ConsensusMode {
    effective_consensus_mode_for_height_from_world(view.world(), height, fallback)
}

/// Derive the effective consensus mode for a specific height from a world snapshot.
pub fn effective_consensus_mode_for_height_from_world(
    world: &impl WorldReadOnly,
    height: u64,
    fallback: ConsensusMode,
) -> ConsensusMode {
    let params = world.parameters();
    let sumeragi = params.sumeragi();
    match (sumeragi.next_mode, sumeragi.mode_activation_height) {
        (Some(next), Some(activation_height)) => {
            let next_mode = match next {
                SumeragiConsensusMode::Permissioned => ConsensusMode::Permissioned,
                SumeragiConsensusMode::Npos => ConsensusMode::Npos,
            };
            if height >= activation_height {
                return next_mode;
            }
            if fallback == next_mode {
                return match next_mode {
                    ConsensusMode::Permissioned => ConsensusMode::Npos,
                    ConsensusMode::Npos => ConsensusMode::Permissioned,
                };
            }
            fallback
        }
        _ => fallback,
    }
}

/// Derive the effective runtime consensus mode based on on-chain parameters and the current
/// height snapshot, falling back to the configured mode when no activation has occurred yet.
pub fn effective_consensus_mode(view: &StateView<'_>, fallback: ConsensusMode) -> ConsensusMode {
    let height = u64::try_from(view.height()).unwrap_or(0);
    effective_consensus_mode_for_height(view, height, fallback)
}

/// Snapshot of `NPoS` collector configuration derived from on-chain parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NposCollectorConfig {
    /// Epoch PRF seed currently in effect.
    pub seed: [u8; 32],
    /// Number of collectors K to designate per round.
    pub k: usize,
    /// Redundant send fan-out.
    pub redundant_send_r: u8,
}

/// Snapshot of VRF epoch scheduling parameters for `NPoS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NposEpochParams {
    /// Epoch length in blocks.
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start.
    pub commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start.
    pub reveal_deadline_offset: u64,
}

/// Snapshot of epoch boundaries derived from finalized VRF records.
#[derive(Clone, Debug)]
pub(crate) struct EpochScheduleSnapshot {
    finalized: Vec<(u64, u64)>,
    last_finalized_epoch: Option<u64>,
    last_finalized_end: u64,
    fallback_epoch_length: u64,
}

impl EpochScheduleSnapshot {
    pub(crate) fn from_world_with_fallback(
        world: &impl WorldReadOnly,
        fallback_epoch_length: u64,
    ) -> Self {
        let mut finalized = Vec::new();
        let mut last_end = 0;
        for (epoch, record) in world.vrf_epochs().iter() {
            if !record.finalized || record.updated_at_height == 0 {
                continue;
            }
            if record.updated_at_height < last_end {
                iroha_logger::warn!(
                    epoch = record.epoch,
                    observed = record.updated_at_height,
                    expected = last_end,
                    "ignoring non-monotonic VRF epoch end height"
                );
                break;
            }
            finalized.push((*epoch, record.updated_at_height));
            last_end = record.updated_at_height;
        }

        let fallback_epoch_length = world
            .sumeragi_npos_parameters()
            .map(|params| params.epoch_length_blocks())
            .or_else(|| {
                world
                    .vrf_epochs()
                    .iter()
                    .last()
                    .map(|(_, record)| record.epoch_length)
            })
            .unwrap_or(fallback_epoch_length)
            .max(1);
        let last_finalized_epoch = finalized.last().map(|(epoch, _)| *epoch);
        let last_finalized_end = finalized.last().map_or(0, |(_, end)| *end);

        Self {
            finalized,
            last_finalized_epoch,
            last_finalized_end,
            fallback_epoch_length,
        }
    }

    pub(crate) fn from_world(world: &impl WorldReadOnly) -> Self {
        Self::from_world_with_fallback(world, EPOCH_LENGTH_BLOCKS)
    }

    pub(crate) fn last_finalized_end(&self) -> u64 {
        self.last_finalized_end
    }

    pub(crate) fn epoch_for_height(&self, height: u64) -> u64 {
        if height == 0 {
            return 0;
        }
        for (epoch, end_height) in &self.finalized {
            if height <= *end_height {
                return *epoch;
            }
        }
        let fallback_len = self.fallback_epoch_length.max(1);
        self.last_finalized_epoch.map_or_else(
            || height.saturating_sub(1) / fallback_len,
            |last_epoch| {
                let start = self.last_finalized_end.saturating_add(1);
                if height < start {
                    last_epoch
                } else {
                    let offset = height.saturating_sub(start);
                    last_epoch.saturating_add(1 + offset / fallback_len)
                }
            },
        )
    }

    pub(crate) fn is_epoch_boundary(&self, height: u64) -> bool {
        if height == 0 {
            return false;
        }
        if self.finalized.iter().any(|(_, end)| *end == height) {
            return true;
        }
        let start = self.last_finalized_end.saturating_add(1);
        if height < start {
            return false;
        }
        let offset = height.saturating_sub(start);
        (offset + 1).is_multiple_of(self.fallback_epoch_length.max(1))
    }
}

/// Attempt to load `NPoS` collector configuration (PRF seed + tunables) from the given state view.
pub fn load_npos_collector_config(view: &StateView<'_>) -> Option<NposCollectorConfig> {
    let params = view.world.sumeragi_npos_parameters()?;
    let seed = latest_epoch_seed(view);
    let k = usize::from(params.k_aggregators());
    let redundant_send_r = params.redundant_send_r();
    Some(NposCollectorConfig {
        seed,
        k,
        redundant_send_r,
    })
}

/// Resolve VRF epoch parameters from on-chain `SumeragiNposParameters` when available,
/// falling back to the local configuration otherwise.
pub(crate) fn load_npos_epoch_params(
    view: &StateView<'_>,
    config: &SumeragiConfig,
) -> NposEpochParams {
    view.world.sumeragi_npos_parameters().map_or(
        NposEpochParams {
            epoch_length_blocks: config.epoch_length_blocks,
            commit_deadline_offset: config.vrf_commit_deadline_offset,
            reveal_deadline_offset: config.vrf_reveal_deadline_offset,
        },
        |params| {
            let commit_window = params.vrf_commit_window_blocks();
            let reveal_window = params.vrf_reveal_window_blocks();
            NposEpochParams {
                epoch_length_blocks: params.epoch_length_blocks(),
                commit_deadline_offset: commit_window,
                reveal_deadline_offset: commit_window.saturating_add(reveal_window),
            }
        },
    )
}

/// Resolve the `NPoS` pacemaker block time from on-chain parameters, falling back to config.
pub(crate) fn resolve_npos_block_time(view: &StateView<'_>, fallback: &SumeragiNpos) -> Duration {
    view.world
        .sumeragi_npos_parameters()
        .and_then(|params| {
            let ms = params.block_time_ms();
            (ms > 0).then_some(Duration::from_millis(ms))
        })
        .unwrap_or(fallback.block_time)
}

/// Resolve `NPoS` pacemaker timeouts from on-chain parameters, falling back to config values.
pub(crate) fn resolve_npos_timeouts(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> SumeragiNposTimeouts {
    let mut out = fallback.timeouts;
    let Some(params) = view.world.sumeragi_npos_parameters() else {
        return out;
    };
    let resolve = |value_ms: u64, fallback: Duration| {
        if value_ms == 0 {
            fallback
        } else {
            Duration::from_millis(value_ms)
        }
    };
    out.propose = resolve(params.timeout_propose_ms(), fallback.timeouts.propose);
    out.prevote = resolve(params.timeout_prevote_ms(), fallback.timeouts.prevote);
    out.precommit = resolve(params.timeout_precommit_ms(), fallback.timeouts.precommit);
    out.commit = resolve(params.timeout_commit_ms(), fallback.timeouts.commit);
    out.da = resolve(params.timeout_da_ms(), fallback.timeouts.da);
    out.aggregator = resolve(params.timeout_aggregator_ms(), fallback.timeouts.aggregator);
    out
}

/// Resolve `NPoS` election parameters from on-chain values, falling back to config defaults.
pub(crate) fn resolve_npos_election_params(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> ValidatorElectionParameters {
    view.world.sumeragi_npos_parameters().map_or(
        ValidatorElectionParameters {
            max_validators: fallback.election.max_validators,
            min_self_bond: fallback.election.min_self_bond,
            min_nomination_bond: fallback.election.min_nomination_bond,
            max_nominator_concentration_pct: fallback.election.max_nominator_concentration_pct,
            seat_band_pct: fallback.election.seat_band_pct,
            max_entity_correlation_pct: fallback.election.max_entity_correlation_pct,
            finality_margin_blocks: fallback.election.finality_margin_blocks,
        },
        |params| ValidatorElectionParameters {
            max_validators: params.max_validators(),
            min_self_bond: params.min_self_bond(),
            min_nomination_bond: params.min_nomination_bond(),
            max_nominator_concentration_pct: params.max_nominator_concentration_pct(),
            seat_band_pct: params.seat_band_pct(),
            max_entity_correlation_pct: params.max_entity_correlation_pct(),
            finality_margin_blocks: params.finality_margin_blocks(),
        },
    )
}

/// Resolve `NPoS` activation lag for VRF penalties from on-chain parameters or config.
pub(crate) fn resolve_npos_activation_lag_blocks(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> u64 {
    view.world
        .sumeragi_npos_parameters()
        .map_or(fallback.reconfig.activation_lag_blocks, |params| {
            params.activation_lag_blocks()
        })
}

/// Resolve `NPoS` slashing delay (blocks) for evidence penalties from on-chain parameters or config.
pub(crate) fn resolve_npos_slashing_delay_blocks(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> u64 {
    view.world
        .sumeragi_npos_parameters()
        .map_or(fallback.reconfig.slashing_delay_blocks, |params| {
            params.slashing_delay_blocks()
        })
}

fn latest_epoch_seed(view: &StateView<'_>) -> [u8; 32] {
    latest_epoch_seed_from_world(&view.world, view.chain_id())
}

fn chain_epoch_seed(chain_id: &ChainId) -> [u8; 32] {
    let chain = chain_id.clone().into_inner();
    let hash = CryptoHash::new(chain.as_bytes());
    <[u8; 32]>::from(hash)
}

fn latest_epoch_seed_from_world(world: &impl WorldReadOnly, chain_id: &ChainId) -> [u8; 32] {
    let mut seed_opt = None;
    for (_epoch, record) in world.vrf_epochs().iter() {
        seed_opt = Some(record.seed);
    }
    seed_opt
        .or_else(|| {
            world
                .sumeragi_npos_parameters()
                .map(|params| params.epoch_seed())
        })
        .unwrap_or_else(|| chain_epoch_seed(chain_id))
}

fn next_epoch_seed_from_record(record: &VrfEpochRecord) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let mut h = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut h, &record.seed);
    let mut reveals: Vec<(u32, [u8; 32])> = record
        .participants
        .iter()
        .filter_map(|p| p.reveal.map(|reveal| (p.signer, reveal)))
        .collect();
    reveals.sort_by_key(|(signer, _)| *signer);
    for (signer, reveal) in reveals {
        iroha_crypto::blake2::digest::Update::update(&mut h, &signer.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut h, &reveal);
    }
    let digest = iroha_crypto::blake2::Digest::finalize(h);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

/// Resolve the epoch index for a height using finalized VRF epoch boundaries when available.
pub(crate) fn epoch_for_height_from_world(world: &impl WorldReadOnly, height: u64) -> u64 {
    EpochScheduleSnapshot::from_world(world).epoch_for_height(height)
}

/// Resolve the `NPoS` PRF seed for the epoch containing `height`.
pub fn npos_seed_for_height(view: &StateView<'_>, height: u64) -> [u8; 32] {
    npos_seed_for_height_from_world(&view.world, view.chain_id(), height)
}

/// Resolve the PRF seed for the epoch containing `height`.
pub fn prf_seed_for_height(view: &StateView<'_>, height: u64) -> [u8; 32] {
    prf_seed_for_height_from_world(&view.world, view.chain_id(), height)
}

/// Resolve the `NPoS` PRF seed for the epoch containing `height` from any world snapshot.
pub(crate) fn npos_seed_for_height_from_world(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    height: u64,
) -> [u8; 32] {
    if let Some(params) = world.sumeragi_npos_parameters() {
        let epoch = epoch_for_height_from_world(world, height);
        if let Some(record) = world.vrf_epochs().get(&epoch) {
            return record.seed;
        }
        if let Some((_last_epoch, record)) = world.vrf_epochs().iter().last() {
            if record.finalized && epoch == record.epoch.saturating_add(1) {
                // Crash recovery: derive the next-epoch seed if the in-progress snapshot
                // was not persisted before restart.
                return next_epoch_seed_from_record(record);
            }
        }
        return params.epoch_seed();
    }
    latest_epoch_seed_from_world(world, chain_id)
}

/// Resolve the PRF seed for the epoch containing `height` from any world snapshot.
pub(crate) fn prf_seed_for_height_from_world(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    height: u64,
) -> [u8; 32] {
    npos_seed_for_height_from_world(world, chain_id, height)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU64,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        time::Duration,
    };

    use iroha_config::parameters::actual::SumeragiNpos;
    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf, bls_normal_pop_prove};
    use iroha_data_model::{
        block::{
            BlockHeader, BlockSignature, SignedBlock,
            consensus::{ConsensusBlockHeader, LaneBlockCommitment, Proposal, RbcChunk},
        },
        consensus::VrfEpochRecord,
        nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters},
        },
        peer::{Peer, PeerId},
    };
    use iroha_primitives::{addr::SocketAddr, unique_vec::UniqueVec};

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::consensus::{Phase, QcHeaderRef, ValidatorIndex, Vote},
    };

    const TEST_CHANNEL_CAP: usize = 16;

    fn inbound(msg: BlockMessage) -> InboundBlockMessage {
        InboundBlockMessage::new(msg, None)
    }

    fn backdate(now: Instant, duration: Duration) -> Instant {
        now.checked_sub(duration).unwrap_or(now)
    }

    fn vrf_record(
        epoch: u64,
        end_height: u64,
        epoch_length: u64,
        finalized: bool,
    ) -> VrfEpochRecord {
        VrfEpochRecord {
            epoch,
            seed: [u8::try_from(epoch).unwrap_or(0xAA); 32],
            epoch_length,
            commit_deadline_offset: 1,
            reveal_deadline_offset: 2,
            roster_len: 0,
            finalized,
            updated_at_height: end_height,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        }
    }

    fn make_peer(key_pair: &KeyPair, port: u16) -> Peer {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        Peer::new(addr, key_pair.public_key().clone())
    }

    fn state_with_npos_params(params: SumeragiNposParameters) -> State {
        let world = World::new();
        {
            let mut block = world.block();
            let custom = block.parameters.get_mut();
            custom.custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            block.commit();
        }
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    #[test]
    fn trusted_roster_without_pops_keeps_bls_peers() {
        let kp0 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp1 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp2 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer0 = make_peer(&kp0, 10_000);
        let peer1 = make_peer(&kp1, 10_001);
        let peer2 = make_peer(&kp2, 10_002);

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: peer0.clone(),
            others: UniqueVec::from_iter(vec![peer1.clone(), peer2.clone()]),
            pops: BTreeMap::new(),
        };
        let expected: BTreeSet<_> =
            vec![peer0.id().clone(), peer1.id().clone(), peer2.id().clone()]
                .into_iter()
                .collect();
        let actual: BTreeSet<_> = filter_validators_from_trusted(&trusted)
            .into_iter()
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn trusted_roster_pop_filter_falls_back_on_sub_quorum() {
        let kp0 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp1 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp2 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp3 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer0 = make_peer(&kp0, 10_010);
        let peer1 = make_peer(&kp1, 10_011);
        let peer2 = make_peer(&kp2, 10_012);
        let peer3 = make_peer(&kp3, 10_013);

        let mut pops = BTreeMap::new();
        let pop = bls_normal_pop_prove(kp0.private_key()).expect("pop prove");
        pops.insert(kp0.public_key().clone(), pop);

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: peer0.clone(),
            others: UniqueVec::from_iter(vec![peer1.clone(), peer2.clone(), peer3.clone()]),
            pops,
        };
        let expected: BTreeSet<_> = vec![
            peer0.id().clone(),
            peer1.id().clone(),
            peer2.id().clone(),
            peer3.id().clone(),
        ]
        .into_iter()
        .collect();
        let actual: BTreeSet<_> = filter_validators_from_trusted(&trusted)
            .into_iter()
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn trusted_roster_skips_incomplete_pops() {
        let kp0 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp1 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp2 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp3 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer0 = make_peer(&kp0, 10_020);
        let peer1 = make_peer(&kp1, 10_021);
        let peer2 = make_peer(&kp2, 10_022);
        let peer3 = make_peer(&kp3, 10_023);

        let mut pops = BTreeMap::new();
        for kp in [&kp0, &kp1, &kp2] {
            let pop = bls_normal_pop_prove(kp.private_key()).expect("pop prove");
            pops.insert(kp.public_key().clone(), pop);
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: peer0.clone(),
            others: UniqueVec::from_iter(vec![peer1.clone(), peer2.clone(), peer3.clone()]),
            pops,
        };
        let expected: BTreeSet<_> = vec![
            peer0.id().clone(),
            peer1.id().clone(),
            peer2.id().clone(),
            peer3.id().clone(),
        ]
        .into_iter()
        .collect();
        let actual: BTreeSet<_> = filter_validators_from_trusted(&trusted)
            .into_iter()
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn epoch_schedule_uses_finalized_boundaries() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        {
            let mut world = state.world.block();
            let params = SumeragiNposParameters {
                epoch_length_blocks: 12,
                ..SumeragiNposParameters::default()
            };
            world
                .parameters
                .set_parameter(Parameter::Custom(params.into_custom_parameter()));
            world.vrf_epochs.insert(0, vrf_record(0, 10, 10, true));
            world.vrf_epochs.insert(1, vrf_record(1, 22, 12, true));
            world.commit();
        }

        let view = state.view();
        let schedule = EpochScheduleSnapshot::from_world(view.world());
        assert_eq!(schedule.epoch_for_height(1), 0);
        assert_eq!(schedule.epoch_for_height(10), 0);
        assert_eq!(schedule.epoch_for_height(11), 1);
        assert_eq!(schedule.epoch_for_height(22), 1);
        assert_eq!(schedule.epoch_for_height(23), 2);
        assert!(schedule.is_epoch_boundary(10));
        assert!(schedule.is_epoch_boundary(22));
        assert!(!schedule.is_epoch_boundary(21));
    }

    #[test]
    fn should_run_tick_when_queues_not_exhausted() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(250));
        assert!(should_run_tick(
            now,
            last_tick,
            false,
            false,
            false,
            false,
            status::WorkerQueueDepthSnapshot::default(),
            Duration::from_millis(200),
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn should_skip_tick_when_idle_and_gap_small() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(50));
        assert!(!should_run_tick(
            now,
            last_tick,
            false,
            false,
            false,
            false,
            status::WorkerQueueDepthSnapshot::default(),
            Duration::from_millis(200),
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn should_run_tick_when_gap_exceeds_max() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(500));
        assert!(should_run_tick(
            now,
            last_tick,
            true,
            true,
            true,
            true,
            status::WorkerQueueDepthSnapshot {
                vote_rx: 1,
                ..status::WorkerQueueDepthSnapshot::default()
            },
            Duration::from_millis(100),
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn should_skip_tick_when_exhausted_and_gap_small() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(50));
        assert!(!should_run_tick(
            now,
            last_tick,
            true,
            true,
            true,
            true,
            status::WorkerQueueDepthSnapshot::default(),
            Duration::from_millis(100),
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn should_skip_tick_when_queue_busy_and_gap_small() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(50));
        assert!(!should_run_tick(
            now,
            last_tick,
            false,
            false,
            false,
            false,
            status::WorkerQueueDepthSnapshot {
                block_payload_rx: 1,
                ..status::WorkerQueueDepthSnapshot::default()
            },
            Duration::from_millis(100),
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn idle_wait_duration_tracks_min_gap() {
        let base = Instant::now();
        let min_gap = Duration::from_millis(200);
        assert_eq!(idle_wait_duration(base, base, min_gap), Some(min_gap));
        assert_eq!(
            idle_wait_duration(base + Duration::from_millis(50), base, min_gap),
            Some(Duration::from_millis(150))
        );
        assert_eq!(idle_wait_duration(base + min_gap, base, min_gap), None);
    }

    #[test]
    fn select_next_tier_prefers_votes_when_not_starved() {
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");

        let cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let budgets = TierBudgets::new(&cfg);
        let mut mailbox = WorkerMailbox::new(
            &mut loop_state.mailbox,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );
        mailbox.fill_slots(&budgets);

        let selected =
            select_next_tier(now, &mailbox, &budgets, &loop_state.last_served, &cfg, true);
        assert_eq!(selected, Some(PriorityTier::Votes));
    }

    #[test]
    fn select_next_tier_prefers_starved_non_vote() {
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");

        let cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(2),
            non_vote_starve_max: Duration::from_millis(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        loop_state.last_served[PriorityTier::BlockPayload.idx()] =
            backdate(now, cfg.non_vote_starve_max + Duration::from_millis(1));
        let budgets = TierBudgets::new(&cfg);
        let mut mailbox = WorkerMailbox::new(
            &mut loop_state.mailbox,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );
        mailbox.fill_slots(&budgets);

        let selected = select_next_tier(
            now,
            &mailbox,
            &budgets,
            &loop_state.last_served,
            &cfg,
            false,
        );
        assert_eq!(selected, Some(PriorityTier::BlockPayload));
    }

    #[test]
    fn select_next_tier_picks_oldest_pending_after_vote_burst() {
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");

        let cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(2),
            non_vote_starve_max: Duration::from_secs(2),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        loop_state.last_served[PriorityTier::BlockPayload.idx()] =
            backdate(now, Duration::from_millis(500));
        let budgets = TierBudgets::new(&cfg);
        let mut mailbox = WorkerMailbox::new(
            &mut loop_state.mailbox,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );
        mailbox.fill_slots(&budgets);

        let selected = select_next_tier(
            now,
            &mailbox,
            &budgets,
            &loop_state.last_served,
            &cfg,
            false,
        );
        assert_eq!(selected, Some(PriorityTier::BlockPayload));
    }

    #[test]
    fn resolve_sumeragi_timeouts_prefers_on_chain_values() {
        let (block_time, commit_time) = resolve_sumeragi_timeouts(
            Duration::from_millis(900),
            Duration::from_millis(1_100),
            Duration::from_secs(2),
            Duration::from_secs(4),
        );

        assert_eq!(block_time, Duration::from_millis(900));
        assert_eq!(commit_time, Duration::from_millis(1_100));
    }

    #[test]
    fn resolve_sumeragi_timeouts_uses_fallback_for_zero() {
        let (block_time, commit_time) = resolve_sumeragi_timeouts(
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(2),
            Duration::from_secs(4),
        );

        assert_eq!(block_time, Duration::from_secs(2));
        assert_eq!(commit_time, Duration::from_secs(4));
    }

    #[test]
    fn vote_rx_drain_budget_caps_at_time_budget() {
        let budget = vote_rx_drain_budget(
            Duration::from_secs(1),
            Duration::from_secs(1),
            true,
            1,
            Duration::from_millis(200),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(200));
    }

    #[test]
    fn vote_rx_drain_budget_uses_commit_quorum_window() {
        let budget = vote_rx_drain_budget(
            Duration::from_millis(80),
            Duration::from_millis(50),
            true,
            1,
            Duration::from_secs(1),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(280));
    }

    #[test]
    fn vote_rx_drain_budget_never_zero() {
        let budget = vote_rx_drain_budget(
            Duration::ZERO,
            Duration::ZERO,
            true,
            1,
            Duration::from_millis(200),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(1));
    }

    #[test]
    fn vote_rx_drain_budget_scales_with_da_multiplier() {
        let budget = vote_rx_drain_budget(
            Duration::from_millis(10),
            Duration::from_millis(20),
            true,
            2,
            Duration::from_secs(5),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(180));
    }

    #[test]
    fn vote_rx_drain_budget_caps_at_drain_cap() {
        let budget = vote_rx_drain_budget(
            Duration::from_millis(200),
            Duration::from_millis(300),
            true,
            3,
            Duration::from_secs(10),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS));
    }

    #[test]
    fn vote_rx_drain_budget_clamps_to_config_cap() {
        let budget = vote_rx_drain_budget(
            Duration::from_millis(200),
            Duration::from_millis(300),
            true,
            3,
            Duration::from_secs(10),
            Duration::from_millis(150),
        );
        assert_eq!(budget, Duration::from_millis(150));
    }

    #[test]
    fn worker_time_budget_clamps_to_floor_for_small_quorum_timeout() {
        let budget = worker_time_budget(
            Duration::from_millis(10),
            Duration::from_millis(10),
            true,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(TIME_BUDGET_FLOOR_MS));
    }

    #[test]
    fn worker_time_budget_clamps_to_cap_for_large_quorum_timeout() {
        let budget = worker_time_budget(
            Duration::from_secs(1),
            Duration::from_secs(2),
            true,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(TIME_BUDGET_CAP_MS));
    }

    #[test]
    fn worker_time_budget_uses_quorum_timeout_for_non_da() {
        let budget = worker_time_budget(
            Duration::from_millis(100),
            Duration::from_millis(200),
            false,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(500));
    }

    #[test]
    fn worker_time_budget_respects_config_cap() {
        let budget = worker_time_budget(
            Duration::from_secs(1),
            Duration::from_secs(2),
            true,
            1,
            Duration::from_millis(150),
        );
        assert_eq!(budget, Duration::from_millis(150));
    }

    #[test]
    fn idle_tick_gap_clamps_to_floor_and_max() {
        let gap = idle_tick_gap(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(2),
        );
        assert_eq!(gap, Duration::from_millis(250));

        let clamped = idle_tick_gap(
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(2),
        );
        assert_eq!(clamped, Duration::from_secs(2));

        let floored = idle_tick_gap(
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_millis(200),
        );
        assert_eq!(floored, Duration::from_millis(IDLE_TICK_GAP_FLOOR_MS));
    }

    #[test]
    fn cap_drain_budget_clamps_to_floor_and_cap() {
        let floor = Duration::from_millis(200);
        let cap = Duration::from_millis(DRAIN_BUDGET_CAP_MS);
        assert_eq!(
            cap_drain_budget(Duration::from_secs(2), floor, cap),
            Duration::from_millis(DRAIN_BUDGET_CAP_MS)
        );
        assert_eq!(
            cap_drain_budget(Duration::from_millis(100), floor, cap),
            floor
        );
        assert_eq!(
            cap_drain_budget(Duration::from_millis(300), floor, cap),
            Duration::from_millis(300)
        );
    }

    #[test]
    fn cap_vote_drain_budget_clamps_to_floor_and_cap() {
        let floor = Duration::from_millis(200);
        let cap = Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS);
        assert_eq!(
            cap_vote_drain_budget(Duration::from_secs(5), floor, cap),
            Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS)
        );
        assert_eq!(
            cap_vote_drain_budget(Duration::from_millis(100), floor, cap),
            floor
        );
        assert_eq!(
            cap_vote_drain_budget(Duration::from_millis(300), floor, cap),
            Duration::from_millis(300)
        );
    }

    #[test]
    fn cap_rbc_drain_budget_clamps_to_floor_and_cap() {
        let floor = Duration::from_millis(200);
        let cap = Duration::from_millis(RBC_DRAIN_BUDGET_CAP_MS);
        assert_eq!(
            cap_rbc_drain_budget(Duration::from_secs(5), floor, cap),
            Duration::from_millis(RBC_DRAIN_BUDGET_CAP_MS)
        );
        assert_eq!(
            cap_rbc_drain_budget(Duration::from_millis(100), floor, cap),
            floor
        );
        assert_eq!(
            cap_rbc_drain_budget(Duration::from_millis(300), floor, cap),
            Duration::from_millis(300)
        );
    }

    #[test]
    fn effective_consensus_mode_switches_after_activation_height() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(0);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        assert_eq!(
            effective_consensus_mode(&view, ConsensusMode::Permissioned),
            ConsensusMode::Npos
        );
    }

    #[test]
    fn effective_consensus_mode_for_height_switches_after_activation_height() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(5);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        assert_eq!(
            effective_consensus_mode_for_height(&view, 4, ConsensusMode::Permissioned),
            ConsensusMode::Permissioned
        );
        assert_eq!(
            effective_consensus_mode_for_height(&view, 5, ConsensusMode::Permissioned),
            ConsensusMode::Npos
        );
    }

    #[test]
    fn effective_consensus_mode_for_height_from_world_matches_view() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(7);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let world_view = state.world.view();

        for height in [0_u64, 6, 7, 9] {
            assert_eq!(
                effective_consensus_mode_for_height(&view, height, ConsensusMode::Permissioned),
                effective_consensus_mode_for_height_from_world(
                    &world_view,
                    height,
                    ConsensusMode::Permissioned
                ),
                "world view should mirror StateView for consensus mode"
            );
        }
    }

    #[test]
    fn effective_consensus_mode_uses_fallback_before_activation_height() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(10);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        assert_eq!(
            effective_consensus_mode(&view, ConsensusMode::Permissioned),
            ConsensusMode::Permissioned
        );
    }

    #[test]
    fn effective_consensus_mode_for_height_uses_fallback_before_activation_height() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(10);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        assert_eq!(
            effective_consensus_mode_for_height(&view, 9, ConsensusMode::Permissioned),
            ConsensusMode::Permissioned
        );
    }

    #[test]
    fn effective_consensus_mode_for_height_uses_pre_activation_mode_after_flip() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.sumeragi.next_mode = Some(SumeragiConsensusMode::Npos);
            params.sumeragi.mode_activation_height = Some(10);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        assert_eq!(
            effective_consensus_mode_for_height(&view, 9, ConsensusMode::Npos),
            ConsensusMode::Permissioned
        );
    }

    #[test]
    fn load_npos_collector_config_uses_vrf_seed() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.custom.insert(
                SumeragiNposParameters::parameter_id(),
                SumeragiNposParameters::default().into_custom_parameter(),
            );
            block.vrf_epochs.insert(
                0,
                VrfEpochRecord {
                    epoch: 0,
                    seed: [0xAB; 32],
                    epoch_length: 3600,
                    commit_deadline_offset: 100,
                    reveal_deadline_offset: 140,
                    roster_len: 4,
                    finalized: false,
                    updated_at_height: 0,
                    participants: Vec::new(),
                    late_reveals: Vec::new(),
                    committed_no_reveal: Vec::new(),
                    no_participation: Vec::new(),
                    penalties_applied: false,
                    penalties_applied_at_height: None,
                    validator_election: None,
                },
            );
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let config = load_npos_collector_config(&view).expect("npos config present");
        assert_eq!(config.seed, [0xAB; 32]);
        assert_eq!(
            config.k,
            usize::from(SumeragiNposParameters::default().k_aggregators())
        );
        assert_eq!(
            config.redundant_send_r,
            SumeragiNposParameters::default().redundant_send_r()
        );
    }

    #[test]
    fn load_npos_collector_config_falls_back_to_epoch_seed() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = SumeragiNposParameters::default().with_epoch_seed([0xCD; 32]);
            block.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let config = load_npos_collector_config(&view).expect("npos config present");
        assert_eq!(config.seed, [0xCD; 32]);
    }

    #[test]
    fn resolve_npos_block_time_uses_on_chain_or_fallback() {
        let fallback = SumeragiNpos {
            block_time: Duration::from_millis(2_500),
            ..SumeragiNpos::default()
        };

        let state = state_with_npos_params(SumeragiNposParameters {
            block_time_ms: 1_500,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        assert_eq!(
            resolve_npos_block_time(&view, &fallback),
            Duration::from_millis(1_500)
        );
        drop(view);

        let state = state_with_npos_params(SumeragiNposParameters {
            block_time_ms: 0,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        assert_eq!(
            resolve_npos_block_time(&view, &fallback),
            fallback.block_time
        );
    }

    #[test]
    fn resolve_npos_timeouts_overrides_config_fields() {
        let mut fallback = SumeragiNpos::default();
        fallback.timeouts.propose = Duration::from_millis(10);
        fallback.timeouts.prevote = Duration::from_millis(20);
        fallback.timeouts.precommit = Duration::from_millis(30);
        fallback.timeouts.exec = Duration::from_millis(40);
        fallback.timeouts.witness = Duration::from_millis(50);
        fallback.timeouts.commit = Duration::from_millis(60);
        fallback.timeouts.da = Duration::from_millis(70);
        fallback.timeouts.aggregator = Duration::from_millis(80);

        let state = state_with_npos_params(SumeragiNposParameters {
            timeout_propose_ms: 100,
            timeout_prevote_ms: 110,
            timeout_precommit_ms: 120,
            timeout_commit_ms: 0,
            timeout_da_ms: 140,
            timeout_aggregator_ms: 150,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        let resolved = resolve_npos_timeouts(&view, &fallback);
        assert_eq!(resolved.propose, Duration::from_millis(100));
        assert_eq!(resolved.prevote, Duration::from_millis(110));
        assert_eq!(resolved.precommit, Duration::from_millis(120));
        assert_eq!(resolved.commit, fallback.timeouts.commit);
        assert_eq!(resolved.da, Duration::from_millis(140));
        assert_eq!(resolved.aggregator, Duration::from_millis(150));
        assert_eq!(resolved.exec, fallback.timeouts.exec);
        assert_eq!(resolved.witness, fallback.timeouts.witness);
    }

    #[test]
    fn resolve_npos_election_params_prefers_on_chain() {
        let mut fallback = SumeragiNpos::default();
        fallback.election.max_validators = 1;
        fallback.election.min_self_bond = 2;
        fallback.election.min_nomination_bond = 3;
        fallback.election.max_nominator_concentration_pct = 4;
        fallback.election.seat_band_pct = 5;
        fallback.election.max_entity_correlation_pct = 6;
        fallback.election.finality_margin_blocks = 7;

        let state = state_with_npos_params(SumeragiNposParameters {
            max_validators: 11,
            min_self_bond: 12,
            min_nomination_bond: 13,
            max_nominator_concentration_pct: 14,
            seat_band_pct: 15,
            max_entity_correlation_pct: 16,
            finality_margin_blocks: 17,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        let resolved = resolve_npos_election_params(&view, &fallback);
        assert_eq!(resolved.max_validators, 11);
        assert_eq!(resolved.min_self_bond, 12);
        assert_eq!(resolved.min_nomination_bond, 13);
        assert_eq!(resolved.max_nominator_concentration_pct, 14);
        assert_eq!(resolved.seat_band_pct, 15);
        assert_eq!(resolved.max_entity_correlation_pct, 16);
        assert_eq!(resolved.finality_margin_blocks, 17);
    }

    #[test]
    fn resolve_npos_activation_lag_blocks_prefers_on_chain() {
        let mut fallback = SumeragiNpos::default();
        fallback.reconfig.activation_lag_blocks = 12;

        let state = state_with_npos_params(SumeragiNposParameters {
            activation_lag_blocks: 3,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        assert_eq!(resolve_npos_activation_lag_blocks(&view, &fallback), 3);
        drop(view);

        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert_eq!(
            resolve_npos_activation_lag_blocks(&view, &fallback),
            fallback.reconfig.activation_lag_blocks
        );
    }

    #[test]
    fn resolve_npos_slashing_delay_blocks_prefers_on_chain() {
        let mut fallback = SumeragiNpos::default();
        fallback.reconfig.slashing_delay_blocks = 42;

        let state = state_with_npos_params(SumeragiNposParameters {
            slashing_delay_blocks: 7,
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        assert_eq!(resolve_npos_slashing_delay_blocks(&view, &fallback), 7);
        drop(view);

        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert_eq!(
            resolve_npos_slashing_delay_blocks(&view, &fallback),
            fallback.reconfig.slashing_delay_blocks
        );
    }

    #[test]
    fn roster_adapter_from_iter_preserves_peers() {
        let peers: Vec<PeerId> = (0..4)
            .map(|_| {
                let (public_key, _) = KeyPair::random().into_parts();
                public_key.into()
            })
            .collect();

        let adapter = WsvEpochRosterAdapter::from_peer_iter(peers.clone());

        assert_eq!(adapter.peers(), peers.as_slice());
    }

    #[test]
    fn vote_dedup_rejects_duplicates_and_eviction_maintains_capacity() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            Arc::clone(&vote_dedup),
            Arc::clone(&block_payload_dedup),
        );

        let make_hash = |seed: u64| {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&seed.to_be_bytes());
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(bytes))
        };

        let base_key: VoteDedupKey = (
            SumeragiHandle::phase_id(crate::sumeragi::consensus::Phase::Prepare),
            make_hash(0),
            1,
            0,
            0,
            0,
        );

        assert!(handle.dedup_vote(base_key));
        assert!(!handle.dedup_vote(base_key));

        let max_dedup_cache = 8192usize;
        let first_key = (
            0,
            make_hash(1),
            2,
            0,
            0,
            ValidatorIndex::try_from(0).expect("validator index fits") + 1,
        );
        {
            let mut guard = vote_dedup.lock().expect("dedup cache poisoned");
            let now = Instant::now();
            guard.clear();
            for i in 0..max_dedup_cache {
                guard.insert(
                    (
                        0,
                        make_hash(i as u64 + 1),
                        i as u64 + 2,
                        0,
                        0,
                        ValidatorIndex::try_from(i).expect("validator index fits") + 1,
                    ),
                    now,
                );
            }
            assert_eq!(guard.len(), max_dedup_cache);
        }

        assert!(handle.dedup_vote(base_key));
        let guard = vote_dedup.lock().expect("dedup cache poisoned");
        assert!(guard.contains(&base_key));
        assert_eq!(guard.len(), max_dedup_cache);
        assert!(!guard.contains(&first_key));
    }

    #[test]
    fn block_payload_dedup_rejects_duplicates_and_eviction_maintains_capacity() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            Arc::clone(&vote_dedup),
            Arc::clone(&block_payload_dedup),
        );

        let base_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([7u8; 32]));
        let base_key = BlockPayloadDedupKey::BlockCreated {
            height: 1,
            view: 0,
            block_hash: base_hash,
        };

        assert!(handle.dedup_block_payload(base_key));
        assert!(!handle.dedup_block_payload(base_key));

        let max_dedup_cache = BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND;
        let first_payload_hash = Hash::prehashed([0u8; 32]);
        let first_key = BlockPayloadDedupKey::Proposal {
            height: 2,
            view: 0,
            payload_hash: first_payload_hash,
        };
        {
            let mut guard = block_payload_dedup
                .lock()
                .expect("block payload dedup cache poisoned");
            let now = Instant::now();
            guard.clear();
            for i in 0..(max_dedup_cache + 4) {
                let payload_seed = u8::try_from(i % 255).expect("payload seed fits u8");
                let payload_hash = Hash::prehashed([payload_seed; 32]);
                guard.insert(
                    BlockPayloadDedupKey::Proposal {
                        height: i as u64 + 2,
                        view: 0,
                        payload_hash,
                    },
                    now,
                );
            }
            assert_eq!(guard.len_for_key(&first_key), max_dedup_cache);
        }

        assert!(handle.dedup_block_payload(base_key));
        let guard = block_payload_dedup
            .lock()
            .expect("block payload dedup cache poisoned");
        assert!(guard.contains(&base_key));
        assert_eq!(guard.len(), max_dedup_cache + 1);
        assert!(!guard.contains(&first_key));
    }

    #[test]
    fn dedup_cache_refreshes_lru_on_duplicate() {
        let mut cache = DedupCache::new(2, Duration::from_secs(30));
        let now = Instant::now();
        cache.insert(1u64, now);
        cache.insert(2u64, now);
        cache.insert(1u64, now);
        cache.insert(3u64, now);

        assert!(cache.contains(&1));
        assert!(cache.contains(&3));
        assert!(!cache.contains(&2));
    }

    #[test]
    fn dedup_cache_evicts_expired_entries() {
        let ttl = Duration::from_secs(1);
        let mut cache = DedupCache::new(4, ttl);
        let t0 = Instant::now();
        cache.insert(10u64, t0);
        cache.insert(11u64, t0);

        let later = t0 + ttl + Duration::from_millis(1);
        cache.insert(12u64, later);

        assert!(!cache.contains(&10));
        assert!(!cache.contains(&11));
        assert!(cache.contains(&12));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn block_payload_dedup_partitions_by_kind() {
        let mut cache = BlockPayloadDedupCache::new(2, Duration::from_secs(30));
        let now = Instant::now();
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([3u8; 32]));
        let block_key = BlockPayloadDedupKey::BlockCreated {
            height: 1,
            view: 0,
            block_hash,
        };
        cache.insert(block_key, now);

        for idx in 0_u8..3 {
            cache.insert(
                BlockPayloadDedupKey::Proposal {
                    height: u64::from(idx) + 2,
                    view: 0,
                    payload_hash: Hash::prehashed([idx; 32]),
                },
                now,
            );
        }

        assert!(cache.contains(&block_key));
        assert_eq!(cache.len_for_key(&block_key), 1);
        assert_eq!(
            cache.len_for_key(&BlockPayloadDedupKey::Proposal {
                height: 2,
                view: 0,
                payload_hash: Hash::prehashed([0u8; 32]),
            }),
            2
        );
    }

    #[test]
    fn incoming_block_message_drops_duplicate_votes() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            Arc::clone(&vote_dedup),
            Arc::clone(&block_payload_dedup),
        );

        let vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Commit,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([1u8; 32]),
            ),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 3,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let msg = BlockMessage::QcVote(vote.clone());

        handle.incoming_block_message(msg.clone());
        handle.incoming_block_message(msg);

        let received: Vec<_> = vote_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        let only = received.into_iter().next().expect("one message");
        assert!(matches!(only.message, BlockMessage::QcVote(_)));
    }

    #[test]
    fn try_incoming_paths_drop_on_full_channels() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(0);
        let (block_tx, _block_rx) = mpsc::sync_channel(0);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(0);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(0);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(0);
        let (background_tx, _background_rx) = mpsc::sync_channel(0);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(0);
        let vote_dedup = Arc::new(Mutex::new(DedupCache::new(4, VOTE_DEDUP_CACHE_TTL)));
        let block_payload_dedup = Arc::new(Mutex::new(BlockPayloadDedupCache::new(
            4,
            BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
        )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32]));
        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        assert!(!handle.try_incoming_block_message(BlockMessage::RbcChunk(rbc_chunk)));

        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let v2 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let evidence = crate::sumeragi::consensus::Evidence {
            kind: crate::sumeragi::consensus::EvidenceKind::DoublePrepare,
            payload: crate::sumeragi::consensus::EvidencePayload::DoubleVote { v1, v2 },
        };
        assert!(
            !handle.try_incoming_consensus_control_flow_message(ControlFlow::Evidence(evidence))
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let commitment = LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
        };
        let envelope =
            LaneRelayEnvelope::new(header, None, None, commitment, 0).expect("relay envelope");
        assert!(!handle.try_incoming_lane_relay(envelope));

        let signature = MergeCommitteeSignature {
            epoch_id: 0,
            view: 0,
            signer: 0,
            message_digest: Hash::new(b"merge-signature"),
            bls_sig: vec![0x22; 48],
        };
        assert!(!handle.try_incoming_merge_signature(signature));
    }

    #[test]
    fn try_incoming_rbc_chunk_releases_dedup_on_queue_full() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(1);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup = Arc::new(Mutex::new(DedupCache::new(4, VOTE_DEDUP_CACHE_TTL)));
        let block_payload_dedup = Arc::new(Mutex::new(BlockPayloadDedupCache::new(
            4,
            BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
        )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx.clone(),
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32]));
        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::RbcChunk(rbc_chunk.clone())))
            .expect("fill queue");
        assert!(
            !handle.try_incoming_block_message(BlockMessage::RbcChunk(rbc_chunk.clone())),
            "queue full should reject"
        );
        let _ = rbc_chunk_rx.try_recv().expect("drain queue");
        assert!(
            handle.try_incoming_block_message(BlockMessage::RbcChunk(rbc_chunk)),
            "dedup should allow retry after drop"
        );
        let received = rbc_chunk_rx.try_recv().expect("queued chunk");
        assert!(matches!(received.message, BlockMessage::RbcChunk(_)));
    }

    #[test]
    fn try_incoming_block_message_wakes_worker_on_accept() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let vote_dedup = Arc::new(Mutex::new(DedupCache::new(4, VOTE_DEDUP_CACHE_TTL)));
        let block_payload_dedup = Arc::new(Mutex::new(BlockPayloadDedupCache::new(
            4,
            BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
        )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        )
        .with_wake(wake_tx);

        let advert = message::ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        assert!(handle.try_incoming_block_message(BlockMessage::ConsensusParams(advert)));
        assert!(matches!(wake_rx.try_recv(), Ok(())));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_rbc_ready() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let msg = BlockMessage::RbcReady(crate::sumeragi::consensus::RbcReady {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2u8; 32])),
            height: 4,
            view: 1,
            epoch: 0,
            roster_hash: Hash::prehashed([0x11; 32]),
            chunk_root: Hash::prehashed([3u8; 32]),
            sender: 1,
            signature: Vec::new(),
        });

        handle.incoming_block_message(msg.clone());
        handle.incoming_block_message(msg);

        let received: Vec<_> = vote_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(received.iter().all(|msg| {
            matches!(
                msg,
                InboundBlockMessage {
                    message: BlockMessage::RbcReady(_),
                    ..
                }
            )
        }));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_allows_distinct_rbc_ready_signatures() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let base_ready = crate::sumeragi::consensus::RbcReady {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2u8; 32])),
            height: 4,
            view: 1,
            epoch: 0,
            roster_hash: Hash::prehashed([0x12; 32]),
            chunk_root: Hash::prehashed([3u8; 32]),
            sender: 1,
            signature: vec![0xAA],
        };
        let mut alt_ready = base_ready.clone();
        alt_ready.signature = vec![0xBB];

        handle.incoming_block_message(BlockMessage::RbcReady(base_ready));
        handle.incoming_block_message(BlockMessage::RbcReady(alt_ready));

        let received: Vec<_> = vote_rx.try_iter().collect();
        assert_eq!(received.len(), 2);
        assert!(received.iter().all(|msg| {
            matches!(
                msg,
                InboundBlockMessage {
                    message: BlockMessage::RbcReady(_),
                    ..
                }
            )
        }));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_rbc_deliver() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let msg = BlockMessage::RbcDeliver(crate::sumeragi::consensus::RbcDeliver {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([4u8; 32])),
            height: 5,
            view: 2,
            epoch: 0,
            roster_hash: Hash::prehashed([0x21; 32]),
            chunk_root: Hash::prehashed([5u8; 32]),
            sender: 2,
            signature: Vec::new(),
            ready_signatures: Vec::new(),
        });

        handle.incoming_block_message(msg.clone());
        handle.incoming_block_message(msg);

        let received: Vec<_> = vote_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(received.iter().all(|msg| {
            matches!(
                msg,
                InboundBlockMessage {
                    message: BlockMessage::RbcDeliver(_),
                    ..
                }
            )
        }));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_block_created_via_payload_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);

        handle.incoming_block_message(BlockMessage::BlockCreated(message::BlockCreated { block }));

        let received = block_payload_rx
            .try_recv()
            .expect("BlockCreated should be enqueued to block-payload channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockCreated(_),
                ..
            }
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_block_payload() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let msg = BlockMessage::BlockCreated(message::BlockCreated { block });

        handle.incoming_block_message(msg.clone());
        handle.incoming_block_message(msg);

        let received: Vec<_> = block_payload_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received.first(),
            Some(InboundBlockMessage {
                message: BlockMessage::BlockCreated(_),
                ..
            })
        ));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_block_sync_update() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let update = message::BlockSyncUpdate::from(&block);

        handle.incoming_block_message(BlockMessage::BlockSyncUpdate(update.clone()));
        handle.incoming_block_message(BlockMessage::BlockSyncUpdate(update));

        let received: Vec<_> = block_payload_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received.first(),
            Some(InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            })
        ));
    }

    #[test]
    fn incoming_block_message_accepts_block_sync_update_with_new_evidence() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let block_hash = block.hash();
        let update = message::BlockSyncUpdate::from(&block);
        let mut update_with_votes = message::BlockSyncUpdate::from(&block);
        update_with_votes.commit_votes.push(Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root: Hash::prehashed([0xAA; 32]),
            post_state_root: Hash::prehashed([0xBB; 32]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: vec![1, 2, 3],
        });

        handle.incoming_block_message(BlockMessage::BlockSyncUpdate(update));
        handle.incoming_block_message(BlockMessage::BlockSyncUpdate(update_with_votes));

        let received: Vec<_> = block_payload_rx.try_iter().collect();
        assert_eq!(received.len(), 2);
        assert!(received.iter().all(|msg| matches!(
            msg,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        )));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_rbc_chunk() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32]));
        let chunk = crate::sumeragi::consensus::RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };

        handle.incoming_block_message(BlockMessage::RbcChunk(chunk.clone()));
        handle.incoming_block_message(BlockMessage::RbcChunk(chunk));

        let received: Vec<_> = rbc_chunk_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received.first(),
            Some(InboundBlockMessage {
                message: BlockMessage::RbcChunk(_),
                ..
            })
        ));
    }

    #[test]
    fn incoming_block_message_routes_block_sync_update_via_block_payload_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let update = message::BlockSyncUpdate::from(&block);

        handle.incoming_block_message(BlockMessage::BlockSyncUpdate(update));

        let received = block_payload_rx
            .try_recv()
            .expect("BlockSyncUpdate should be enqueued to block payload channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn incoming_block_message_routes_fetch_pending_block_via_payload_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let requester = PeerId::new(KeyPair::random().public_key().clone());
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
        let request = message::FetchPendingBlock {
            requester,
            block_hash,
        };
        handle.incoming_block_message(BlockMessage::FetchPendingBlock(request));

        let received = block_payload_rx
            .try_recv()
            .expect("FetchPendingBlock should be enqueued to block payload channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::FetchPendingBlock(_),
                ..
            }
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn incoming_block_message_blocks_block_sync_update_when_block_payload_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let block_payload_tx_fill = block_payload_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let update = message::BlockSyncUpdate::from(&block);
        let overflow_header = BlockHeader {
            height: NonZeroU64::new(2).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let overflow_signature = SignatureOf::from_hash(&private_key, overflow_header.hash());
        let overflow_block_signature = BlockSignature::new(0, overflow_signature);
        let overflow_block = SignedBlock::presigned_with_da(
            overflow_block_signature,
            overflow_header,
            Vec::new(),
            None,
        );
        let update_overflow = message::BlockSyncUpdate::from(&overflow_block);

        block_payload_tx_fill
            .send(inbound(BlockMessage::BlockSyncUpdate(update)))
            .expect("fill block payload channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.incoming_block_message(BlockMessage::BlockSyncUpdate(update_overflow));
            let _ = done_tx.send(());
        });

        done_rx
            .recv_timeout(Duration::from_millis(200))
            .expect_err("BlockSyncUpdate should block when block payload queue is full");
        let received = block_payload_rx
            .try_recv()
            .expect("original BlockSyncUpdate should remain in the block payload queue");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        ));
        done_rx
            .recv_timeout(Duration::from_millis(200))
            .expect("BlockSyncUpdate should enqueue after capacity frees");
        let received = block_payload_rx
            .try_recv()
            .expect("overflow BlockSyncUpdate should be enqueued after capacity frees");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        ));
        join.join().expect("join BlockSyncUpdate sender");
    }

    #[test]
    fn try_incoming_block_message_blocks_block_sync_update_when_block_payload_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let block_payload_tx_fill = block_payload_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let update = message::BlockSyncUpdate::from(&block);
        let overflow_header = BlockHeader {
            height: NonZeroU64::new(2).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let overflow_signature = SignatureOf::from_hash(&private_key, overflow_header.hash());
        let overflow_block_signature = BlockSignature::new(0, overflow_signature);
        let overflow_block = SignedBlock::presigned_with_da(
            overflow_block_signature,
            overflow_header,
            Vec::new(),
            None,
        );
        let update_overflow = message::BlockSyncUpdate::from(&overflow_block);

        block_payload_tx_fill
            .send(inbound(BlockMessage::BlockSyncUpdate(update)))
            .expect("fill block payload channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.try_incoming_block_message(BlockMessage::BlockSyncUpdate(update_overflow));
            let _ = done_tx.send(());
        });

        done_rx
            .recv_timeout(Duration::from_millis(200))
            .expect_err("BlockSyncUpdate should block when block payload queue is full");
        let received = block_payload_rx
            .try_recv()
            .expect("original BlockSyncUpdate should remain in the block payload queue");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        ));
        done_rx
            .recv_timeout(Duration::from_millis(200))
            .expect("BlockSyncUpdate should enqueue after capacity frees");
        let received = block_payload_rx
            .try_recv()
            .expect("overflow BlockSyncUpdate should be enqueued after capacity frees");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockSyncUpdate(_),
                ..
            }
        ));
        join.join().expect("join BlockSyncUpdate sender");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn incoming_block_message_waits_when_block_payload_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let block_payload_tx_fill = block_payload_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header_one = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let header_two = BlockHeader {
            height: NonZeroU64::new(2).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature_one = SignatureOf::from_hash(&private_key, header_one.hash());
        let signature_two = SignatureOf::from_hash(&private_key, header_two.hash());
        let block_one = SignedBlock::presigned_with_da(
            BlockSignature::new(0, signature_one),
            header_one,
            Vec::new(),
            None,
        );
        let block_two = SignedBlock::presigned_with_da(
            BlockSignature::new(0, signature_two),
            header_two,
            Vec::new(),
            None,
        );

        block_payload_tx_fill
            .send(inbound(BlockMessage::BlockCreated(message::BlockCreated {
                block: block_one,
            })))
            .expect("fill block payload channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.incoming_block_message(BlockMessage::BlockCreated(
                message::BlockCreated { block: block_two },
            ));
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "BlockCreated should wait for block payload queue capacity"
        );
        let _ = block_payload_rx
            .recv()
            .expect("drain block payload queue to unblock sender");
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("BlockCreated should be enqueued after space is available");
        join.join().expect("join BlockCreated sender");

        let received = block_payload_rx
            .try_recv()
            .expect("BlockCreated should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockCreated(_),
                ..
            }
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn try_incoming_block_message_waits_when_block_payload_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let block_payload_tx_fill = block_payload_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let header_one = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let header_two = BlockHeader {
            height: NonZeroU64::new(2).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature_one = SignatureOf::from_hash(&private_key, header_one.hash());
        let signature_two = SignatureOf::from_hash(&private_key, header_two.hash());
        let block_one = SignedBlock::presigned_with_da(
            BlockSignature::new(0, signature_one),
            header_one,
            Vec::new(),
            None,
        );
        let block_two = SignedBlock::presigned_with_da(
            BlockSignature::new(0, signature_two),
            header_two,
            Vec::new(),
            None,
        );

        block_payload_tx_fill
            .send(inbound(BlockMessage::BlockCreated(message::BlockCreated {
                block: block_one,
            })))
            .expect("fill block payload channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.try_incoming_block_message(BlockMessage::BlockCreated(
                message::BlockCreated { block: block_two },
            ));
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "BlockCreated should wait for block payload queue capacity"
        );
        let _ = block_payload_rx
            .recv()
            .expect("drain block payload queue to unblock sender");
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("BlockCreated should be enqueued after space is available");
        join.join().expect("join BlockCreated sender");

        let received = block_payload_rx
            .try_recv()
            .expect("BlockCreated should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::BlockCreated(_),
                ..
            }
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_rbc_ready_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let vote_tx_fill = vote_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let filler_ready = BlockMessage::RbcReady(crate::sumeragi::consensus::RbcReady {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([6u8; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x20; 32]),
            chunk_root: Hash::prehashed([0x30; 32]),
            sender: 0,
            signature: vec![0x10],
        });

        vote_tx_fill
            .send(inbound(filler_ready))
            .expect("fill vote channel");

        let ready = BlockMessage::RbcReady(crate::sumeragi::consensus::RbcReady {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([7u8; 32])),
            height: 2,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x21; 32]),
            chunk_root: Hash::prehashed([0x31; 32]),
            sender: 1,
            signature: vec![0x11],
        });

        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            let accepted = handle_clone.try_incoming_block_message(ready);
            let _ = done_tx.send(accepted);
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "RbcReady should wait for vote queue capacity"
        );
        let _ = vote_rx.recv().expect("drain vote queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RbcReady should be enqueued after space is available");
        assert!(
            accepted,
            "RbcReady should be accepted after space is available"
        );
        join.join().expect("join RbcReady sender");

        let received = vote_rx
            .try_recv()
            .expect("RbcReady should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcReady(_),
                ..
            }
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_rbc_init_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let rbc_chunk_tx_fill = rbc_chunk_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let filler_chunk = BlockMessage::RbcChunk(RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0x10],
        });
        rbc_chunk_tx_fill
            .send(inbound(filler_chunk))
            .expect("fill rbc chunk channel");

        let height = 2;
        let view = 0;
        let block_header = BlockHeader::new(
            NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            view,
        );
        let leader_key = KeyPair::random();
        let (_, leader_private) = leader_key.into_parts();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(&leader_private, block_header.hash()),
        );
        let init = BlockMessage::RbcInit(crate::sumeragi::consensus::RbcInit {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([3u8; 32])),
            height,
            view,
            epoch: 0,
            roster: vec![PeerId::new(KeyPair::random().public_key().clone())],
            roster_hash: Hash::prehashed([0x14; 32]),
            total_chunks: 1,
            chunk_digests: vec![[5u8; 32]],
            payload_hash: Hash::prehashed([4u8; 32]),
            chunk_root: Hash::prehashed([5u8; 32]),
            block_header,
            leader_signature,
        });

        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            let accepted = handle_clone.try_incoming_block_message(init);
            let _ = done_tx.send(accepted);
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "RbcInit should wait for RBC chunk queue capacity"
        );
        let _ = rbc_chunk_rx
            .recv()
            .expect("drain rbc chunk queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RbcInit should be enqueued after space is available");
        assert!(
            accepted,
            "RbcInit should be accepted after space is available"
        );
        join.join().expect("join RbcInit sender");

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcInit should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcInit(_),
                ..
            }
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_rbc_deliver_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let vote_tx_fill = vote_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let filler_deliver = BlockMessage::RbcDeliver(crate::sumeragi::consensus::RbcDeliver {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([7u8; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x21; 32]),
            chunk_root: Hash::prehashed([0x41; 32]),
            sender: 0,
            signature: vec![0x21],
            ready_signatures: Vec::new(),
        });

        vote_tx_fill
            .send(inbound(filler_deliver))
            .expect("fill vote channel");

        let deliver = BlockMessage::RbcDeliver(crate::sumeragi::consensus::RbcDeliver {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([8u8; 32])),
            height: 2,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x22; 32]),
            chunk_root: Hash::prehashed([0x42; 32]),
            sender: 1,
            signature: vec![0x22],
            ready_signatures: Vec::new(),
        });

        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            let accepted = handle_clone.try_incoming_block_message(deliver);
            let _ = done_tx.send(accepted);
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "RbcDeliver should wait for vote queue capacity"
        );
        let _ = vote_rx.recv().expect("drain vote queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RbcDeliver should be enqueued after space is available");
        assert!(
            accepted,
            "RbcDeliver should be accepted after space is available"
        );
        join.join().expect("join RbcDeliver sender");

        let received = vote_rx
            .try_recv()
            .expect("RbcDeliver should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcDeliver(_),
                ..
            }
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_qc_vote_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let vote_tx_fill = vote_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let filler_vote = Vote {
            phase: Phase::Commit,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32])),
            parent_state_root: Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: Hash::prehashed([1u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };

        vote_tx_fill
            .send(inbound(BlockMessage::QcVote(filler_vote)))
            .expect("fill vote channel");

        let vote = Vote {
            phase: Phase::Commit,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([8u8; 32])),
            parent_state_root: Hash::prehashed([2u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: Hash::prehashed([3u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };

        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            let accepted = handle_clone.try_incoming_block_message(BlockMessage::QcVote(vote));
            let _ = done_tx.send(accepted);
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "QcVote should wait for vote queue capacity"
        );
        let _ = vote_rx.recv().expect("drain vote queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("QcVote should be enqueued after space is available");
        assert!(
            accepted,
            "QcVote should be accepted after space is available"
        );
        join.join().expect("join QcVote sender");

        let received = vote_rx
            .try_recv()
            .expect("QcVote should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::QcVote(_),
                ..
            }
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_drops_when_rbc_chunk_queue_full() {
        const CAP: usize = 1;
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, _block_rx) = mpsc::sync_channel(CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(CAP);
        let (vote_tx, _vote_rx) = mpsc::sync_channel(CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let rbc_chunk_tx_fill = rbc_chunk_tx.clone();
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32]));
        let chunk_one = crate::sumeragi::consensus::RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        let chunk_two = crate::sumeragi::consensus::RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 1,
            bytes: vec![4, 5, 6],
        };
        let chunk_two_again = chunk_two.clone();

        rbc_chunk_tx_fill
            .send(inbound(BlockMessage::RbcChunk(chunk_one)))
            .expect("fill RBC chunk channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.incoming_block_message(BlockMessage::RbcChunk(chunk_two));
            let _ = done_tx.send(());
        });

        done_rx
            .recv_timeout(Duration::from_millis(50))
            .expect("RBC chunk enqueue should not block on full queue");
        join.join().expect("join RBC chunk sender");

        let received = rbc_chunk_rx.recv().expect("drain RBC chunk queue");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcChunk(_),
                ..
            }
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        handle.incoming_block_message(BlockMessage::RbcChunk(chunk_two_again));
        let received = rbc_chunk_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RBC chunk should enqueue after dedup release");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcChunk(_),
                ..
            }
        ));
    }

    #[test]
    fn incoming_block_message_routes_rbc_ready_via_vote_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let msg = BlockMessage::RbcReady(crate::sumeragi::consensus::RbcReady {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32])),
            height: 2,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x13; 32]),
            chunk_root: Hash::prehashed([2u8; 32]),
            sender: 0,
            signature: Vec::new(),
        });

        handle.incoming_block_message(msg);

        let received = vote_rx
            .try_recv()
            .expect("RbcReady should be enqueued to vote channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcReady(_),
                ..
            }
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_proposal_hint_via_vote_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32]));
        let parent_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([8u8; 32]));
        let qc = crate::sumeragi::consensus::QcHeaderRef {
            height: 1,
            view: 0,
            epoch: 0,
            subject_block_hash: parent_hash,
            phase: crate::sumeragi::consensus::Phase::Prepare,
        };
        let hint = crate::sumeragi::message::ProposalHint {
            block_hash,
            height: 2,
            view: 0,
            highest_qc: qc,
        };

        handle.incoming_block_message(BlockMessage::ProposalHint(hint));

        let received = vote_rx
            .try_recv()
            .expect("ProposalHint should be enqueued to vote channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::ProposalHint(_),
                ..
            }
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_rbc_chunk_via_rbc_queue() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let msg = BlockMessage::RbcChunk(crate::sumeragi::consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32])),
            height: 2,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0u8],
        });

        handle.incoming_block_message(msg);

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcChunk should be enqueued to RBC chunk channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcChunk(_),
                ..
            }
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn incoming_block_message_routes_rbc_init_via_rbc_queue() {
        let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, _consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, _background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, _lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            vote_dedup,
            block_payload_dedup,
        );

        let height = 2;
        let view = 0;
        let block_header = BlockHeader::new(
            NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            view,
        );
        let leader_key = KeyPair::random();
        let (_, leader_private) = leader_key.into_parts();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(&leader_private, block_header.hash()),
        );
        let msg = BlockMessage::RbcInit(crate::sumeragi::consensus::RbcInit {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([3u8; 32])),
            height,
            view,
            epoch: 0,
            roster: vec![PeerId::new(KeyPair::random().public_key().clone())],
            roster_hash: Hash::prehashed([0x14; 32]),
            total_chunks: 1,
            chunk_digests: vec![[5u8; 32]],
            payload_hash: Hash::prehashed([4u8; 32]),
            chunk_root: Hash::prehashed([5u8; 32]),
            block_header,
            leader_signature,
        });

        handle.incoming_block_message(msg);

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcInit should be enqueued to RBC chunk channel");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcInit(_),
                ..
            }
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[derive(Default)]
    struct RecordingActor {
        events: Vec<&'static str>,
        tick_calls: usize,
    }

    impl WorkerActor for RecordingActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            let label = match msg.message {
                BlockMessage::QcVote(_) => "vote",
                BlockMessage::RbcChunk(_) => "rbc",
                BlockMessage::Proposal(_) => "payload",
                BlockMessage::ConsensusParams(_) => "block",
                _ => "other",
            };
            self.events.push(label);
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            self.events.push("control");
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            self.events.push("lane");
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            self.events.push("background");
            Ok(())
        }

        fn tick(&mut self) -> bool {
            self.tick_calls = self.tick_calls.saturating_add(1);
            true
        }
    }

    struct RefreshingActor {
        refresh_count: Arc<AtomicUsize>,
    }

    impl WorkerActor for RefreshingActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn refresh_worker_loop_config(&mut self, _cfg: &mut WorkerLoopConfig) {
            self.refresh_count.fetch_add(1, Ordering::Relaxed);
        }

        fn tick(&mut self) -> bool {
            false
        }
    }

    #[derive(Default)]
    struct CommitPollingActor {
        poll_calls: usize,
    }

    impl WorkerActor for CommitPollingActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn poll_commit_results(&mut self) -> bool {
            self.poll_calls = self.poll_calls.saturating_add(1);
            true
        }

        fn tick(&mut self) -> bool {
            false
        }
    }

    #[derive(Default)]
    struct ValidationPollingActor {
        poll_calls: usize,
    }

    impl WorkerActor for ValidationPollingActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn poll_validation_results(&mut self) -> bool {
            self.poll_calls = self.poll_calls.saturating_add(1);
            true
        }

        fn tick(&mut self) -> bool {
            false
        }
    }

    #[derive(Default)]
    struct RbcPollingActor {
        poll_calls: usize,
    }

    impl WorkerActor for RbcPollingActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn poll_rbc_persist_results(&mut self) -> bool {
            self.poll_calls = self.poll_calls.saturating_add(1);
            true
        }

        fn tick(&mut self) -> bool {
            false
        }
    }

    #[derive(Default)]
    struct NoTickActor {
        tick_calls: usize,
    }

    impl WorkerActor for NoTickActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn should_tick(&self) -> bool {
            false
        }

        fn tick(&mut self) -> bool {
            self.tick_calls = self.tick_calls.saturating_add(1);
            true
        }
    }

    #[derive(Default)]
    struct FutureTickActor {
        delay: Duration,
        tick_calls: usize,
    }

    impl WorkerActor for FutureTickActor {
        fn on_block_message(&mut self, _msg: InboundBlockMessage) -> Result<()> {
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn next_tick_deadline(&self, now: Instant) -> Option<Instant> {
            Some(now + self.delay)
        }

        fn tick(&mut self) -> bool {
            self.tick_calls = self.tick_calls.saturating_add(1);
            true
        }
    }

    #[derive(Default)]
    struct RecordingActorWithTick {
        events: Vec<&'static str>,
        tick_calls: usize,
    }

    impl WorkerActor for RecordingActorWithTick {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            let label = match msg.message {
                BlockMessage::QcVote(_) => "vote",
                BlockMessage::RbcChunk(_) => "rbc",
                BlockMessage::Proposal(_) => "payload",
                BlockMessage::ConsensusParams(_) => "block",
                _ => "other",
            };
            self.events.push(label);
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            self.events.push("control");
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            self.events.push("lane");
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            self.events.push("background");
            Ok(())
        }

        fn tick(&mut self) -> bool {
            self.tick_calls = self.tick_calls.saturating_add(1);
            self.events.push("tick");
            true
        }
    }

    struct SlowVoteActor {
        events: Vec<&'static str>,
        vote_sleep: Duration,
    }

    impl WorkerActor for SlowVoteActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::QcVote(_) => {
                    std::thread::sleep(self.vote_sleep);
                    self.events.push("vote");
                }
                BlockMessage::Proposal(_) => self.events.push("payload"),
                _ => self.events.push("other"),
            }
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn tick(&mut self) -> bool {
            false
        }
    }

    struct TickEnqueueActor {
        events: Vec<&'static str>,
        block_payload_tx: mpsc::SyncSender<InboundBlockMessage>,
    }

    impl WorkerActor for TickEnqueueActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            if matches!(msg.message, BlockMessage::Proposal(_)) {
                self.events.push("payload");
            }
            Ok(())
        }

        fn on_consensus_control(&mut self, _msg: ControlFlow) -> Result<()> {
            Ok(())
        }

        fn on_lane_relay(&mut self, _message: LaneRelayMessage) -> Result<()> {
            Ok(())
        }

        fn on_background_request(&mut self, _request: BackgroundRequest) -> Result<()> {
            Ok(())
        }

        fn tick(&mut self) -> bool {
            self.events.push("tick");
            let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
            let proposal = Proposal {
                header: ConsensusBlockHeader {
                    parent_hash: block_hash,
                    tx_root: Hash::new(b"tx"),
                    state_root: Hash::new(b"state"),
                    proposer: 0,
                    height: 1,
                    view: 0,
                    epoch: 0,
                    highest_qc: QcHeaderRef {
                        height: 0,
                        view: 0,
                        epoch: 0,
                        subject_block_hash: block_hash,
                        phase: Phase::Prepare,
                    },
                },
                payload_hash: Hash::new(b"payload"),
            };
            self.block_payload_tx
                .send(InboundBlockMessage::new(
                    BlockMessage::Proposal(proposal),
                    None,
                ))
                .expect("send proposal");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);
            true
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn run_worker_iteration_drains_in_priority_order() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::RbcChunk(rbc_chunk)))
            .expect("send rbc chunk");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

        block_tx
            .send(inbound(BlockMessage::ConsensusParams(
                message::ConsensusParamsAdvert {
                    collectors_k: 1,
                    redundant_send_r: 1,
                    membership: None,
                },
            )))
            .expect("send consensus params");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let past = now
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["vote", "rbc", "payload", "block"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(stats.blocks_handled, 1);
        assert!(stats.progress);
        assert!(!stats.budget_exceeded);
        assert_eq!(actor.tick_calls, 1);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn run_worker_iteration_preempts_votes_for_starved_payloads() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::RbcChunk(rbc_chunk)))
            .expect("send rbc chunk");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

        block_tx
            .send(inbound(BlockMessage::ConsensusParams(
                message::ConsensusParamsAdvert {
                    collectors_k: 1,
                    redundant_send_r: 1,
                    membership: None,
                },
            )))
            .expect("send consensus params");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let past = now
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["rbc", "payload", "block", "vote"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(stats.blocks_handled, 1);
        assert!(stats.progress);
        assert!(!stats.budget_exceeded);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn run_worker_iteration_ticks_after_drains() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::RbcChunk(rbc_chunk)))
            .expect("send rbc chunk");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

        block_tx
            .send(inbound(BlockMessage::ConsensusParams(
                message::ConsensusParamsAdvert {
                    collectors_k: 1,
                    redundant_send_r: 1,
                    membership: None,
                },
            )))
            .expect("send consensus params");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActorWithTick::default();

        run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(
            actor.events,
            vec!["vote", "rbc", "payload", "block", "tick"]
        );
        assert_eq!(actor.tick_calls, 1);
    }

    #[test]
    fn run_worker_iteration_skips_tick_when_actor_disables_tick() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = NoTickActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.tick_calls, 0);
        assert!(!stats.progress);
    }

    #[test]
    fn run_worker_iteration_respects_tick_deadline() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = FutureTickActor {
            delay: Duration::from_secs(1),
            ..FutureTickActor::default()
        };

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.tick_calls, 0);
        assert!(!stats.progress);

        actor.delay = Duration::ZERO;
        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.tick_calls, 1);
        assert!(stats.progress);
    }

    #[test]
    fn run_worker_iteration_polls_commit_results() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = CommitPollingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.poll_calls, 1);
        assert!(stats.progress);
    }

    #[test]
    fn run_worker_iteration_polls_validation_results() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = ValidationPollingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.poll_calls, 1);
        assert!(stats.progress);
    }

    #[test]
    fn run_worker_iteration_polls_rbc_persist_results() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RbcPollingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.poll_calls, 1);
        assert!(stats.progress);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn run_worker_iteration_caps_total_drain_budget() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_millis(5),
            vote_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_millis(50),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_millis(50),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_millis(50),
            block_rx_starve_max: Duration::from_millis(50),
            non_vote_starve_max: Duration::from_millis(50),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = SlowVoteActor {
            events: Vec::new(),
            vote_sleep: Duration::from_millis(20),
        };

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["vote", "payload"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert!(stats.budget_exceeded);
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn run_worker_iteration_caps_drain_at_tick_gap() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        vote_tx
            .send(inbound(BlockMessage::QcVote(vote)))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        block_payload_tx
            .send(inbound(BlockMessage::Proposal(proposal)))
            .expect("send proposal");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_millis(10),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = SlowVoteActor {
            events: Vec::new(),
            vote_sleep: Duration::from_millis(20),
        };

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["vote"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.block_payloads_handled, 0);
        assert!(stats.budget_exceeded);
        assert!(
            loop_state.mailbox.slots[PriorityTier::BlockPayload.idx()].is_some(),
            "payload should remain buffered for the next iteration"
        );
    }

    #[test]
    fn run_worker_iteration_drains_rbc_when_votes_hit_cap_but_empty() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for signer in 0..2u32 {
            let vote = Vote {
                phase: Phase::Prepare,
                block_hash,
                parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: None,
                signer,
                bls_sig: Vec::new(),
            };
            vote_tx
                .send(inbound(BlockMessage::QcVote(vote)))
                .expect("send prevote");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);
        }

        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::RbcChunk(rbc_chunk)))
            .expect("send rbc chunk");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 2,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActor::default();

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["vote", "vote", "rbc"]);
        assert_eq!(stats.votes_handled, 2);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert!(!stats.vote_rx_budget_exhausted);
    }

    #[test]
    fn run_worker_iteration_drains_consensus_before_tick() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let v1 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let v2 = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let evidence = crate::sumeragi::consensus::Evidence {
            kind: crate::sumeragi::consensus::EvidenceKind::DoublePrepare,
            payload: crate::sumeragi::consensus::EvidencePayload::DoubleVote { v1, v2 },
        };
        consensus_tx
            .send(ControlFlow::Evidence(evidence))
            .expect("send evidence");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Consensus);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActorWithTick::default();

        run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["control", "tick"]);
        assert_eq!(actor.tick_calls, 1);
    }

    #[test]
    fn run_worker_iteration_drains_post_tick_payloads() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = TickEnqueueActor {
            events: Vec::new(),
            block_payload_tx,
        };

        let stats = run_worker_iteration(
            &mut actor,
            &config,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        assert_eq!(actor.events, vec!["tick", "payload"]);
        assert_eq!(stats.block_payloads_handled, 1);
        assert!(stats.progress);
        let depths = status::worker_queue_depth_snapshot();
        assert_eq!(depths.block_payload_rx, 0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn worker_scheduler_replay_is_deterministic() {
        fn run_trace() -> Vec<&'static str> {
            let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
            let (background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

            let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
            let vote = Vote {
                phase: Phase::Prepare,
                block_hash,
                parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: None,
                signer: 0,
                bls_sig: Vec::new(),
            };
            vote_tx
                .send(inbound(BlockMessage::QcVote(vote)))
                .expect("send prevote");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);

            let rbc_chunk = RbcChunk {
                block_hash,
                height: 1,
                view: 0,
                epoch: 0,
                idx: 0,
                bytes: vec![1, 2, 3],
            };
            rbc_chunk_tx
                .send(inbound(BlockMessage::RbcChunk(rbc_chunk)))
                .expect("send rbc chunk");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

            let proposal = Proposal {
                header: ConsensusBlockHeader {
                    parent_hash: block_hash,
                    tx_root: Hash::new(b"tx"),
                    state_root: Hash::new(b"state"),
                    proposer: 0,
                    height: 1,
                    view: 0,
                    epoch: 0,
                    highest_qc: QcHeaderRef {
                        height: 0,
                        view: 0,
                        epoch: 0,
                        subject_block_hash: block_hash,
                        phase: Phase::Prepare,
                    },
                },
                payload_hash: Hash::new(b"payload"),
            };
            block_payload_tx
                .send(inbound(BlockMessage::Proposal(proposal)))
                .expect("send proposal");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);

            block_tx
                .send(inbound(BlockMessage::ConsensusParams(
                    message::ConsensusParamsAdvert {
                        collectors_k: 1,
                        redundant_send_r: 1,
                        membership: None,
                    },
                )))
                .expect("send consensus params");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);

            let v1 = Vote {
                phase: Phase::Prepare,
                block_hash,
                parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                height: 1,
                view: 1,
                epoch: 0,
                highest_qc: None,
                signer: 0,
                bls_sig: Vec::new(),
            };
            let v2 = Vote {
                phase: Phase::Prepare,
                block_hash,
                parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                height: 1,
                view: 1,
                epoch: 0,
                highest_qc: None,
                signer: 0,
                bls_sig: Vec::new(),
            };
            let evidence = crate::sumeragi::consensus::Evidence {
                kind: crate::sumeragi::consensus::EvidenceKind::DoublePrepare,
                payload: crate::sumeragi::consensus::EvidencePayload::DoubleVote { v1, v2 },
            };
            consensus_tx
                .send(ControlFlow::Evidence(evidence))
                .expect("send evidence");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Consensus);

            let merge_signature = MergeCommitteeSignature {
                epoch_id: 0,
                view: 0,
                signer: ValidatorIndex::from(0_u32),
                message_digest: Hash::new(b"merge"),
                bls_sig: Vec::new(),
            };
            lane_tx
                .send(LaneRelayMessage::MergeSignature(merge_signature))
                .expect("send merge signature");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);

            background_tx
                .send(BackgroundRequest::Broadcast {
                    msg: BlockMessage::ConsensusParams(message::ConsensusParamsAdvert {
                        collectors_k: 2,
                        redundant_send_r: 2,
                        membership: None,
                    }),
                })
                .expect("send background request");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Background);

            let config = WorkerLoopConfig {
                time_budget: Duration::from_secs(1),
                vote_rx_drain_budget: Duration::from_secs(1),
                block_payload_rx_drain_budget: Duration::from_secs(1),
                block_payload_rx_drain_max_messages: 16,
                vote_rx_drain_max_messages: 16,
                block_rx_drain_budget: Duration::from_secs(1),
                block_rx_drain_max_messages: 16,
                rbc_chunk_rx_drain_budget: Duration::from_secs(1),
                rbc_chunk_rx_drain_max_messages: 16,
                consensus_rx_drain_max_messages: 16,
                lane_relay_rx_drain_max_messages: 16,
                background_rx_drain_max_messages: 16,
                tick_min_gap: Duration::from_millis(1),
                tick_max_gap: Duration::from_secs(1),
                block_rx_starve_max: Duration::from_secs(1),
                non_vote_starve_max: Duration::from_secs(1),
            };
            let now = Instant::now();
            let mut loop_state = WorkerLoopState {
                last_tick: now,
                last_served: [now; PRIORITY_TIER_COUNT],
                mailbox: WorkerMailboxState::new(),
            };
            let mut actor = RecordingActor::default();

            run_worker_iteration(
                &mut actor,
                &config,
                &mut loop_state,
                &vote_rx,
                &block_payload_rx,
                &rbc_chunk_rx,
                &block_rx,
                &consensus_rx,
                &lane_rx,
                &background_rx,
            );

            actor.events
        }

        status::reset_worker_loop_snapshot_for_tests();
        let first = run_trace();
        status::reset_worker_loop_snapshot_for_tests();
        let second = run_trace();

        assert_eq!(first, second);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn worker_queue_depths_track_enqueues_and_drains() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let vote_dedup = Arc::new(Mutex::new(DedupCache::new(
            VOTE_DEDUP_CACHE_CAP,
            VOTE_DEDUP_CACHE_TTL,
        )));
        let block_payload_dedup = Arc::new(Mutex::new(BlockPayloadDedupCache::new(
            BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
            BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
        )));
        let handle = SumeragiHandle::new(
            block_payload_tx,
            block_tx,
            rbc_chunk_tx,
            vote_tx,
            consensus_tx,
            background_tx,
            lane_tx,
            Arc::clone(&vote_dedup),
            Arc::clone(&block_payload_dedup),
        );

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let vote = Vote {
            phase: Phase::Prepare,
            block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        handle.incoming_block_message(BlockMessage::QcVote(vote));

        let rbc_chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        handle.incoming_block_message(BlockMessage::RbcChunk(rbc_chunk));

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash: Hash::new(b"payload"),
        };
        handle.incoming_block_message(BlockMessage::Proposal(proposal));

        handle.incoming_block_message(BlockMessage::ConsensusParams(
            message::ConsensusParamsAdvert {
                collectors_k: 1,
                redundant_send_r: 1,
                membership: None,
            },
        ));

        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        handle.post_to_peer(
            peer_id,
            BlockMessage::ConsensusParams(message::ConsensusParamsAdvert {
                collectors_k: 2,
                redundant_send_r: 1,
                membership: None,
            }),
        );
        handle.broadcast(BlockMessage::ConsensusParams(
            message::ConsensusParamsAdvert {
                collectors_k: 3,
                redundant_send_r: 1,
                membership: None,
            },
        ));

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let commitment = LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
        };
        let envelope =
            LaneRelayEnvelope::new(header, None, None, commitment, 0).expect("relay envelope");
        handle.incoming_lane_relay(envelope);
        handle.incoming_merge_signature(MergeCommitteeSignature {
            epoch_id: 0,
            view: 0,
            signer: 0,
            message_digest: Hash::new(b"merge-signature"),
            bls_sig: vec![0x22; 48],
        });

        let depths = status::worker_queue_depth_snapshot();
        assert_eq!(depths.vote_rx, 1);
        assert_eq!(depths.block_payload_rx, 1);
        assert_eq!(depths.rbc_chunk_rx, 1);
        assert_eq!(depths.block_rx, 1);
        assert_eq!(depths.background_rx, 2);
        assert_eq!(depths.lane_relay_rx, 2);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActor::default();

        run_worker_iteration(
            &mut actor,
            &config,
            &mut state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_rx,
            &background_rx,
        );

        let depths = status::worker_queue_depth_snapshot();
        assert_eq!(depths.vote_rx, 0);
        assert_eq!(depths.block_payload_rx, 0);
        assert_eq!(depths.rbc_chunk_rx, 0);
        assert_eq!(depths.block_rx, 0);
        assert_eq!(depths.background_rx, 0);
        assert_eq!(depths.lane_relay_rx, 0);
    }

    #[test]
    fn run_worker_loop_exits_when_shutdown_is_sent() {
        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_wake_tx, wake_rx) = mpsc::sync_channel::<()>(WORKER_WAKE_CHANNEL_CAP);
        let mut actor = RecordingActor::default();
        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let shutdown_signal = ShutdownSignal::new();
        shutdown_signal.send();

        run_worker_loop(
            &mut actor,
            config,
            state,
            vote_rx,
            block_payload_rx,
            rbc_chunk_rx,
            block_rx,
            consensus_rx,
            lane_rx,
            background_rx,
            wake_rx,
            shutdown_signal,
        );

        assert!(actor.events.is_empty());
        assert_eq!(actor.tick_calls, 0);
    }

    #[test]
    fn run_worker_loop_refreshes_config_each_iteration() {
        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(WORKER_WAKE_CHANNEL_CAP);
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let mut actor = RefreshingActor {
            refresh_count: Arc::clone(&refresh_count),
        };
        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let shutdown_signal = ShutdownSignal::new();
        let shutdown_worker = shutdown_signal.clone();

        let join = std::thread::spawn(move || {
            run_worker_loop(
                &mut actor,
                config,
                state,
                vote_rx,
                block_payload_rx,
                rbc_chunk_rx,
                block_rx,
                consensus_rx,
                lane_rx,
                background_rx,
                wake_rx,
                shutdown_worker,
            );
        });

        let deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while refresh_count.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        shutdown_signal.send();
        let _ = wake_tx.send(());
        join.join().expect("worker loop thread");

        assert!(refresh_count.load(Ordering::Relaxed) > 0);
    }
}

/// QC-based consensus message types and helpers (single-chain).
pub mod collectors;
pub mod consensus;
pub mod da;
pub mod election;
pub mod epoch;
pub mod epoch_report;
pub(crate) mod evidence;
pub(crate) mod exec;
pub mod main_loop;
pub mod message;
pub mod network_topology;
pub(crate) mod new_view_stats;
pub(crate) mod penalties;
pub mod rbc_sampling;
pub mod rbc_status;
pub mod rbc_store;
pub(crate) mod smt;
pub(crate) mod stake_snapshot;
pub mod status;
pub mod witness;
pub use evidence::EvidenceValidationContext;
pub use evidence::evidence_subject_height_view;

/// Validate an evidence payload using the canonical rules.
///
/// # Errors
///
/// Propagates [`EvidenceValidationError`](evidence::EvidenceValidationError) when the payload
/// fails any of the structural or metadata consistency checks enforced by consensus.
pub fn validate_evidence(
    evidence: &consensus::Evidence,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), evidence::EvidenceValidationError> {
    evidence::validate_evidence(evidence, context)
}

/// Placeholder for in-flight voting block state tracked by consensus.
#[derive(Debug, Clone, Copy, Default)]
pub struct VotingBlock;

/// Public snapshot of `NEW_VIEW` counts for operator-facing APIs (SSE/JSON).
pub fn new_view_snapshot_counts() -> Vec<(u64, u64, u64)> {
    new_view_stats::snapshot_counts()
}

/// Public snapshot of leader index and `HighestQC` tuple for status endpoints.
pub use status::StatusSnapshot;

/// Return the latest consensus status snapshot (leader, QCs, drop counters).
pub fn status_snapshot() -> StatusSnapshot {
    status::snapshot()
}

use self::message::*;
#[cfg(feature = "telemetry")]
use crate::telemetry::Telemetry;
use crate::{
    EventsSender, IrohaNetwork, kura::Kura, peers_gossiper::PeersGossiperHandle, queue::Queue,
};

/// Bundle of genesis block and its publishing key.
#[derive(Clone)]
pub struct GenesisWithPubKey {
    /// Optional genesis block to seed the chain; `None` when submitted elsewhere.
    pub genesis: Option<GenesisBlock>,
    /// Public key used to sign the genesis payload.
    pub public_key: PublicKey,
}

/// Configuration for the persisted RBC session store.
#[derive(Clone)]
pub struct RbcStoreConfig {
    /// Filesystem directory where sessions are stored.
    pub dir: PathBuf,
    /// Hard cap on concurrent persisted sessions.
    pub max_sessions: usize,
    /// Soft limit on concurrent sessions before pressure is applied.
    pub soft_sessions: usize,
    /// Hard cap on the total persisted payload size in bytes.
    pub max_bytes: usize,
    /// Soft limit on persisted payload bytes.
    pub soft_bytes: usize,
    /// Time-to-live for cached sessions before eviction.
    pub ttl: Duration,
}

/// Background posting tasks issued by Sumeragi.
#[derive(Debug, Clone)]
pub enum BackgroundPost {
    /// Post a consensus message to a specific peer.
    Post {
        /// Destination peer identifier.
        peer: PeerId,
        /// Message to dispatch.
        msg: BlockMessage,
        /// Time when the task was enqueued.
        enqueued_at: Instant,
    },
    /// Post a consensus control-flow frame to a specific peer.
    PostControlFlow {
        /// Destination peer identifier.
        peer: PeerId,
        /// Control-flow frame to dispatch.
        frame: ControlFlow,
        /// Time when the task was enqueued.
        enqueued_at: Instant,
    },
    /// Broadcast a consensus message to all peers.
    Broadcast {
        /// Message to broadcast.
        msg: BlockMessage,
        /// Time when the task was enqueued.
        enqueued_at: Instant,
    },
    /// Broadcast a consensus control-flow frame to all peers.
    BroadcastControlFlow {
        /// Frame to broadcast.
        frame: ControlFlow,
        /// Time when the task was enqueued.
        enqueued_at: Instant,
    },
}

/// Internal request type enqueued by subsystems that need the actor to schedule a background
/// consensus transmission (control frames, payloads, RBC chunks, etc.).
#[derive(Debug, Clone)]
pub enum BackgroundRequest {
    /// Send a point-to-point `BlockMessage` to the given peer.
    Post {
        /// Peer that should receive the consensus message.
        peer: PeerId,
        /// Consensus message to send to the peer.
        msg: BlockMessage,
    },
    /// Send a point-to-point control-flow frame to the given peer.
    PostControlFlow {
        /// Peer that should receive the control-flow frame.
        peer: PeerId,
        /// Control-flow frame to send to the peer.
        frame: ControlFlow,
    },
    /// Broadcast a `BlockMessage` to all peers.
    Broadcast {
        /// Consensus message to broadcast.
        msg: BlockMessage,
    },
    /// Broadcast a control-flow frame to all peers.
    BroadcastControlFlow {
        /// Control-flow frame to broadcast.
        frame: ControlFlow,
    },
}

type VoteDedupKey = (
    u8,
    HashOf<BlockHeader>,
    u64,
    u64,
    u64,
    crate::sumeragi::consensus::ValidatorIndex,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum BlockPayloadDedupKey {
    BlockCreated {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    },
    Proposal {
        height: u64,
        view: u64,
        payload_hash: CryptoHash,
    },
    RbcReady {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        sender: crate::sumeragi::consensus::ValidatorIndex,
        signature_hash: CryptoHash,
    },
    RbcDeliver {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        sender: crate::sumeragi::consensus::ValidatorIndex,
        signature_hash: CryptoHash,
    },
    BlockSyncUpdate {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        evidence_hash: CryptoHash,
    },
    RbcChunk {
        height: u64,
        view: u64,
        epoch: u64,
        block_hash: HashOf<BlockHeader>,
        idx: u32,
        bytes_hash: CryptoHash,
    },
}

const VOTE_DEDUP_CACHE_CAP: usize = 8192;
const VOTE_DEDUP_CACHE_TTL: Duration = Duration::from_secs(60);
const BLOCK_PAYLOAD_DEDUP_CACHE_CAP: usize = 8192;
const BLOCK_PAYLOAD_DEDUP_CACHE_TTL: Duration = Duration::from_secs(120);
const BLOCK_PAYLOAD_DEDUP_KIND_COUNT: usize = 6;
const BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND: usize =
    if BLOCK_PAYLOAD_DEDUP_CACHE_CAP / BLOCK_PAYLOAD_DEDUP_KIND_COUNT == 0 {
        1
    } else {
        BLOCK_PAYLOAD_DEDUP_CACHE_CAP / BLOCK_PAYLOAD_DEDUP_KIND_COUNT
    };

fn block_sync_update_evidence_hash(update: &message::BlockSyncUpdate) -> CryptoHash {
    let commit_votes_hash = CryptoHash::new(&update.commit_votes.encode());
    let commit_qc_hash = update
        .commit_qc
        .as_ref()
        .map(|qc| CryptoHash::new(&qc.encode()));
    let checkpoint_hash = update
        .validator_checkpoint
        .as_ref()
        .map(|checkpoint| CryptoHash::new(&checkpoint.encode()));
    let stake_snapshot_hash = update
        .stake_snapshot
        .as_ref()
        .map(|snapshot| CryptoHash::new(&snapshot.encode()));
    let mut buf = Vec::new();
    buf.extend_from_slice(commit_votes_hash.as_ref());
    push_optional_hash(&mut buf, commit_qc_hash);
    push_optional_hash(&mut buf, checkpoint_hash);
    push_optional_hash(&mut buf, stake_snapshot_hash);
    CryptoHash::new(&buf)
}

fn push_optional_hash(buf: &mut Vec<u8>, hash: Option<CryptoHash>) {
    buf.push(hash.is_some() as u8);
    if let Some(hash) = hash {
        buf.extend_from_slice(hash.as_ref());
    }
}

#[derive(Debug)]
struct DedupEntry {
    last_seen: Instant,
    order: u64,
}

#[derive(Debug, Clone, Copy)]
struct DedupInsertOutcome {
    inserted: bool,
    evicted_capacity: usize,
    evicted_expired: usize,
}

#[derive(Debug)]
struct DedupCache<T>
where
    T: Ord + Clone,
{
    cap: usize,
    ttl: Duration,
    next_order: u64,
    entries: BTreeMap<T, DedupEntry>,
    lru: BTreeSet<(u64, T)>,
}

impl<T> DedupCache<T>
where
    T: Ord + Clone,
{
    fn new(cap: usize, ttl: Duration) -> Self {
        Self {
            cap: cap.max(1),
            ttl,
            next_order: 0,
            entries: BTreeMap::new(),
            lru: BTreeSet::new(),
        }
    }

    fn insert(&mut self, key: T, now: Instant) -> DedupInsertOutcome {
        let evicted_expired = self.evict_expired(now);
        if let Some(mut entry) = self.entries.remove(&key) {
            let order = if self.lru.remove(&(entry.order, key.clone())) {
                self.next_order()
            } else {
                entry.order
            };
            entry.order = order;
            entry.last_seen = now;
            self.entries.insert(key.clone(), entry);
            self.lru.insert((order, key));
            return DedupInsertOutcome {
                inserted: false,
                evicted_capacity: 0,
                evicted_expired,
            };
        }

        let evicted_capacity = self.evict_capacity();
        let order = self.next_order();
        self.entries.insert(
            key.clone(),
            DedupEntry {
                last_seen: now,
                order,
            },
        );
        self.lru.insert((order, key));
        DedupInsertOutcome {
            inserted: true,
            evicted_capacity,
            evicted_expired,
        }
    }

    fn remove(&mut self, key: &T) -> bool {
        let Some(entry) = self.entries.remove(key) else {
            return false;
        };
        self.lru.remove(&(entry.order, key.clone()));
        true
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        order
    }

    fn evict_capacity(&mut self) -> usize {
        let mut evicted = 0usize;
        while self.entries.len() >= self.cap {
            let Some((order, key)) = self.lru.pop_first() else {
                self.entries.clear();
                break;
            };
            if self.entries.remove(&key).is_some() {
                evicted = evicted.saturating_add(1);
            } else {
                self.lru.remove(&(order, key));
            }
        }
        evicted
    }

    fn evict_expired(&mut self, now: Instant) -> usize {
        if self.ttl == Duration::ZERO {
            return 0;
        }
        let mut expired = Vec::new();
        for (key, entry) in &self.entries {
            if now.saturating_duration_since(entry.last_seen) >= self.ttl {
                expired.push((key.clone(), entry.order));
            }
        }
        let mut evicted = 0usize;
        for (key, order) in expired {
            if self.entries.remove(&key).is_some() {
                self.lru.remove(&(order, key));
                evicted = evicted.saturating_add(1);
            }
        }
        evicted
    }

    #[cfg(test)]
    fn contains(&self, key: &T) -> bool {
        self.entries.contains_key(key)
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.lru.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug)]
struct BlockPayloadDedupCache {
    block_created: DedupCache<BlockPayloadDedupKey>,
    proposal: DedupCache<BlockPayloadDedupKey>,
    rbc_ready: DedupCache<BlockPayloadDedupKey>,
    rbc_deliver: DedupCache<BlockPayloadDedupKey>,
    block_sync_update: DedupCache<BlockPayloadDedupKey>,
    rbc_chunk: DedupCache<BlockPayloadDedupKey>,
}

impl BlockPayloadDedupCache {
    fn new(cap_per_kind: usize, ttl: Duration) -> Self {
        Self {
            block_created: DedupCache::new(cap_per_kind, ttl),
            proposal: DedupCache::new(cap_per_kind, ttl),
            rbc_ready: DedupCache::new(cap_per_kind, ttl),
            rbc_deliver: DedupCache::new(cap_per_kind, ttl),
            block_sync_update: DedupCache::new(cap_per_kind, ttl),
            rbc_chunk: DedupCache::new(cap_per_kind, ttl),
        }
    }

    fn insert(&mut self, key: BlockPayloadDedupKey, now: Instant) -> DedupInsertOutcome {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.insert(key, now),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.insert(key, now),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.insert(key, now),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.insert(key, now),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.insert(key, now),
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.insert(key, now),
        }
    }

    fn remove(&mut self, key: &BlockPayloadDedupKey) -> bool {
        match *key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.remove(key),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.remove(key),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.remove(key),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.remove(key),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.remove(key),
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.remove(key),
        }
    }

    #[cfg(test)]
    fn contains(&self, key: &BlockPayloadDedupKey) -> bool {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.contains(key),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.contains(key),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.contains(key),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.contains(key),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.contains(key),
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.contains(key),
        }
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.block_created.clear();
        self.proposal.clear();
        self.rbc_ready.clear();
        self.rbc_deliver.clear();
        self.block_sync_update.clear();
        self.rbc_chunk.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.block_created.len()
            + self.proposal.len()
            + self.rbc_ready.len()
            + self.rbc_deliver.len()
            + self.block_sync_update.len()
            + self.rbc_chunk.len()
    }

    #[cfg(test)]
    fn len_for_key(&self, key: &BlockPayloadDedupKey) -> usize {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.len(),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.len(),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.len(),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.len(),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.len(),
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.len(),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum LaneRelayMessage {
    Envelope(LaneRelayEnvelope),
    MergeSignature(MergeCommitteeSignature),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressMode {
    Blocking,
    NonBlocking,
}

#[derive(Clone, Debug)]
pub(crate) struct InboundBlockMessage {
    message: BlockMessage,
    sender: Option<PeerId>,
}

impl InboundBlockMessage {
    fn new(message: BlockMessage, sender: Option<PeerId>) -> Self {
        Self { message, sender }
    }
}

/// Minimal handle for the Sumeragi actor.
#[derive(Clone)]
pub struct SumeragiHandle {
    block_payload: mpsc::SyncSender<InboundBlockMessage>,
    block: mpsc::SyncSender<InboundBlockMessage>,
    rbc_chunks: mpsc::SyncSender<InboundBlockMessage>,
    votes: mpsc::SyncSender<InboundBlockMessage>,
    consensus_control: mpsc::SyncSender<ControlFlow>,
    background: mpsc::SyncSender<BackgroundRequest>,
    lane_relay: mpsc::SyncSender<LaneRelayMessage>,
    wake: Option<mpsc::SyncSender<()>>,
    vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>>,
    block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>>,
}

impl SumeragiHandle {
    fn phase_id(phase: crate::sumeragi::consensus::Phase) -> u8 {
        match phase {
            crate::sumeragi::consensus::Phase::Prepare => 0,
            crate::sumeragi::consensus::Phase::Commit => 1,
            crate::sumeragi::consensus::Phase::NewView => 2,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        block_payload_tx: mpsc::SyncSender<InboundBlockMessage>,
        block_tx: mpsc::SyncSender<InboundBlockMessage>,
        rbc_chunk_tx: mpsc::SyncSender<InboundBlockMessage>,
        vote_tx: mpsc::SyncSender<InboundBlockMessage>,
        consensus_control_tx: mpsc::SyncSender<ControlFlow>,
        background_tx: mpsc::SyncSender<BackgroundRequest>,
        lane_relay_tx: mpsc::SyncSender<LaneRelayMessage>,
        vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>>,
        block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>>,
    ) -> Self {
        Self {
            block_payload: block_payload_tx,
            block: block_tx,
            rbc_chunks: rbc_chunk_tx,
            votes: vote_tx,
            consensus_control: consensus_control_tx,
            background: background_tx,
            lane_relay: lane_relay_tx,
            wake: None,
            vote_dedup,
            block_payload_dedup,
        }
    }

    fn with_wake(mut self, wake: mpsc::SyncSender<()>) -> Self {
        self.wake = Some(wake);
        self
    }

    fn wake(&self) {
        if let Some(wake) = self.wake.as_ref() {
            let _ = wake.try_send(());
        }
    }

    fn dedup_vote(&self, key: VoteDedupKey) -> bool {
        let mut guard = self.vote_dedup.lock().expect("vote dedup cache poisoned");
        let outcome = guard.insert(key, Instant::now());
        if outcome.evicted_capacity > 0 || outcome.evicted_expired > 0 {
            status::record_dedup_evictions(
                status::DedupEvictionKind::Vote,
                outcome.evicted_capacity,
                outcome.evicted_expired,
            );
        }
        outcome.inserted
    }

    fn dedup_block_payload(&self, key: BlockPayloadDedupKey) -> bool {
        let kind = match key {
            BlockPayloadDedupKey::BlockCreated { .. } => status::DedupEvictionKind::BlockCreated,
            BlockPayloadDedupKey::Proposal { .. } => status::DedupEvictionKind::Proposal,
            BlockPayloadDedupKey::RbcReady { .. } => status::DedupEvictionKind::RbcReady,
            BlockPayloadDedupKey::RbcDeliver { .. } => status::DedupEvictionKind::RbcDeliver,
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => {
                status::DedupEvictionKind::BlockSyncUpdate
            }
            BlockPayloadDedupKey::RbcChunk { .. } => status::DedupEvictionKind::RbcChunk,
        };
        let mut guard = self
            .block_payload_dedup
            .lock()
            .expect("block payload dedup cache poisoned");
        let outcome = guard.insert(key, Instant::now());
        if outcome.evicted_capacity > 0 || outcome.evicted_expired > 0 {
            status::record_dedup_evictions(kind, outcome.evicted_capacity, outcome.evicted_expired);
        }
        outcome.inserted
    }

    fn release_block_payload_dedup(&self, key: &BlockPayloadDedupKey) {
        let mut guard = self
            .block_payload_dedup
            .lock()
            .expect("block payload dedup cache poisoned");
        guard.remove(key);
    }

    /// Enqueue an incoming block-synchronization or consensus message.
    #[allow(clippy::too_many_lines)]
    pub fn incoming_block_message(&self, msg: BlockMessage) {
        let _ = self.incoming_block_message_with_sender(msg, None, IngressMode::Blocking);
    }

    /// Enqueue an incoming block-synchronization or consensus message from a known peer.
    pub fn incoming_block_message_from(&self, sender: PeerId, msg: BlockMessage) {
        let _ = self.incoming_block_message_with_sender(msg, Some(sender), IngressMode::Blocking);
    }

    /// Enqueue an incoming block message without blocking the caller.
    /// Returns `true` if the message was accepted by the queue.
    ///
    /// Note: this is a best-effort enqueue that drops messages when queues are saturated
    /// to avoid stalling upstream relays. Block-sync updates and critical payload messages
    /// (block creation and proposals), plus RBC READY/DELIVER and QC votes/certificates,
    /// always use blocking semantics because dropping them can stall consensus recovery.
    pub fn try_incoming_block_message(&self, msg: BlockMessage) -> bool {
        let blocking = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(_)
                | BlockMessage::BlockCreated(_)
                | BlockMessage::Proposal(_)
                | BlockMessage::QcVote(_)
                | BlockMessage::Qc(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_)
        );
        let mode = if blocking {
            IngressMode::Blocking
        } else {
            IngressMode::NonBlocking
        };
        self.incoming_block_message_with_sender(msg, None, mode)
    }

    /// Enqueue an incoming block message from a known peer without blocking the caller.
    /// Returns `true` if the message was accepted by the queue.
    pub fn try_incoming_block_message_from(&self, sender: PeerId, msg: BlockMessage) -> bool {
        let blocking = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(_)
                | BlockMessage::BlockCreated(_)
                | BlockMessage::Proposal(_)
                | BlockMessage::QcVote(_)
                | BlockMessage::Qc(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_)
        );
        let mode = if blocking {
            IngressMode::Blocking
        } else {
            IngressMode::NonBlocking
        };
        self.incoming_block_message_with_sender(msg, Some(sender), mode)
    }

    #[allow(clippy::too_many_lines)]
    fn incoming_block_message_with_sender(
        &self,
        msg: BlockMessage,
        sender: Option<PeerId>,
        mode: IngressMode,
    ) -> bool {
        let inbound = InboundBlockMessage::new(msg, sender);
        let log_drop = |kind: &'static str,
                        queue: status::WorkerQueueKind,
                        msg: &InboundBlockMessage,
                        reason: &'static str| {
            match &msg.message {
                BlockMessage::RbcChunk(chunk) => {
                    iroha_logger::warn!(
                        height = chunk.height,
                        view = chunk.view,
                        idx = chunk.idx,
                        block = %chunk.block_hash,
                        queue = ?queue,
                        reason,
                        "dropping RBC chunk message"
                    );
                }
                BlockMessage::RbcInit(init) => {
                    iroha_logger::warn!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        total_chunks = init.total_chunks,
                        queue = ?queue,
                        reason,
                        "dropping RBC init message"
                    );
                }
                BlockMessage::RbcReady(ready) => {
                    iroha_logger::warn!(
                        height = ready.height,
                        view = ready.view,
                        sender = ready.sender,
                        block = %ready.block_hash,
                        queue = ?queue,
                        reason,
                        "dropping RBC READY message"
                    );
                }
                BlockMessage::RbcDeliver(deliver) => {
                    iroha_logger::warn!(
                        height = deliver.height,
                        view = deliver.view,
                        sender = deliver.sender,
                        block = %deliver.block_hash,
                        queue = ?queue,
                        reason,
                        "dropping RBC DELIVER message"
                    );
                }
                BlockMessage::BlockCreated(created) => {
                    let header = created.block.header();
                    iroha_logger::warn!(
                        height = header.height().get(),
                        view = header.view_change_index(),
                        block = %created.block.hash(),
                        queue = ?queue,
                        reason,
                        "dropping BlockCreated message"
                    );
                }
                BlockMessage::Proposal(proposal) => {
                    iroha_logger::warn!(
                        height = proposal.header.height,
                        view = proposal.header.view,
                        payload = %proposal.payload_hash,
                        queue = ?queue,
                        reason,
                        "dropping Proposal message"
                    );
                }
                BlockMessage::BlockSyncUpdate(update) => {
                    let header = update.block.header();
                    iroha_logger::warn!(
                        height = header.height().get(),
                        view = header.view_change_index(),
                        block = %update.block.hash(),
                        queue = ?queue,
                        reason,
                        "dropping BlockSyncUpdate message"
                    );
                }
                _ => {
                    iroha_logger::debug!(
                        kind,
                        queue = ?queue,
                        reason,
                        "dropping block message"
                    );
                }
            }
        };
        let wake = || self.wake();
        let enqueue_with_mode = |tx: &mpsc::SyncSender<InboundBlockMessage>,
                                 msg: InboundBlockMessage,
                                 kind: &'static str,
                                 queue: status::WorkerQueueKind,
                                 mode: IngressMode| match mode {
            IngressMode::Blocking => {
                wake();
                let start = Instant::now();
                match tx.send(msg) {
                    Ok(()) => {
                        status::record_worker_queue_enqueue(queue);
                        status::record_worker_queue_blocked(queue, start.elapsed());
                        true
                    }
                    Err(err) => {
                        status::record_worker_queue_drop(queue);
                        iroha_logger::warn!(?err, kind, "Sumeragi actor dropped block message");
                        false
                    }
                }
            }
            IngressMode::NonBlocking => match tx.try_send(msg) {
                Ok(()) => {
                    status::record_worker_queue_enqueue(queue);
                    wake();
                    true
                }
                Err(mpsc::TrySendError::Full(msg)) => {
                    status::record_worker_queue_drop(queue);
                    log_drop(kind, queue, &msg, "channel_full");
                    false
                }
                Err(mpsc::TrySendError::Disconnected(msg)) => {
                    status::record_worker_queue_drop(queue);
                    log_drop(kind, queue, &msg, "channel_disconnected");
                    false
                }
            },
        };
        let InboundBlockMessage {
            message: msg,
            sender,
        } = inbound;
        match msg {
            BlockMessage::QcVote(vote) => {
                let duplicate = !self.dedup_vote((
                    Self::phase_id(vote.phase),
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    vote.epoch,
                    vote.signer,
                ));
                if duplicate {
                    iroha_logger::debug!(
                        phase = ?vote.phase,
                        height = vote.height,
                        view = vote.view,
                        epoch = vote.epoch,
                        signer = vote.signer,
                        block_hash = %vote.block_hash,
                        "dropping duplicate commit vote from network"
                    );
                    return false;
                }
                iroha_logger::debug!(
                    phase = ?vote.phase,
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    "enqueueing commit vote from network"
                );
                enqueue_with_mode(
                    &self.votes,
                    InboundBlockMessage::new(BlockMessage::QcVote(vote), sender),
                    "QcVote",
                    status::WorkerQueueKind::Votes,
                    mode,
                )
            }
            BlockMessage::Qc(cert) => enqueue_with_mode(
                &self.votes,
                InboundBlockMessage::new(BlockMessage::Qc(cert), sender),
                "Qc",
                status::WorkerQueueKind::Votes,
                mode,
            ),
            BlockMessage::ProposalHint(hint) => enqueue_with_mode(
                &self.votes,
                InboundBlockMessage::new(BlockMessage::ProposalHint(hint), sender),
                "ProposalHint",
                status::WorkerQueueKind::Votes,
                mode,
            ),
            BlockMessage::RbcReady(message) => {
                let height = message.height;
                let view = message.view;
                let block_hash = message.block_hash;
                let validator_idx = message.sender;
                let signature_hash = CryptoHash::new(&message.signature);
                let dedup_key = BlockPayloadDedupKey::RbcReady {
                    height,
                    view,
                    block_hash,
                    sender: validator_idx,
                    signature_hash,
                };
                let duplicate = !self.dedup_block_payload(dedup_key);
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        block = %block_hash,
                        sender = validator_idx,
                        "dropping duplicate RBC READY from network"
                    );
                    return false;
                }
                let accepted = enqueue_with_mode(
                    &self.votes,
                    InboundBlockMessage::new(BlockMessage::RbcReady(message), sender),
                    "RbcReady",
                    status::WorkerQueueKind::Votes,
                    mode,
                );
                if !accepted {
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::RbcDeliver(message) => {
                let height = message.height;
                let view = message.view;
                let block_hash = message.block_hash;
                let validator_idx = message.sender;
                let signature_hash = CryptoHash::new(&message.signature);
                let dedup_key = BlockPayloadDedupKey::RbcDeliver {
                    height,
                    view,
                    block_hash,
                    sender: validator_idx,
                    signature_hash,
                };
                let duplicate = !self.dedup_block_payload(dedup_key);
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        block = %block_hash,
                        sender = validator_idx,
                        "dropping duplicate RBC DELIVER from network"
                    );
                    return false;
                }
                let accepted = enqueue_with_mode(
                    &self.votes,
                    InboundBlockMessage::new(BlockMessage::RbcDeliver(message), sender),
                    "RbcDeliver",
                    status::WorkerQueueKind::Votes,
                    mode,
                );
                if !accepted {
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::BlockCreated(created) => {
                let header = created.block.header();
                let height = header.height().get();
                let view = header.view_change_index();
                let block_hash = created.block.hash();
                let duplicate = !self.dedup_block_payload(BlockPayloadDedupKey::BlockCreated {
                    height,
                    view,
                    block_hash,
                });
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        block = %block_hash,
                        "dropping duplicate BlockCreated from network"
                    );
                    return false;
                }
                enqueue_with_mode(
                    &self.block_payload,
                    InboundBlockMessage::new(BlockMessage::BlockCreated(created), sender),
                    "BlockPayload",
                    status::WorkerQueueKind::BlockPayload,
                    mode,
                )
            }
            BlockMessage::Proposal(proposal) => {
                let height = proposal.header.height;
                let view = proposal.header.view;
                let payload_hash = proposal.payload_hash;
                let duplicate = !self.dedup_block_payload(BlockPayloadDedupKey::Proposal {
                    height,
                    view,
                    payload_hash,
                });
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        payload = %payload_hash,
                        "dropping duplicate Proposal from network"
                    );
                    return false;
                }
                enqueue_with_mode(
                    &self.block_payload,
                    InboundBlockMessage::new(BlockMessage::Proposal(proposal), sender),
                    "BlockPayload",
                    status::WorkerQueueKind::BlockPayload,
                    mode,
                )
            }
            BlockMessage::BlockSyncUpdate(update) => {
                let header = update.block.header();
                let height = header.height().get();
                let view = header.view_change_index();
                let block_hash = update.block.hash();
                let evidence_hash = block_sync_update_evidence_hash(&update);
                let duplicate = !self.dedup_block_payload(BlockPayloadDedupKey::BlockSyncUpdate {
                    height,
                    view,
                    block_hash,
                    evidence_hash,
                });
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        block = %block_hash,
                        "dropping duplicate BlockSyncUpdate from network"
                    );
                    return false;
                }
                // Block sync updates carry commit/QC evidence; prioritize with payload traffic.
                enqueue_with_mode(
                    &self.block_payload,
                    InboundBlockMessage::new(BlockMessage::BlockSyncUpdate(update), sender),
                    "BlockSyncUpdate",
                    status::WorkerQueueKind::BlockPayload,
                    mode,
                )
            }
            BlockMessage::RbcInit(init) => enqueue_with_mode(
                &self.rbc_chunks,
                InboundBlockMessage::new(BlockMessage::RbcInit(init), sender),
                "RbcChunk",
                status::WorkerQueueKind::RbcChunks,
                mode,
            ),
            BlockMessage::RbcChunk(chunk) => {
                let height = chunk.height;
                let view = chunk.view;
                let epoch = chunk.epoch;
                let block_hash = chunk.block_hash;
                let idx = chunk.idx;
                let bytes_hash = CryptoHash::new(&chunk.bytes);
                let dedup_key = BlockPayloadDedupKey::RbcChunk {
                    height,
                    view,
                    epoch,
                    block_hash,
                    idx,
                    bytes_hash,
                };
                let inserted = self.dedup_block_payload(dedup_key);
                let duplicate = !inserted;
                if duplicate {
                    iroha_logger::debug!(
                        height,
                        view,
                        epoch,
                        idx,
                        block = %block_hash,
                        "dropping duplicate RBC chunk from network"
                    );
                    return false;
                }
                // Avoid blocking consensus ingress on bulk RBC chunks; rely on rebroadcasts.
                let accepted = enqueue_with_mode(
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::RbcChunk(chunk), sender),
                    "RbcChunk",
                    status::WorkerQueueKind::RbcChunks,
                    IngressMode::NonBlocking,
                );
                if !accepted {
                    // Allow rebroadcasts to be enqueued if the queue was full.
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::FetchPendingBlock(request) => enqueue_with_mode(
                &self.block_payload,
                InboundBlockMessage::new(BlockMessage::FetchPendingBlock(request), sender),
                "FetchPendingBlock",
                status::WorkerQueueKind::BlockPayload,
                mode,
            ),
            other => enqueue_with_mode(
                &self.block,
                InboundBlockMessage::new(other, sender),
                "BlockMessage",
                status::WorkerQueueKind::Blocks,
                mode,
            ),
        }
    }

    /// Enqueue a consensus control-flow command for the actor.
    pub fn incoming_consensus_control_flow_message(&self, msg: ControlFlow) {
        let _ = self.incoming_consensus_control_flow_message_with_mode(msg, IngressMode::Blocking);
    }

    /// Enqueue a consensus control-flow command without blocking the caller.
    /// Returns `true` if the message was accepted by the queue.
    pub fn try_incoming_consensus_control_flow_message(&self, msg: ControlFlow) -> bool {
        self.incoming_consensus_control_flow_message_with_mode(msg, IngressMode::NonBlocking)
    }

    fn incoming_consensus_control_flow_message_with_mode(
        &self,
        msg: ControlFlow,
        mode: IngressMode,
    ) -> bool {
        match mode {
            IngressMode::Blocking => {
                self.wake();
                let start = Instant::now();
                match self.consensus_control.send(msg) {
                    Ok(()) => {
                        status::record_worker_queue_enqueue(status::WorkerQueueKind::Consensus);
                        status::record_worker_queue_blocked(
                            status::WorkerQueueKind::Consensus,
                            start.elapsed(),
                        );
                        true
                    }
                    Err(err) => {
                        status::record_worker_queue_drop(status::WorkerQueueKind::Consensus);
                        iroha_logger::warn!(
                            ?err,
                            "Sumeragi actor dropped consensus control message"
                        );
                        false
                    }
                }
            }
            IngressMode::NonBlocking => match self.consensus_control.try_send(msg) {
                Ok(()) => {
                    status::record_worker_queue_enqueue(status::WorkerQueueKind::Consensus);
                    self.wake();
                    true
                }
                Err(mpsc::TrySendError::Full(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::Consensus);
                    iroha_logger::warn!("Sumeragi actor dropped consensus control message");
                    false
                }
                Err(mpsc::TrySendError::Disconnected(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::Consensus);
                    iroha_logger::warn!("Sumeragi actor dropped consensus control message");
                    false
                }
            },
        }
    }

    /// Enqueue an inbound lane relay envelope for processing.
    pub fn incoming_lane_relay(&self, envelope: LaneRelayEnvelope) {
        let _ = self.incoming_lane_relay_with_mode(envelope, IngressMode::Blocking);
    }

    /// Enqueue an inbound lane relay envelope without blocking the caller.
    /// Returns `true` if the message was accepted by the queue.
    pub fn try_incoming_lane_relay(&self, envelope: LaneRelayEnvelope) -> bool {
        self.incoming_lane_relay_with_mode(envelope, IngressMode::NonBlocking)
    }

    fn incoming_lane_relay_with_mode(
        &self,
        envelope: LaneRelayEnvelope,
        mode: IngressMode,
    ) -> bool {
        match mode {
            IngressMode::Blocking => {
                self.wake();
                let start = Instant::now();
                match self.lane_relay.send(LaneRelayMessage::Envelope(envelope)) {
                    Ok(()) => {
                        status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                        status::record_worker_queue_blocked(
                            status::WorkerQueueKind::LaneRelay,
                            start.elapsed(),
                        );
                        true
                    }
                    Err(err) => {
                        status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                        iroha_logger::warn!(?err, "Sumeragi actor dropped lane relay envelope");
                        false
                    }
                }
            }
            IngressMode::NonBlocking => match self
                .lane_relay
                .try_send(LaneRelayMessage::Envelope(envelope))
            {
                Ok(()) => {
                    status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                    self.wake();
                    true
                }
                Err(mpsc::TrySendError::Full(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                    iroha_logger::warn!("Sumeragi actor dropped lane relay envelope");
                    false
                }
                Err(mpsc::TrySendError::Disconnected(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                    iroha_logger::warn!("Sumeragi actor dropped lane relay envelope");
                    false
                }
            },
        }
    }

    /// Enqueue an inbound merge committee signature for processing.
    pub fn incoming_merge_signature(&self, signature: MergeCommitteeSignature) {
        let _ = self.incoming_merge_signature_with_mode(signature, IngressMode::Blocking);
    }

    /// Enqueue an inbound merge committee signature without blocking the caller.
    /// Returns `true` if the message was accepted by the queue.
    pub fn try_incoming_merge_signature(&self, signature: MergeCommitteeSignature) -> bool {
        self.incoming_merge_signature_with_mode(signature, IngressMode::NonBlocking)
    }

    fn incoming_merge_signature_with_mode(
        &self,
        signature: MergeCommitteeSignature,
        mode: IngressMode,
    ) -> bool {
        match mode {
            IngressMode::Blocking => {
                self.wake();
                let start = Instant::now();
                match self
                    .lane_relay
                    .send(LaneRelayMessage::MergeSignature(signature))
                {
                    Ok(()) => {
                        status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                        status::record_worker_queue_blocked(
                            status::WorkerQueueKind::LaneRelay,
                            start.elapsed(),
                        );
                        true
                    }
                    Err(err) => {
                        status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                        iroha_logger::warn!(?err, "Sumeragi actor dropped merge signature");
                        false
                    }
                }
            }
            IngressMode::NonBlocking => match self
                .lane_relay
                .try_send(LaneRelayMessage::MergeSignature(signature))
            {
                Ok(()) => {
                    status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                    self.wake();
                    true
                }
                Err(mpsc::TrySendError::Full(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                    iroha_logger::warn!("Sumeragi actor dropped merge signature");
                    false
                }
                Err(mpsc::TrySendError::Disconnected(_msg)) => {
                    status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                    iroha_logger::warn!("Sumeragi actor dropped merge signature");
                    false
                }
            },
        }
    }

    /// Schedule a high-priority consensus message to be posted to a specific peer.
    pub fn post_to_peer(&self, peer: PeerId, msg: BlockMessage) {
        self.wake();
        let start = Instant::now();
        match self.background.send(BackgroundRequest::Post { peer, msg }) {
            Ok(()) => {
                status::record_worker_queue_enqueue(status::WorkerQueueKind::Background);
                status::record_worker_queue_blocked(
                    status::WorkerQueueKind::Background,
                    start.elapsed(),
                );
            }
            Err(err) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::Background);
                iroha_logger::warn!(?err, "failed to enqueue background post task");
            }
        }
    }

    /// Schedule a consensus broadcast to all peers.
    pub fn broadcast(&self, msg: BlockMessage) {
        self.wake();
        let start = Instant::now();
        match self.background.send(BackgroundRequest::Broadcast { msg }) {
            Ok(()) => {
                status::record_worker_queue_enqueue(status::WorkerQueueKind::Background);
                status::record_worker_queue_blocked(
                    status::WorkerQueueKind::Background,
                    start.elapsed(),
                );
            }
            Err(err) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::Background);
                iroha_logger::warn!(?err, "failed to enqueue background broadcast task");
            }
        }
    }

    /// Schedule a control-flow broadcast to all peers.
    pub fn broadcast_control_flow(&self, frame: ControlFlow) {
        self.wake();
        let start = Instant::now();
        match self
            .background
            .send(BackgroundRequest::BroadcastControlFlow { frame })
        {
            Ok(()) => {
                status::record_worker_queue_enqueue(status::WorkerQueueKind::Background);
                status::record_worker_queue_blocked(
                    status::WorkerQueueKind::Background,
                    start.elapsed(),
                );
            }
            Err(err) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::Background);
                iroha_logger::warn!(?err, "failed to enqueue background control-flow task");
            }
        }
    }
}

/// Build a lightweight Sumeragi handle with a configurable block queue for unit tests.
#[cfg(test)]
pub(crate) fn test_sumeragi_handle(
    block_channel_cap: usize,
) -> (SumeragiHandle, mpsc::Receiver<InboundBlockMessage>) {
    let (block_payload_tx, _block_payload_rx) = mpsc::sync_channel(1);
    let (block_tx, block_rx) = mpsc::sync_channel(block_channel_cap);
    let (rbc_chunk_tx, _rbc_chunk_rx) = mpsc::sync_channel(1);
    let (vote_tx, _vote_rx) = mpsc::sync_channel(1);
    let (consensus_tx, _consensus_rx) = mpsc::sync_channel(1);
    let (background_tx, _background_rx) = mpsc::sync_channel(1);
    let (lane_tx, _lane_rx) = mpsc::sync_channel(1);
    let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(DedupCache::new(
        VOTE_DEDUP_CACHE_CAP,
        VOTE_DEDUP_CACHE_TTL,
    )));
    let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
        Arc::new(Mutex::new(BlockPayloadDedupCache::new(
            BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
            BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
        )));
    let handle = SumeragiHandle::new(
        block_payload_tx,
        block_tx,
        rbc_chunk_tx,
        vote_tx,
        consensus_tx,
        background_tx,
        lane_tx,
        vote_dedup,
        block_payload_dedup,
    );
    (handle, block_rx)
}

/// Adapter exposing a static roster derived from WSV peers.
#[derive(Clone)]
pub struct WsvEpochRosterAdapter {
    peers: Vec<PeerId>,
}

impl WsvEpochRosterAdapter {
    /// Construct an adapter from the current world-state peer roster.
    pub fn new(peers: Vec<PeerId>) -> Self {
        Self { peers }
    }

    /// Convenience helper for benches/tests to build a roster from iterators.
    #[cfg(any(test, feature = "bench-internal"))]
    pub fn from_peer_iter<I>(peers: I) -> Arc<Self>
    where
        I: IntoIterator<Item = PeerId>,
    {
        Arc::new(Self::new(peers.into_iter().collect()))
    }

    #[allow(dead_code)]
    /// Return the cached peer roster.
    pub fn peers(&self) -> &[PeerId] {
        &self.peers
    }
}

/// Spawn configuration for the Sumeragi actor.
pub struct SumeragiStartArgs {
    /// Consensus configuration knobs for the Sumeragi actor.
    pub config: SumeragiConfig,
    /// Common configuration shared with other subsystems (keys, peers, chain id).
    pub common_config: CommonConfig,
    /// Maximum consensus frame size (bytes) for control-plane messages (plaintext cap).
    pub consensus_frame_cap: usize,
    /// Maximum consensus payload frame size (bytes) for block/RBC payloads (plaintext cap).
    pub consensus_payload_frame_cap: usize,
    /// Channel used to emit consensus lifecycle events to observers.
    pub events_sender: EventsSender,
    /// Handle to the world state view.
    pub state: Arc<State>,
    /// Transaction queue shared with the pipeline.
    pub queue: Arc<Queue>,
    /// Persistent block store interface.
    pub kura: Arc<Kura>,
    /// Network transport handle for broadcasting consensus messages.
    pub network: IrohaNetwork,
    /// Gossiper handle used to propagate updated peer topologies.
    pub peers_gossiper: PeersGossiperHandle,
    /// Genesis network data augmented with leader public keys.
    pub genesis_network: GenesisWithPubKey,
    /// Current committed block count.
    pub block_count: BlockCount,
    /// Maximum fanout for block sync updates, availability votes, and `NEW_VIEW` gossip.
    pub block_sync_gossip_limit: usize,
    #[cfg(feature = "telemetry")]
    /// Telemetry sink describing enabled metrics/logging endpoints.
    pub telemetry: Telemetry,
    /// Provider yielding epoch rosters derived from WSV peers, when available.
    pub epoch_roster_provider: Option<Arc<WsvEpochRosterAdapter>>,
    /// Configuration for the reliable broadcast (RBC) store.
    pub rbc_store: Option<RbcStoreConfig>,
    /// Optional sender for background post-task execution.
    pub background_post_tx: Option<mpsc::SyncSender<BackgroundPost>>,
    /// Directory containing DA commitment spool artefacts emitted by Torii.
    pub da_spool_dir: PathBuf,
}

impl SumeragiStartArgs {
    /// Launch the Sumeragi actor, returning handles for control and supervision.
    #[allow(clippy::too_many_lines)]
    pub fn start(self, shutdown_signal: ShutdownSignal) -> (SumeragiHandle, Child) {
        let SumeragiStartArgs {
            config,
            common_config,
            consensus_frame_cap,
            consensus_payload_frame_cap,
            events_sender,
            state,
            queue,
            kura,
            network,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            rbc_store,
            background_post_tx,
            da_spool_dir,
        } = self;

        status::set_commit_cert_history_cap(config.commit_cert_history_cap);
        status::set_commit_inflight_timeout(config.commit_inflight_timeout);

        let vote_channel_cap = config.msg_channel_cap_votes.max(1);
        let block_payload_channel_cap = config.msg_channel_cap_block_payload.max(1);
        let rbc_chunk_channel_cap = config.msg_channel_cap_rbc_chunks.max(1);
        let block_channel_cap = config.msg_channel_cap_blocks.max(1);
        let control_msg_channel_cap = config.control_msg_channel_cap.max(1);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(block_payload_channel_cap);
        let (block_tx, block_rx) = mpsc::sync_channel(block_channel_cap);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(rbc_chunk_channel_cap);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_channel_cap);
        let (consensus_tx, consensus_rx) = mpsc::sync_channel(control_msg_channel_cap);
        let (background_tx, background_rx) = mpsc::sync_channel(control_msg_channel_cap);
        let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(control_msg_channel_cap);
        let (wake_tx, wake_rx) = mpsc::sync_channel(WORKER_WAKE_CHANNEL_CAP);
        queue.set_sumeragi_wake(wake_tx.clone());
        let vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>> = Arc::new(Mutex::new(
            DedupCache::new(VOTE_DEDUP_CACHE_CAP, VOTE_DEDUP_CACHE_TTL),
        ));
        let block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>> =
            Arc::new(Mutex::new(BlockPayloadDedupCache::new(
                BLOCK_PAYLOAD_DEDUP_CACHE_PER_KIND,
                BLOCK_PAYLOAD_DEDUP_CACHE_TTL,
            )));

        let handle = SumeragiHandle::new(
            block_payload_tx.clone(),
            block_tx.clone(),
            rbc_chunk_tx.clone(),
            vote_tx.clone(),
            consensus_tx.clone(),
            background_tx,
            lane_relay_tx,
            Arc::clone(&vote_dedup),
            Arc::clone(&block_payload_dedup),
        )
        .with_wake(wake_tx.clone());

        let rbc_status_handle = rbc_status::register_handle();
        rbc_status::set_active(&rbc_status_handle);

        let actor = SumeragiWorker {
            config,
            common_config,
            consensus_frame_cap,
            consensus_payload_frame_cap,
            events_sender,
            state,
            queue,
            kura,
            network,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            lane_relay_rx,
            rbc_store,
            background_post_tx,
            da_spool_dir,
            vote_dedup,
            block_payload_dedup,
            vote_rx,
            block_payload_rx,
            block_rx,
            rbc_chunk_rx,
            consensus_rx,
            background_rx,
            wake_tx,
            wake_rx,
            shutdown_signal,
            rbc_status_handle,
        };

        let join_handle = tokio::task::spawn(spawn_os_thread_as_future(
            std::thread::Builder::new()
                .name("sumeragi".to_owned())
                .stack_size(SUMERAGI_STACK_SIZE_BYTES),
            move || actor.run(),
        ));

        (
            handle,
            Child::new(join_handle, OnShutdown::Wait(Duration::from_secs(5))),
        )
    }
}

struct SumeragiWorker {
    config: SumeragiConfig,
    common_config: CommonConfig,
    consensus_frame_cap: usize,
    consensus_payload_frame_cap: usize,
    events_sender: EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    network: IrohaNetwork,
    peers_gossiper: PeersGossiperHandle,
    genesis_network: GenesisWithPubKey,
    block_count: BlockCount,
    block_sync_gossip_limit: usize,
    #[cfg(feature = "telemetry")]
    telemetry: Telemetry,
    epoch_roster_provider: Option<Arc<WsvEpochRosterAdapter>>,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    rbc_store: Option<RbcStoreConfig>,
    background_post_tx: Option<mpsc::SyncSender<BackgroundPost>>,
    da_spool_dir: PathBuf,
    vote_dedup: Arc<Mutex<DedupCache<VoteDedupKey>>>,
    block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>>,
    vote_rx: mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: mpsc::Receiver<InboundBlockMessage>,
    block_rx: mpsc::Receiver<InboundBlockMessage>,
    rbc_chunk_rx: mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: mpsc::Receiver<ControlFlow>,
    background_rx: mpsc::Receiver<BackgroundRequest>,
    wake_tx: mpsc::SyncSender<()>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
    rbc_status_handle: rbc_status::Handle,
}

/// Skip ticks when message queues are saturated or pending, while enforcing a periodic tick cadence.
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn should_run_tick(
    now: Instant,
    last_tick: Instant,
    vote_rx_budget_exhausted: bool,
    block_payload_rx_budget_exhausted: bool,
    rbc_chunk_rx_budget_exhausted: bool,
    block_rx_budget_exhausted: bool,
    queue_depths: status::WorkerQueueDepthSnapshot,
    min_tick_gap: Duration,
    max_tick_gap: Duration,
) -> bool {
    let queues_saturated = vote_rx_budget_exhausted
        || block_payload_rx_budget_exhausted
        || rbc_chunk_rx_budget_exhausted
        || block_rx_budget_exhausted;
    let queues_pending = has_pending_queue_depths(queue_depths);
    if !queues_saturated && !queues_pending {
        return now.saturating_duration_since(last_tick) >= min_tick_gap;
    }
    now.saturating_duration_since(last_tick) >= max_tick_gap
}

fn idle_wait_duration(
    now: Instant,
    last_tick: Instant,
    min_tick_gap: Duration,
) -> Option<Duration> {
    let elapsed = now.saturating_duration_since(last_tick);
    if elapsed >= min_tick_gap {
        None
    } else {
        Some(min_tick_gap.saturating_sub(elapsed))
    }
}

fn resolve_sumeragi_timeouts(
    on_chain_block_time: Duration,
    on_chain_commit_time: Duration,
    fallback_block_time: Duration,
    fallback_commit_time: Duration,
) -> (Duration, Duration) {
    let block_time = if on_chain_block_time == Duration::ZERO {
        fallback_block_time
    } else {
        on_chain_block_time
    };
    let commit_time = if on_chain_commit_time == Duration::ZERO {
        fallback_commit_time
    } else {
        on_chain_commit_time
    };
    (block_time, commit_time)
}

fn worker_time_budget(
    block_time: Duration,
    commit_time: Duration,
    da_enabled: bool,
    da_quorum_timeout_multiplier: u32,
    budget_cap: Duration,
) -> Duration {
    let quorum_timeout = crate::sumeragi::main_loop::commit_quorum_timeout_from_durations(
        block_time,
        commit_time,
        da_enabled,
        da_quorum_timeout_multiplier,
    );
    let scaled = quorum_timeout
        .checked_div(4)
        .unwrap_or_else(|| Duration::from_millis(1));
    let cap = Duration::from_millis(TIME_BUDGET_CAP_MS).min(budget_cap);
    let mut budget = scaled.min(cap);
    let floor = Duration::from_millis(TIME_BUDGET_FLOOR_MS);
    if floor <= cap {
        budget = budget.max(floor);
    }
    budget
}

fn vote_rx_drain_budget(
    block_time: Duration,
    commit_time: Duration,
    da_enabled: bool,
    da_quorum_timeout_multiplier: u32,
    max_budget: Duration,
    budget_cap: Duration,
) -> Duration {
    let quorum_timeout = crate::sumeragi::main_loop::commit_quorum_timeout_from_durations(
        block_time,
        commit_time,
        da_enabled,
        da_quorum_timeout_multiplier,
    );
    let budget = quorum_timeout.min(max_budget);
    // Cap vote draining so long queues don't stall RBC/DA progress in a single iteration.
    let cap = Duration::from_millis(VOTE_DRAIN_BUDGET_CAP_MS).min(budget_cap);
    cap_vote_drain_budget(budget, Duration::from_millis(1), cap)
}

const TIME_BUDGET_FLOOR_MS: u64 = 200;
const TIME_BUDGET_CAP_MS: u64 = 2_000;
const IDLE_TICK_GAP_FLOOR_MS: u64 = 50;
const IDLE_SHUTDOWN_POLL_MS: u64 = 1_000;
const DRAIN_BUDGET_CAP_MS: u64 = 500;
const VOTE_DRAIN_BUDGET_CAP_MS: u64 = 2_000;
const RBC_DRAIN_BUDGET_CAP_MS: u64 = 2_000;

fn idle_tick_gap(block_time: Duration, commit_time: Duration, max_tick_gap: Duration) -> Duration {
    let base = block_time.min(commit_time);
    let raw = if base == Duration::ZERO {
        max_tick_gap
    } else {
        base / 4
    };
    raw.max(Duration::from_millis(IDLE_TICK_GAP_FLOOR_MS))
        .min(max_tick_gap)
}

fn cap_drain_budget(raw: Duration, floor: Duration, budget_cap: Duration) -> Duration {
    let cap = Duration::from_millis(DRAIN_BUDGET_CAP_MS).min(budget_cap);
    raw.min(cap).max(floor)
}

fn cap_vote_drain_budget(raw: Duration, floor: Duration, budget_cap: Duration) -> Duration {
    raw.min(budget_cap).max(floor)
}

fn cap_rbc_drain_budget(raw: Duration, floor: Duration, budget_cap: Duration) -> Duration {
    let cap = Duration::from_millis(RBC_DRAIN_BUDGET_CAP_MS).min(budget_cap);
    raw.min(cap).max(floor)
}

trait WorkerActor {
    fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()>;
    fn on_consensus_control(&mut self, msg: ControlFlow) -> Result<()>;
    fn on_lane_relay(&mut self, message: LaneRelayMessage) -> Result<()>;
    fn on_background_request(&mut self, request: BackgroundRequest) -> Result<()>;
    fn refresh_worker_loop_config(&mut self, _cfg: &mut WorkerLoopConfig) {}
    fn poll_commit_results(&mut self) -> bool {
        false
    }
    fn poll_validation_results(&mut self) -> bool {
        false
    }
    fn poll_rbc_persist_results(&mut self) -> bool {
        false
    }
    fn should_tick(&self) -> bool {
        true
    }
    fn next_tick_deadline(&self, now: Instant) -> Option<Instant> {
        self.should_tick().then_some(now)
    }
    fn tick(&mut self) -> bool;
}

impl WorkerActor for crate::sumeragi::main_loop::Actor {
    fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
        crate::sumeragi::main_loop::Actor::on_block_message(self, msg)
    }

    fn on_consensus_control(&mut self, msg: ControlFlow) -> Result<()> {
        crate::sumeragi::main_loop::Actor::on_consensus_control(self, msg)
    }

    fn on_lane_relay(&mut self, message: LaneRelayMessage) -> Result<()> {
        crate::sumeragi::main_loop::Actor::on_lane_relay_message(self, message)
    }

    fn on_background_request(&mut self, request: BackgroundRequest) -> Result<()> {
        crate::sumeragi::main_loop::Actor::on_background_request(self, request)
    }

    fn poll_commit_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_commit_results(self)
    }

    fn poll_validation_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_validation_results(self)
    }

    fn poll_rbc_persist_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_rbc_persist_results_inner(self)
    }

    fn refresh_worker_loop_config(&mut self, cfg: &mut WorkerLoopConfig) {
        crate::sumeragi::main_loop::Actor::refresh_worker_loop_config(self, cfg)
    }

    fn should_tick(&self) -> bool {
        crate::sumeragi::main_loop::Actor::should_tick(self)
    }

    fn next_tick_deadline(&self, now: Instant) -> Option<Instant> {
        crate::sumeragi::main_loop::Actor::next_tick_deadline(self, now)
    }

    fn tick(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::tick(self)
    }
}

const PRIORITY_TIER_COUNT: usize = 7;
// Keep vote processing ahead of heavy payload tiers without starving them entirely.
const VOTE_BURST_CAP: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PriorityTier {
    Votes,
    RbcChunks,
    BlockPayload,
    Blocks,
    Consensus,
    LaneRelay,
    Background,
}

impl PriorityTier {
    const ORDER: [PriorityTier; PRIORITY_TIER_COUNT] = [
        PriorityTier::Votes,
        PriorityTier::RbcChunks,
        PriorityTier::BlockPayload,
        PriorityTier::Blocks,
        PriorityTier::Consensus,
        PriorityTier::LaneRelay,
        PriorityTier::Background,
    ];

    const fn idx(self) -> usize {
        match self {
            PriorityTier::Votes => 0,
            PriorityTier::RbcChunks => 1,
            PriorityTier::BlockPayload => 2,
            PriorityTier::Blocks => 3,
            PriorityTier::Consensus => 4,
            PriorityTier::LaneRelay => 5,
            PriorityTier::Background => 6,
        }
    }

    const fn queue_kind(self) -> status::WorkerQueueKind {
        match self {
            PriorityTier::Votes => status::WorkerQueueKind::Votes,
            PriorityTier::RbcChunks => status::WorkerQueueKind::RbcChunks,
            PriorityTier::BlockPayload => status::WorkerQueueKind::BlockPayload,
            PriorityTier::Blocks => status::WorkerQueueKind::Blocks,
            PriorityTier::Consensus => status::WorkerQueueKind::Consensus,
            PriorityTier::LaneRelay => status::WorkerQueueKind::LaneRelay,
            PriorityTier::Background => status::WorkerQueueKind::Background,
        }
    }

    const fn stage(self) -> status::WorkerLoopStage {
        match self {
            PriorityTier::Votes => status::WorkerLoopStage::DrainVotes,
            PriorityTier::RbcChunks => status::WorkerLoopStage::DrainRbcChunks,
            PriorityTier::BlockPayload => status::WorkerLoopStage::DrainBlockPayloads,
            PriorityTier::Blocks => status::WorkerLoopStage::DrainBlocks,
            PriorityTier::Consensus => status::WorkerLoopStage::DrainConsensus,
            PriorityTier::LaneRelay => status::WorkerLoopStage::DrainLaneRelay,
            PriorityTier::Background => status::WorkerLoopStage::DrainBackground,
        }
    }
}

const HIGH_PRIORITY_TIERS: [PriorityTier; 4] = [
    PriorityTier::Votes,
    PriorityTier::RbcChunks,
    PriorityTier::BlockPayload,
    PriorityTier::Blocks,
];
const LOW_PRIORITY_TIERS: [PriorityTier; 3] = [
    PriorityTier::Consensus,
    PriorityTier::LaneRelay,
    PriorityTier::Background,
];

#[derive(Debug)]
enum WorkerMessage {
    Block(InboundBlockMessage),
    Control(ControlFlow),
    LaneRelay(LaneRelayMessage),
    Background(BackgroundRequest),
}

#[derive(Debug)]
struct WorkerEnvelope {
    tier: PriorityTier,
    seq: u64,
    message: WorkerMessage,
}

#[derive(Debug)]
struct WorkerMailboxState {
    slots: Vec<Option<WorkerEnvelope>>,
    next_seq: u64,
}

impl WorkerMailboxState {
    fn new() -> Self {
        Self {
            slots: std::iter::repeat_with(|| None)
                .take(PRIORITY_TIER_COUNT)
                .collect(),
            next_seq: 0,
        }
    }
}

struct WorkerMailbox<'a> {
    state: &'a mut WorkerMailboxState,
    vote_rx: &'a mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: &'a mpsc::Receiver<InboundBlockMessage>,
    rbc_chunk_rx: &'a mpsc::Receiver<InboundBlockMessage>,
    block_rx: &'a mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: &'a mpsc::Receiver<ControlFlow>,
    lane_relay_rx: &'a mpsc::Receiver<LaneRelayMessage>,
    background_rx: &'a mpsc::Receiver<BackgroundRequest>,
}

impl<'a> WorkerMailbox<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        state: &'a mut WorkerMailboxState,
        vote_rx: &'a mpsc::Receiver<InboundBlockMessage>,
        block_payload_rx: &'a mpsc::Receiver<InboundBlockMessage>,
        rbc_chunk_rx: &'a mpsc::Receiver<InboundBlockMessage>,
        block_rx: &'a mpsc::Receiver<InboundBlockMessage>,
        consensus_rx: &'a mpsc::Receiver<ControlFlow>,
        lane_relay_rx: &'a mpsc::Receiver<LaneRelayMessage>,
        background_rx: &'a mpsc::Receiver<BackgroundRequest>,
    ) -> Self {
        Self {
            state,
            vote_rx,
            block_payload_rx,
            rbc_chunk_rx,
            block_rx,
            consensus_rx,
            lane_relay_rx,
            background_rx,
        }
    }

    fn fill_slots(&mut self, budgets: &TierBudgets) {
        for tier in PriorityTier::ORDER {
            if budgets.remaining(tier) > 0 {
                self.fill_slot(tier);
            }
        }
    }

    fn fill_slot(&mut self, tier: PriorityTier) {
        let idx = tier.idx();
        if self.state.slots[idx].is_some() {
            return;
        }
        let message = match tier {
            PriorityTier::Votes => self.vote_rx.try_recv().ok().map(WorkerMessage::Block),
            PriorityTier::RbcChunks => self.rbc_chunk_rx.try_recv().ok().map(WorkerMessage::Block),
            PriorityTier::BlockPayload => self
                .block_payload_rx
                .try_recv()
                .ok()
                .map(WorkerMessage::Block),
            PriorityTier::Blocks => self.block_rx.try_recv().ok().map(WorkerMessage::Block),
            PriorityTier::Consensus => self
                .consensus_rx
                .try_recv()
                .ok()
                .map(WorkerMessage::Control),
            PriorityTier::LaneRelay => self
                .lane_relay_rx
                .try_recv()
                .ok()
                .map(WorkerMessage::LaneRelay),
            PriorityTier::Background => self
                .background_rx
                .try_recv()
                .ok()
                .map(WorkerMessage::Background),
        };
        let Some(message) = message else {
            return;
        };
        let seq = self.state.next_seq;
        self.state.next_seq = self.state.next_seq.saturating_add(1);
        self.state.slots[idx] = Some(WorkerEnvelope { tier, seq, message });
    }

    fn take(&mut self, tier: PriorityTier) -> Option<WorkerEnvelope> {
        let idx = tier.idx();
        self.state.slots[idx].take()
    }

    fn has_pending(&self, tier: PriorityTier) -> bool {
        self.state.slots[tier.idx()].is_some()
    }

    fn any_pending(&self) -> bool {
        self.state.slots.iter().any(Option::is_some)
    }
}

#[derive(Clone, Copy, Debug)]
struct TierBudgets {
    remaining: [usize; PRIORITY_TIER_COUNT],
}

impl TierBudgets {
    fn new(cfg: &WorkerLoopConfig) -> Self {
        let remaining = [
            cfg.vote_rx_drain_max_messages.max(1),
            cfg.rbc_chunk_rx_drain_max_messages.max(1),
            cfg.block_payload_rx_drain_max_messages.max(1),
            cfg.block_rx_drain_max_messages.max(1),
            cfg.consensus_rx_drain_max_messages.max(1),
            cfg.lane_relay_rx_drain_max_messages.max(1),
            cfg.background_rx_drain_max_messages.max(1),
        ];
        Self { remaining }
    }

    fn remaining(&self, tier: PriorityTier) -> usize {
        self.remaining[tier.idx()]
    }

    fn consume(&mut self, tier: PriorityTier) {
        let slot = &mut self.remaining[tier.idx()];
        *slot = slot.saturating_sub(1);
    }
}

#[derive(Clone, Copy, Debug)]
struct WorkerLoopConfig {
    time_budget: Duration,
    vote_rx_drain_budget: Duration,
    block_payload_rx_drain_budget: Duration,
    block_payload_rx_drain_max_messages: usize,
    vote_rx_drain_max_messages: usize,
    block_rx_drain_budget: Duration,
    block_rx_drain_max_messages: usize,
    rbc_chunk_rx_drain_budget: Duration,
    rbc_chunk_rx_drain_max_messages: usize,
    consensus_rx_drain_max_messages: usize,
    lane_relay_rx_drain_max_messages: usize,
    background_rx_drain_max_messages: usize,
    tick_min_gap: Duration,
    tick_max_gap: Duration,
    block_rx_starve_max: Duration,
    non_vote_starve_max: Duration,
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
struct WorkerLoopState {
    last_tick: Instant,
    last_served: [Instant; PRIORITY_TIER_COUNT],
    mailbox: WorkerMailboxState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct WorkerIterationStats {
    block_payloads_handled: usize,
    blocks_handled: usize,
    rbc_chunks_handled: usize,
    votes_handled: usize,
    precommit_votes_handled: usize,
    last_precommit_vote: Option<(u64, u64, u64, HashOf<BlockHeader>)>,
    consensus_handled: usize,
    lane_relays_handled: usize,
    background_handled: usize,
    pre_tick_drain_ms: u64,
    tick_elapsed_ms: u64,
    post_tick_drain_ms: u64,
    queue_depths: status::WorkerQueueDepthSnapshot,
    last_envelope: Option<(PriorityTier, u64)>,
    budget_exceeded: bool,
    progress: bool,
    vote_rx_budget_exhausted: bool,
    block_payload_rx_budget_exhausted: bool,
    rbc_chunk_rx_budget_exhausted: bool,
    block_rx_budget_exhausted: bool,
}

fn has_pending_queue_depths(queue_depths: status::WorkerQueueDepthSnapshot) -> bool {
    queue_depths.vote_rx > 0
        || queue_depths.block_payload_rx > 0
        || queue_depths.rbc_chunk_rx > 0
        || queue_depths.block_rx > 0
        || queue_depths.consensus_rx > 0
        || queue_depths.lane_relay_rx > 0
        || queue_depths.background_rx > 0
}

fn should_warn_slow_iteration(stats: &WorkerIterationStats) -> bool {
    stats.progress
        || stats.budget_exceeded
        || stats.tick_elapsed_ms > 0
        || stats.vote_rx_budget_exhausted
        || stats.block_payload_rx_budget_exhausted
        || stats.rbc_chunk_rx_budget_exhausted
        || stats.block_rx_budget_exhausted
        || has_pending_queue_depths(stats.queue_depths)
}

fn drain_mailbox<A: WorkerActor>(
    actor: &mut A,
    cfg: &WorkerLoopConfig,
    iter_start: Instant,
    mailbox: &mut WorkerMailbox<'_>,
    budgets: &mut TierBudgets,
    stats: &mut WorkerIterationStats,
    last_served: &mut [Instant; PRIORITY_TIER_COUNT],
) {
    let vote_burst = cfg.vote_rx_drain_max_messages.min(VOTE_BURST_CAP).max(1);
    // Cap per-iteration draining so ticks cannot be starved by long queue backlogs.
    let drain_budget = cfg.time_budget.min(cfg.tick_max_gap);
    loop {
        if iter_start.elapsed() >= drain_budget {
            stats.budget_exceeded = true;
            break;
        }
        if !mailbox.any_pending() {
            break;
        }
        let now = Instant::now();
        let prefer_votes = stats.votes_handled < vote_burst;
        let Some(tier) = select_next_tier(now, mailbox, budgets, last_served, cfg, prefer_votes)
        else {
            break;
        };
        let Some(envelope) = mailbox.take(tier) else {
            mailbox.fill_slot(tier);
            continue;
        };

        stats.last_envelope = Some((envelope.tier, envelope.seq));
        status::set_worker_stage(tier.stage());
        match envelope.message {
            WorkerMessage::Block(msg) => {
                if let BlockMessage::QcVote(vote) = &msg.message {
                    stats.precommit_votes_handled = stats.precommit_votes_handled.saturating_add(1);
                    stats.last_precommit_vote =
                        Some((vote.height, vote.view, vote.epoch, vote.block_hash));
                    iroha_logger::debug!(
                        height = vote.height,
                        view = vote.view,
                        epoch = vote.epoch,
                        signer = vote.signer,
                        block_hash = %vote.block_hash,
                        tier = ?tier,
                        "received precommit vote"
                    );
                }
                if let Err(err) = actor.on_block_message(msg) {
                    iroha_logger::error!(?err, "Sumeragi block-message handler failed");
                }
                match tier {
                    PriorityTier::Votes => {
                        stats.votes_handled = stats.votes_handled.saturating_add(1);
                    }
                    PriorityTier::RbcChunks => {
                        stats.rbc_chunks_handled = stats.rbc_chunks_handled.saturating_add(1);
                    }
                    PriorityTier::BlockPayload => {
                        stats.block_payloads_handled =
                            stats.block_payloads_handled.saturating_add(1);
                    }
                    PriorityTier::Blocks => {
                        stats.blocks_handled = stats.blocks_handled.saturating_add(1);
                    }
                    PriorityTier::Consensus
                    | PriorityTier::LaneRelay
                    | PriorityTier::Background => {}
                }
                stats.progress = true;
            }
            WorkerMessage::Control(msg) => {
                if let Err(err) = actor.on_consensus_control(msg) {
                    iroha_logger::error!(?err, "Sumeragi consensus control handler failed");
                }
                stats.consensus_handled = stats.consensus_handled.saturating_add(1);
                stats.progress = true;
            }
            WorkerMessage::LaneRelay(message) => {
                if let Err(err) = actor.on_lane_relay(message) {
                    iroha_logger::error!(?err, "Sumeragi lane-relay handler failed");
                }
                stats.lane_relays_handled = stats.lane_relays_handled.saturating_add(1);
                stats.progress = true;
            }
            WorkerMessage::Background(request) => {
                if let Err(err) = actor.on_background_request(request) {
                    iroha_logger::error!(?err, "Sumeragi background-post handler failed");
                }
                stats.background_handled = stats.background_handled.saturating_add(1);
                stats.progress = true;
            }
        }

        status::record_worker_queue_drain(tier.queue_kind(), 1);
        budgets.consume(tier);
        last_served[tier.idx()] = now;
        mailbox.fill_slot(tier);
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_worker_iteration<A: WorkerActor>(
    actor: &mut A,
    cfg: &WorkerLoopConfig,
    loop_state: &mut WorkerLoopState,
    vote_rx: &mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: &mpsc::Receiver<InboundBlockMessage>,
    rbc_chunk_rx: &mpsc::Receiver<InboundBlockMessage>,
    block_rx: &mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: &mpsc::Receiver<ControlFlow>,
    lane_relay_rx: &mpsc::Receiver<LaneRelayMessage>,
    background_rx: &mpsc::Receiver<BackgroundRequest>,
) -> WorkerIterationStats {
    let mut stats = WorkerIterationStats {
        block_payloads_handled: 0,
        blocks_handled: 0,
        rbc_chunks_handled: 0,
        votes_handled: 0,
        precommit_votes_handled: 0,
        last_precommit_vote: None,
        consensus_handled: 0,
        lane_relays_handled: 0,
        background_handled: 0,
        pre_tick_drain_ms: 0,
        tick_elapsed_ms: 0,
        post_tick_drain_ms: 0,
        queue_depths: status::WorkerQueueDepthSnapshot::default(),
        last_envelope: None,
        budget_exceeded: false,
        progress: false,
        vote_rx_budget_exhausted: false,
        block_payload_rx_budget_exhausted: false,
        rbc_chunk_rx_budget_exhausted: false,
        block_rx_budget_exhausted: false,
    };

    let iter_start = Instant::now();
    let mut budgets = TierBudgets::new(cfg);
    let WorkerLoopState {
        last_tick,
        last_served,
        mailbox: mailbox_state,
    } = loop_state;
    let mut mailbox = WorkerMailbox::new(
        mailbox_state,
        vote_rx,
        block_payload_rx,
        rbc_chunk_rx,
        block_rx,
        consensus_rx,
        lane_relay_rx,
        background_rx,
    );
    mailbox.fill_slots(&budgets);
    let drain_start = Instant::now();
    drain_mailbox(
        actor,
        cfg,
        iter_start,
        &mut mailbox,
        &mut budgets,
        &mut stats,
        last_served,
    );
    stats.pre_tick_drain_ms = u64::try_from(drain_start.elapsed().as_millis()).unwrap_or(u64::MAX);

    if stats.precommit_votes_handled > 0 {
        iroha_logger::debug!(
            precommit_votes_handled = stats.precommit_votes_handled,
            blocks_handled = stats.blocks_handled,
            ?stats.last_precommit_vote,
            "drained precommit votes from queues"
        );
    }

    if actor.poll_commit_results() {
        stats.progress = true;
    }
    if actor.poll_validation_results() {
        stats.progress = true;
    }
    if actor.poll_rbc_persist_results() {
        stats.progress = true;
    }

    let pre_tick_depths = status::worker_queue_depth_snapshot();
    stats.vote_rx_budget_exhausted =
        budgets.remaining(PriorityTier::Votes) == 0 && pre_tick_depths.vote_rx > 0;
    stats.block_payload_rx_budget_exhausted =
        budgets.remaining(PriorityTier::BlockPayload) == 0 && pre_tick_depths.block_payload_rx > 0;
    stats.rbc_chunk_rx_budget_exhausted =
        budgets.remaining(PriorityTier::RbcChunks) == 0 && pre_tick_depths.rbc_chunk_rx > 0;
    stats.block_rx_budget_exhausted =
        budgets.remaining(PriorityTier::Blocks) == 0 && pre_tick_depths.block_rx > 0;

    let tick_now = Instant::now();
    let tick_deadline = actor.next_tick_deadline(tick_now);
    let tick_due = tick_deadline.is_some_and(|deadline| deadline <= tick_now);
    if tick_due
        && should_run_tick(
            tick_now,
            *last_tick,
            stats.vote_rx_budget_exhausted,
            stats.block_payload_rx_budget_exhausted,
            stats.rbc_chunk_rx_budget_exhausted,
            stats.block_rx_budget_exhausted,
            pre_tick_depths,
            cfg.tick_min_gap,
            cfg.tick_max_gap,
        )
    {
        status::set_worker_stage(status::WorkerLoopStage::Tick);
        let tick_start = Instant::now();
        stats.progress |= actor.tick();
        stats.tick_elapsed_ms = u64::try_from(tick_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        *last_tick = tick_now;
    }

    if !stats.budget_exceeded && iter_start.elapsed() < cfg.time_budget {
        mailbox.fill_slots(&budgets);
        let drain_start = Instant::now();
        drain_mailbox(
            actor,
            cfg,
            iter_start,
            &mut mailbox,
            &mut budgets,
            &mut stats,
            last_served,
        );
        stats.post_tick_drain_ms =
            u64::try_from(drain_start.elapsed().as_millis()).unwrap_or(u64::MAX);
    }

    let post_tick_depths = status::worker_queue_depth_snapshot();
    stats.vote_rx_budget_exhausted |=
        budgets.remaining(PriorityTier::Votes) == 0 && post_tick_depths.vote_rx > 0;
    stats.block_payload_rx_budget_exhausted |=
        budgets.remaining(PriorityTier::BlockPayload) == 0 && post_tick_depths.block_payload_rx > 0;
    stats.rbc_chunk_rx_budget_exhausted |=
        budgets.remaining(PriorityTier::RbcChunks) == 0 && post_tick_depths.rbc_chunk_rx > 0;
    stats.block_rx_budget_exhausted |=
        budgets.remaining(PriorityTier::Blocks) == 0 && post_tick_depths.block_rx > 0;

    stats.queue_depths = post_tick_depths;
    stats
}

fn tier_starve_max(cfg: &WorkerLoopConfig, tier: PriorityTier) -> Duration {
    match tier {
        PriorityTier::Votes => Duration::ZERO,
        PriorityTier::RbcChunks | PriorityTier::BlockPayload => cfg.non_vote_starve_max,
        PriorityTier::Blocks
        | PriorityTier::Consensus
        | PriorityTier::LaneRelay
        | PriorityTier::Background => cfg.block_rx_starve_max,
    }
}

fn select_oldest_pending(
    now: Instant,
    mailbox: &WorkerMailbox<'_>,
    budgets: &TierBudgets,
    last_served: &[Instant; PRIORITY_TIER_COUNT],
    tiers: &[PriorityTier],
) -> Option<PriorityTier> {
    let mut oldest: Option<(PriorityTier, Duration)> = None;
    for &tier in tiers {
        if budgets.remaining(tier) == 0 || !mailbox.has_pending(tier) {
            continue;
        }
        let elapsed = now.saturating_duration_since(last_served[tier.idx()]);
        let replace = match oldest {
            None => true,
            Some((_, best)) => elapsed > best,
        };
        if replace {
            oldest = Some((tier, elapsed));
        }
    }
    oldest.map(|(tier, _)| tier)
}

fn select_next_tier(
    now: Instant,
    mailbox: &WorkerMailbox<'_>,
    budgets: &TierBudgets,
    last_served: &[Instant; PRIORITY_TIER_COUNT],
    cfg: &WorkerLoopConfig,
    prefer_votes: bool,
) -> Option<PriorityTier> {
    let mut starved: Option<(PriorityTier, Duration)> = None;
    for tier in PriorityTier::ORDER {
        if tier == PriorityTier::Votes {
            continue;
        }
        if budgets.remaining(tier) == 0 || !mailbox.has_pending(tier) {
            continue;
        }
        let max_starve = tier_starve_max(cfg, tier);
        if max_starve == Duration::ZERO {
            continue;
        }
        let elapsed = now.saturating_duration_since(last_served[tier.idx()]);
        if elapsed >= max_starve {
            let replace = match starved {
                None => true,
                Some((_, best)) => elapsed > best,
            };
            if replace {
                starved = Some((tier, elapsed));
            }
        }
    }
    if let Some((tier, _)) = starved {
        return Some(tier);
    }
    if prefer_votes
        && budgets.remaining(PriorityTier::Votes) > 0
        && mailbox.has_pending(PriorityTier::Votes)
    {
        return Some(PriorityTier::Votes);
    }
    // After the vote burst, rotate to the oldest pending tier to keep payload/RBC moving.
    if let Some(tier) =
        select_oldest_pending(now, mailbox, budgets, last_served, &HIGH_PRIORITY_TIERS)
    {
        return Some(tier);
    }
    select_oldest_pending(now, mailbox, budgets, last_served, &LOW_PRIORITY_TIERS)
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn run_worker_loop<A: WorkerActor>(
    actor: &mut A,
    mut cfg: WorkerLoopConfig,
    mut loop_state: WorkerLoopState,
    vote_rx: mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: mpsc::Receiver<InboundBlockMessage>,
    rbc_chunk_rx: mpsc::Receiver<InboundBlockMessage>,
    block_rx: mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: mpsc::Receiver<ControlFlow>,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    background_rx: mpsc::Receiver<BackgroundRequest>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
) {
    iroha_logger::debug!(
        time_budget = ?cfg.time_budget,
        vote_rx_drain_budget = ?cfg.vote_rx_drain_budget,
        block_payload_rx_drain_budget = ?cfg.block_payload_rx_drain_budget,
        rbc_chunk_rx_drain_budget = ?cfg.rbc_chunk_rx_drain_budget,
        block_rx_drain_budget = ?cfg.block_rx_drain_budget,
        tick_min_gap = ?cfg.tick_min_gap,
        "sumeragi worker drain budgets"
    );
    loop {
        if shutdown_signal.is_sent() {
            break;
        }

        actor.refresh_worker_loop_config(&mut cfg);
        let iter_start = Instant::now();
        let stats = run_worker_iteration(
            actor,
            &cfg,
            &mut loop_state,
            &vote_rx,
            &block_payload_rx,
            &rbc_chunk_rx,
            &block_rx,
            &consensus_rx,
            &lane_relay_rx,
            &background_rx,
        );

        if !stats.progress
            && !stats.budget_exceeded
            && !stats.block_payload_rx_budget_exhausted
            && !stats.block_rx_budget_exhausted
            && !stats.vote_rx_budget_exhausted
        {
            status::set_worker_stage(status::WorkerLoopStage::Idle);
            let now = Instant::now();
            let tick_deadline = actor.next_tick_deadline(now);
            let wait = match tick_deadline {
                None => Some(Duration::from_millis(IDLE_SHUTDOWN_POLL_MS)),
                Some(deadline) => {
                    let due_wait = deadline.saturating_duration_since(now);
                    let min_gap_wait =
                        idle_wait_duration(now, loop_state.last_tick, cfg.tick_min_gap);
                    let mut wait = min_gap_wait.map_or(due_wait, |min_gap| min_gap.max(due_wait));
                    if wait.is_zero() {
                        None
                    } else {
                        let shutdown_poll = Duration::from_millis(IDLE_SHUTDOWN_POLL_MS);
                        if wait > shutdown_poll {
                            wait = shutdown_poll;
                        }
                        Some(wait)
                    }
                }
            };
            if let Some(wait) = wait {
                match wake_rx.recv_timeout(wait) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => std::thread::sleep(wait),
                }
            }
        }

        let iter_elapsed = iter_start.elapsed();
        status::record_worker_iteration(
            u64::try_from(iter_elapsed.as_millis()).unwrap_or(u64::MAX),
        );
        if iter_elapsed >= Duration::from_millis(500) && should_warn_slow_iteration(&stats) {
            iroha_logger::warn!(
                elapsed_ms = iter_elapsed.as_millis(),
                votes_handled = stats.votes_handled,
                block_payloads_handled = stats.block_payloads_handled,
                blocks_handled = stats.blocks_handled,
                rbc_chunks_handled = stats.rbc_chunks_handled,
                consensus_handled = stats.consensus_handled,
                lane_relays_handled = stats.lane_relays_handled,
                background_handled = stats.background_handled,
                pre_tick_drain_ms = stats.pre_tick_drain_ms,
                tick_elapsed_ms = stats.tick_elapsed_ms,
                post_tick_drain_ms = stats.post_tick_drain_ms,
                vote_rx_depth = stats.queue_depths.vote_rx,
                block_payload_rx_depth = stats.queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = stats.queue_depths.rbc_chunk_rx,
                block_rx_depth = stats.queue_depths.block_rx,
                consensus_rx_depth = stats.queue_depths.consensus_rx,
                lane_relay_rx_depth = stats.queue_depths.lane_relay_rx,
                background_rx_depth = stats.queue_depths.background_rx,
                progress = stats.progress,
                vote_rx_budget_exhausted = stats.vote_rx_budget_exhausted,
                block_payload_rx_budget_exhausted = stats.block_payload_rx_budget_exhausted,
                block_rx_budget_exhausted = stats.block_rx_budget_exhausted,
                rbc_chunk_rx_budget_exhausted = stats.rbc_chunk_rx_budget_exhausted,
                last_envelope = ?stats.last_envelope,
                "sumeragi worker iteration slow"
            );
        }
    }
}

#[cfg(test)]
mod worker_iteration_warn_tests {
    use super::*;

    fn empty_stats() -> WorkerIterationStats {
        WorkerIterationStats {
            block_payloads_handled: 0,
            blocks_handled: 0,
            rbc_chunks_handled: 0,
            votes_handled: 0,
            precommit_votes_handled: 0,
            last_precommit_vote: None,
            consensus_handled: 0,
            lane_relays_handled: 0,
            background_handled: 0,
            pre_tick_drain_ms: 0,
            tick_elapsed_ms: 0,
            post_tick_drain_ms: 0,
            queue_depths: status::WorkerQueueDepthSnapshot::default(),
            last_envelope: None,
            budget_exceeded: false,
            progress: false,
            vote_rx_budget_exhausted: false,
            block_payload_rx_budget_exhausted: false,
            rbc_chunk_rx_budget_exhausted: false,
            block_rx_budget_exhausted: false,
        }
    }

    #[test]
    fn slow_iteration_without_progress_or_backlog_does_not_warn() {
        let stats = empty_stats();
        assert!(!should_warn_slow_iteration(&stats));
    }

    #[test]
    fn slow_iteration_with_pending_queue_warns() {
        let mut stats = empty_stats();
        stats.queue_depths.vote_rx = 1;
        assert!(should_warn_slow_iteration(&stats));
    }
}

impl SumeragiWorker {
    #[allow(clippy::too_many_lines)]
    fn run(self) {
        let Self {
            background_post_tx,
            block_rx,
            rbc_chunk_rx,
            block_payload_rx,
            vote_rx,
            consensus_rx,
            lane_relay_rx,
            background_rx,
            wake_tx,
            wake_rx,
            shutdown_signal,
            rbc_status_handle,
            consensus_frame_cap,
            consensus_payload_frame_cap,
            config,
            common_config,
            events_sender,
            state,
            queue,
            kura,
            network,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            rbc_store,
            da_spool_dir,
            vote_dedup: _vote_dedup,
            block_payload_dedup,
        } = self;
        let fallback_block_time = config.npos.block_time;
        let fallback_commit_time = config.npos.timeouts.commit;
        let msg_channel_cap_block_payload = config.msg_channel_cap_block_payload;
        let msg_channel_cap_votes = config.msg_channel_cap_votes;
        let msg_channel_cap_blocks = config.msg_channel_cap_blocks;
        let msg_channel_cap_rbc_chunks = config.msg_channel_cap_rbc_chunks;
        let control_msg_channel_cap = config.control_msg_channel_cap;
        let worker_iteration_budget_cap = config.worker_iteration_budget_cap;
        let (block_time, commit_time, da_enabled) = {
            let view = state.view();
            let params = view.world.parameters().sumeragi();
            let mode = effective_consensus_mode(&view, config.consensus_mode);
            let (block_time, commit_time) = match mode {
                ConsensusMode::Permissioned => resolve_sumeragi_timeouts(
                    params.block_time(),
                    params.commit_time(),
                    fallback_block_time,
                    fallback_commit_time,
                ),
                ConsensusMode::Npos => {
                    let block_time = resolve_npos_block_time(&view, &config.npos);
                    let commit_time = resolve_npos_timeouts(&view, &config.npos).commit;
                    (block_time, commit_time)
                }
            };
            let da_enabled = params.da_enabled();
            (block_time, commit_time, da_enabled)
        };
        let da_quorum_timeout_multiplier = config.da_quorum_timeout_multiplier;
        let time_budget = worker_time_budget(
            block_time,
            commit_time,
            da_enabled,
            da_quorum_timeout_multiplier,
            worker_iteration_budget_cap,
        );
        let mut actor = match crate::sumeragi::main_loop::Actor::new(
            config,
            common_config,
            consensus_frame_cap,
            consensus_payload_frame_cap,
            events_sender,
            state,
            queue,
            kura,
            network,
            da_spool_dir,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            rbc_store,
            background_post_tx,
            Some(wake_tx.clone()),
            block_payload_dedup,
            rbc_status_handle,
        ) {
            Ok(actor) => Box::new(actor),
            Err(err) => {
                iroha_logger::error!(?err, "Failed to initialise Sumeragi actor");
                return;
            }
        };
        let commit_worker_join = actor.attach_commit_worker();
        let validation_worker_join = actor.attach_validation_worker();
        let rbc_persist_worker_join: Option<std::thread::JoinHandle<()>> =
            actor.attach_rbc_persist_worker();
        let vote_rx_drain_budget = vote_rx_drain_budget(
            block_time,
            commit_time,
            da_enabled,
            da_quorum_timeout_multiplier,
            Duration::from_secs(8),
            worker_iteration_budget_cap,
        )
        .max(time_budget);
        let non_vote_drain_budget =
            cap_drain_budget(block_time / 2, time_budget, worker_iteration_budget_cap);
        let rbc_chunk_drain_budget =
            cap_rbc_drain_budget(block_time / 2, time_budget, worker_iteration_budget_cap);
        let block_rx_drain_budget =
            cap_drain_budget(block_time / 4, time_budget, worker_iteration_budget_cap);
        let starve_max = block_time.max(commit_time).max(time_budget);
        let tick_min_gap = idle_tick_gap(block_time, commit_time, time_budget);
        let mut tick_max_gap = if block_time.is_zero() {
            time_budget
        } else {
            block_time.min(time_budget)
        };
        tick_max_gap = tick_max_gap.max(tick_min_gap);
        let loop_config = WorkerLoopConfig {
            time_budget,
            vote_rx_drain_budget,
            block_payload_rx_drain_budget: non_vote_drain_budget,
            block_payload_rx_drain_max_messages: msg_channel_cap_block_payload.max(1),
            vote_rx_drain_max_messages: msg_channel_cap_votes.max(1),
            block_rx_drain_budget,
            block_rx_drain_max_messages: msg_channel_cap_blocks.max(1),
            rbc_chunk_rx_drain_budget: rbc_chunk_drain_budget,
            rbc_chunk_rx_drain_max_messages: msg_channel_cap_rbc_chunks.max(1),
            consensus_rx_drain_max_messages: control_msg_channel_cap.max(1),
            lane_relay_rx_drain_max_messages: control_msg_channel_cap.max(1),
            background_rx_drain_max_messages: control_msg_channel_cap.max(1),
            tick_min_gap,
            tick_max_gap,
            block_rx_starve_max: starve_max,
            non_vote_starve_max: starve_max,
        };
        let now = Instant::now();
        let loop_state = WorkerLoopState {
            last_tick: now,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };

        status::set_worker_stage(status::WorkerLoopStage::Idle);
        run_worker_loop(
            actor.as_mut(),
            loop_config,
            loop_state,
            vote_rx,
            block_payload_rx,
            rbc_chunk_rx,
            block_rx,
            consensus_rx,
            lane_relay_rx,
            background_rx,
            wake_rx,
            shutdown_signal,
        );
        drop(actor);
        if let Err(err) = commit_worker_join.join() {
            iroha_logger::warn!(?err, "sumeragi commit worker thread exited with error");
        }
        if let Err(err) = validation_worker_join.join() {
            iroha_logger::warn!(?err, "sumeragi validation worker thread exited with error");
        }
        if let Some(join) = rbc_persist_worker_join {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi RBC persist worker thread exited with error");
            }
        }
    }
}
