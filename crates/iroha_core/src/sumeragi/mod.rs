//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
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

/// Default stack size reserved for consensus worker threads to handle deep recursion safely.
const DEFAULT_SUMERAGI_STACK_SIZE_BYTES: usize = 256 * 1024 * 1024;
/// Refuse overrides below the historical fixed size because the validation path
/// has already been observed to overflow smaller stacks in production.
const MIN_SUMERAGI_STACK_SIZE_BYTES: usize = 64 * 1024 * 1024;
const SUMERAGI_STACK_SIZE_ENV: &str = "IROHA_SUMERAGI_STACK_SIZE_BYTES";
const WORKER_WAKE_CHANNEL_CAP: usize = 1;

fn sumeragi_stack_size_bytes() -> usize {
    std::env::var(SUMERAGI_STACK_SIZE_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|bytes| *bytes >= MIN_SUMERAGI_STACK_SIZE_BYTES)
        .unwrap_or(DEFAULT_SUMERAGI_STACK_SIZE_BYTES)
}

/// Build a named Sumeragi thread with an explicit stack-size budget.
///
/// Consensus execution must not rely on platform default stack sizing because
/// deep recovery and validation paths can exceed small default thread stacks.
pub(crate) fn sumeragi_thread_builder(name: impl Into<String>) -> std::thread::Builder {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(sumeragi_stack_size_bytes())
}

#[cfg(test)]
mod thread_builder_tests {
    use std::sync::mpsc;

    use super::sumeragi_thread_builder;

    #[test]
    fn sumeragi_thread_builder_applies_requested_thread_name() {
        let (name_tx, name_rx) = mpsc::sync_channel::<String>(1);
        let join = sumeragi_thread_builder("sumeragi-thread-builder-test")
            .spawn(move || {
                let thread_name = std::thread::current()
                    .name()
                    .expect("test thread name should be set")
                    .to_owned();
                let _ = name_tx.send(thread_name);
            })
            .expect("spawn test thread");
        let observed = name_rx.recv().expect("thread name message");
        join.join().expect("join test thread");
        assert_eq!(observed, "sumeragi-thread-builder-test");
    }
}

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

    #[allow(dead_code)]
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
    load_npos_collector_config_from_world(view.world(), view.chain_id())
}

/// Attempt to load `NPoS` collector configuration (PRF seed + tunables) from the given world view.
pub fn load_npos_collector_config_from_world(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
) -> Option<NposCollectorConfig> {
    let params = world.sumeragi_npos_parameters()?;
    let seed = latest_epoch_seed_from_world(world, chain_id);
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
#[cfg(test)]
pub(crate) fn load_npos_epoch_params(
    view: &StateView<'_>,
    config: &SumeragiConfig,
) -> NposEpochParams {
    load_npos_epoch_params_from_world(view.world(), &config.npos)
}

/// Resolve VRF epoch parameters from on-chain `SumeragiNposParameters` using a world snapshot.
pub(crate) fn load_npos_epoch_params_from_world(
    world: &impl WorldReadOnly,
    fallback: &SumeragiNpos,
) -> NposEpochParams {
    world.sumeragi_npos_parameters().map_or(
        NposEpochParams {
            epoch_length_blocks: fallback.epoch_length_blocks,
            commit_deadline_offset: fallback.vrf.commit_deadline_offset_blocks,
            reveal_deadline_offset: fallback.vrf.reveal_deadline_offset_blocks,
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

/// Resolve the pacemaker block time from on-chain Sumeragi parameters.
pub(crate) fn resolve_npos_block_time(view: &StateView<'_>) -> Duration {
    resolve_npos_block_time_from_world(view.world())
}

/// Resolve the pacemaker block time from on-chain Sumeragi parameters using a world snapshot.
pub(crate) fn resolve_npos_block_time_from_world(world: &impl WorldReadOnly) -> Duration {
    let params = world.parameters().sumeragi();
    let min_finality_ms = if params.min_finality_ms() == 0 {
        iroha_data_model::parameter::system::SumeragiParameters::default().min_finality_ms()
    } else {
        params.min_finality_ms()
    };
    let base_block_time_ms = if params.block_time_ms() == 0 {
        iroha_data_model::parameter::system::SumeragiParameters::default().block_time_ms()
    } else {
        params.block_time_ms()
    }
    .max(min_finality_ms.max(1));
    let pacing_factor_bps = params.effective_pacing_factor_bps();
    let scaled_block_time_ms = u64::try_from(
        u128::from(base_block_time_ms)
            .saturating_mul(u128::from(pacing_factor_bps))
            .saturating_div(10_000),
    )
    .unwrap_or(u64::MAX);
    Duration::from_millis(scaled_block_time_ms.max(min_finality_ms.max(1)))
}

/// Resolve `NPoS` pacemaker timeouts from on-chain parameters, falling back to config values.
pub(crate) fn resolve_npos_timeouts(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> SumeragiNposTimeouts {
    resolve_npos_timeouts_from_world(view.world(), fallback)
}

/// Resolve `NPoS` pacemaker timeouts from on-chain parameters using a world snapshot.
pub(crate) fn resolve_npos_timeouts_from_world(
    world: &impl WorldReadOnly,
    fallback: &SumeragiNpos,
) -> SumeragiNposTimeouts {
    let block_time = resolve_npos_block_time_from_world(world);
    // NPoS phase timeouts are derived from block time and can legitimately be below
    // `min_finality_ms` for fast pipelines; clamping each phase to min_finality
    // artificially inflates end-to-end latency and vote fanout cadence.
    fallback.timeouts_overrides.resolve(block_time)
}

/// Resolve `NPoS` election parameters from on-chain values, falling back to config defaults.
#[cfg(test)]
pub(crate) fn resolve_npos_election_params(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> ValidatorElectionParameters {
    resolve_npos_election_params_from_world(view.world(), fallback)
}

/// Resolve `NPoS` election parameters from on-chain values using a world snapshot.
pub(crate) fn resolve_npos_election_params_from_world(
    world: &impl WorldReadOnly,
    fallback: &SumeragiNpos,
) -> ValidatorElectionParameters {
    world.sumeragi_npos_parameters().map_or(
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
#[cfg(test)]
pub(crate) fn resolve_npos_activation_lag_blocks(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> u64 {
    resolve_npos_activation_lag_blocks_from_world(view.world(), fallback)
}

/// Resolve `NPoS` activation lag for VRF penalties from on-chain parameters or config
/// using a world snapshot.
pub(crate) fn resolve_npos_activation_lag_blocks_from_world(
    world: &impl WorldReadOnly,
    fallback: &SumeragiNpos,
) -> u64 {
    world
        .sumeragi_npos_parameters()
        .map_or(fallback.reconfig.activation_lag_blocks, |params| {
            params.activation_lag_blocks()
        })
}

/// Resolve `NPoS` slashing delay (blocks) for evidence penalties from on-chain parameters or config.
#[cfg(test)]
pub(crate) fn resolve_npos_slashing_delay_blocks(
    view: &StateView<'_>,
    fallback: &SumeragiNpos,
) -> u64 {
    resolve_npos_slashing_delay_blocks_from_world(view.world(), fallback)
}

/// Resolve `NPoS` slashing delay (blocks) for evidence penalties from on-chain parameters or
/// config using a world snapshot.
pub(crate) fn resolve_npos_slashing_delay_blocks_from_world(
    world: &impl WorldReadOnly,
    fallback: &SumeragiNpos,
) -> u64 {
    world
        .sumeragi_npos_parameters()
        .map_or(fallback.reconfig.slashing_delay_blocks, |params| {
            params.slashing_delay_blocks()
        })
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
pub fn npos_seed_for_height_from_world(
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
        cell::Cell,
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU64,
        sync::{
            Arc, Barrier, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
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
            system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
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

    fn test_signed_block(height: u64, view: u64) -> SignedBlock {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            view,
        );
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None)
    }

    #[test]
    fn inbound_block_message_tracks_queue_latency() {
        let msg = BlockMessage::ConsensusParams(message::ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        });
        let inbound = InboundBlockMessage::new(msg, None);
        assert!(inbound.queue_latency_ms().is_none());

        let queued = inbound.with_enqueue_metadata(status::WorkerQueueKind::Blocks);
        let (queue, _latency_ms) = queued
            .queue_latency_ms()
            .expect("queue metadata should be present");
        assert_eq!(queue, status::WorkerQueueKind::Blocks);
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

    fn state_with_sumeragi_block_time(block_time_ms: u64) -> State {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.set_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(
                block_time_ms,
            )));
            block.commit();
        }
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn state_with_sumeragi_block_time_and_factor(
        block_time_ms: u64,
        pacing_factor_bps: u32,
    ) -> State {
        let world = World::new();
        {
            let mut block = world.block();
            let params = block.parameters.get_mut();
            params.set_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(
                block_time_ms,
            )));
            params.set_parameter(Parameter::Sumeragi(SumeragiParameter::PacingFactorBps(
                pacing_factor_bps,
            )));
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
    fn should_run_tick_when_gap_exceeds_min() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(250));
        assert!(should_run_tick(now, last_tick, Duration::from_millis(200)));
    }

    #[test]
    fn should_skip_tick_when_gap_small() {
        let now = Instant::now();
        let last_tick = backdate(now, Duration::from_millis(50));
        assert!(!should_run_tick(now, last_tick, Duration::from_millis(200)));
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
    fn busy_tick_gap_clamps_to_idle() {
        let idle_gap = Duration::from_millis(80);
        let busy_gap = busy_tick_gap(
            Duration::from_millis(1_000),
            Duration::from_millis(1_000),
            idle_gap,
        );
        assert!(busy_gap <= idle_gap);
        assert!(busy_gap >= Duration::from_millis(BUSY_TICK_GAP_FLOOR_MS));
    }

    #[test]
    fn select_next_tier_prefers_votes_when_not_starved() {
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
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

        let cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        let selected = select_next_tier(
            now,
            &mailbox,
            &budgets,
            &loop_state.last_served,
            &cfg,
            true,
            false,
        );
        assert_eq!(selected, Some(PriorityTier::Votes));
    }

    #[test]
    fn select_next_tier_preempts_votes_for_urgent_blocks() {
        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
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

        let params = message::ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        block_tx
            .send(inbound(BlockMessage::ConsensusParams(params)))
            .expect("send block message");

        let cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
        loop_state.last_served[PriorityTier::Blocks.idx()] =
            backdate(now, block_rx_urgent_gap(&cfg) + Duration::from_millis(1));

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
            true,
            false,
        );
        assert_eq!(selected, Some(PriorityTier::Blocks));
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            false,
        );
        assert_eq!(selected, Some(PriorityTier::BlockPayload));
    }

    #[test]
    fn adaptive_drain_caps_clamp_for_block_backlog() {
        let mut cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let mut depths = status::WorkerQueueDepthSnapshot::default();
        depths.block_rx = 1;

        apply_adaptive_drain_caps(&mut cfg, depths);
        assert_eq!(
            cfg.block_payload_rx_drain_max_messages,
            BLOCK_BACKLOG_PAYLOAD_RBC_MIN_CAP
        );
        assert_eq!(
            cfg.rbc_chunk_rx_drain_max_messages,
            BLOCK_BACKLOG_PAYLOAD_RBC_MIN_CAP
        );
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            false,
        );
        assert_eq!(selected, Some(PriorityTier::BlockPayload));
    }

    #[test]
    fn resolve_sumeragi_timeouts_prefers_on_chain_values() {
        let params = iroha_data_model::parameter::system::SumeragiParameters {
            block_time_ms: 900,
            commit_time_ms: 1_100,
            min_finality_ms: 100,
            ..iroha_data_model::parameter::system::SumeragiParameters::default()
        };
        let fallback = iroha_data_model::parameter::system::SumeragiParameters::default();
        let (block_time, commit_time) = resolve_sumeragi_timeouts(&params, &fallback);

        assert_eq!(block_time, Duration::from_millis(900));
        assert_eq!(commit_time, Duration::from_millis(1_100));
    }

    #[test]
    fn resolve_sumeragi_timeouts_uses_fallback_for_zero() {
        let params = iroha_data_model::parameter::system::SumeragiParameters {
            block_time_ms: 0,
            commit_time_ms: 0,
            min_finality_ms: 0,
            ..iroha_data_model::parameter::system::SumeragiParameters::default()
        };
        let fallback = iroha_data_model::parameter::system::SumeragiParameters {
            block_time_ms: 2_000,
            commit_time_ms: 4_000,
            min_finality_ms: 100,
            ..iroha_data_model::parameter::system::SumeragiParameters::default()
        };
        let (block_time, commit_time) = resolve_sumeragi_timeouts(&params, &fallback);

        assert_eq!(block_time, Duration::from_secs(2));
        assert_eq!(commit_time, Duration::from_secs(4));
    }

    #[test]
    fn resolve_sumeragi_timeouts_clamps_to_min_finality() {
        let params = iroha_data_model::parameter::system::SumeragiParameters {
            block_time_ms: 50,
            commit_time_ms: 60,
            min_finality_ms: 100,
            ..iroha_data_model::parameter::system::SumeragiParameters::default()
        };
        let fallback = iroha_data_model::parameter::system::SumeragiParameters::default();
        let (block_time, commit_time) = resolve_sumeragi_timeouts(&params, &fallback);
        assert_eq!(block_time, Duration::from_millis(100));
        assert_eq!(commit_time, Duration::from_millis(100));
    }

    #[test]
    fn resolve_sumeragi_timeouts_applies_pacing_factor() {
        let params = iroha_data_model::parameter::system::SumeragiParameters {
            block_time_ms: 1_000,
            commit_time_ms: 1_500,
            min_finality_ms: 100,
            pacing_factor_bps: 12_500,
            ..iroha_data_model::parameter::system::SumeragiParameters::default()
        };
        let fallback = iroha_data_model::parameter::system::SumeragiParameters::default();
        let (block_time, commit_time) = resolve_sumeragi_timeouts(&params, &fallback);
        assert_eq!(block_time, Duration::from_millis(1_250));
        assert_eq!(commit_time, Duration::from_millis(1_875));
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
    fn worker_time_budget_clamps_to_floor_for_small_block_window() {
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
    fn worker_time_budget_clamps_to_cap_for_large_block_window() {
        let budget = worker_time_budget(
            Duration::from_secs(20),
            Duration::from_secs(30),
            true,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(TIME_BUDGET_CAP_MS));
    }

    #[test]
    fn worker_time_budget_uses_block_commit_window() {
        let budget = worker_time_budget(
            Duration::from_secs(2),
            Duration::from_secs(3),
            true,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(budget, Duration::from_millis(500));
    }

    #[test]
    fn worker_time_budget_ignores_da_quorum_multiplier() {
        let baseline = worker_time_budget(
            Duration::from_secs(2),
            Duration::from_secs(2),
            true,
            1,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        let with_large_multiplier = worker_time_budget(
            Duration::from_secs(2),
            Duration::from_secs(2),
            true,
            10,
            Duration::from_millis(TIME_BUDGET_CAP_MS),
        );
        assert_eq!(baseline, with_large_multiplier);
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
    fn idle_tick_gap_ignores_tick_work_budget_cap() {
        let gap = idle_tick_gap(
            Duration::from_millis(400),
            Duration::from_millis(400),
            Duration::from_millis(400),
        );
        assert_eq!(gap, Duration::from_millis(100));
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
    fn load_npos_collector_config_from_world_matches_view_loader() {
        let world = World::new();
        {
            let mut block = world.block();
            let params = SumeragiNposParameters::default().with_epoch_seed([0xEF; 32]);
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
        let from_view = load_npos_collector_config(&view).expect("view loader should resolve");
        let world_view = state.world_view();
        let from_world = load_npos_collector_config_from_world(&world_view, state.chain_id_ref())
            .expect("world loader should resolve");
        assert_eq!(from_world, from_view);
    }

    #[test]
    fn load_npos_epoch_params_from_world_matches_view_loader() {
        let state = state_with_npos_params(SumeragiNposParameters {
            epoch_length_blocks: 77,
            vrf_commit_window_blocks: 5,
            vrf_reveal_window_blocks: 7,
            ..SumeragiNposParameters::default()
        });
        let fallback = SumeragiNpos::default();
        let world_view = state.world_view();
        let from_world = load_npos_epoch_params_from_world(&world_view, &fallback);
        assert_eq!(
            from_world,
            NposEpochParams {
                epoch_length_blocks: 77,
                commit_deadline_offset: 5,
                reveal_deadline_offset: 12,
            }
        );
    }

    #[test]
    fn resolve_npos_block_time_uses_on_chain_or_default() {
        let state = state_with_sumeragi_block_time(1_500);
        let view = state.view();
        assert_eq!(resolve_npos_block_time(&view), Duration::from_millis(1_500));
        drop(view);

        let state = state_with_sumeragi_block_time(0);
        let view = state.view();
        let default_block_time = Duration::from_millis(
            iroha_data_model::parameter::system::SumeragiParameters::default().block_time_ms(),
        );
        assert_eq!(resolve_npos_block_time(&view), default_block_time);
    }

    #[test]
    fn resolve_npos_block_time_applies_pacing_factor() {
        let state = state_with_sumeragi_block_time_and_factor(1_000, 12_000);
        let view = state.view();
        assert_eq!(resolve_npos_block_time(&view), Duration::from_millis(1_200));
    }

    #[test]
    fn resolve_npos_timeouts_apply_overrides_and_ignore_on_chain_timeouts() {
        let mut fallback = SumeragiNpos::default();
        fallback.timeouts_overrides.propose = Some(Duration::from_millis(150));
        fallback.timeouts_overrides.prevote = Some(Duration::from_millis(160));
        fallback.timeouts_overrides.precommit = Some(Duration::from_millis(170));
        fallback.timeouts_overrides.commit = Some(Duration::from_millis(180));
        fallback.timeouts_overrides.da = Some(Duration::from_millis(190));
        fallback.timeouts_overrides.aggregator = Some(Duration::from_millis(200));

        let state = state_with_npos_params(SumeragiNposParameters {
            epoch_seed: [0xAA; 32],
            ..SumeragiNposParameters::default()
        });
        let view = state.view();
        let resolved = resolve_npos_timeouts(&view, &fallback);
        let derived = SumeragiNposTimeouts::from_block_time(resolve_npos_block_time(&view));
        assert_eq!(resolved.propose, Duration::from_millis(150));
        assert_eq!(resolved.prevote, Duration::from_millis(160));
        assert_eq!(resolved.precommit, Duration::from_millis(170));
        assert_eq!(resolved.commit, Duration::from_millis(180));
        assert_eq!(resolved.da, Duration::from_millis(190));
        assert_eq!(resolved.aggregator, Duration::from_millis(200));
        assert_eq!(resolved.exec, derived.exec);
        assert_eq!(resolved.witness, derived.witness);
    }

    #[test]
    fn resolve_npos_timeouts_scale_with_fast_block_time_without_min_finality_clamp() {
        let fallback = SumeragiNpos::default();
        let state = state_with_sumeragi_block_time(333);
        let view = state.view();
        let resolved = resolve_npos_timeouts(&view, &fallback);
        let expected = SumeragiNposTimeouts::from_block_time(Duration::from_millis(333));
        assert_eq!(resolved.propose, expected.propose);
        assert_eq!(resolved.prevote, expected.prevote);
        assert_eq!(resolved.precommit, expected.precommit);
        assert_eq!(resolved.commit, expected.commit);
        assert_eq!(resolved.da, expected.da);
        assert_eq!(resolved.aggregator, expected.aggregator);
        assert_eq!(resolved.exec, expected.exec);
        assert_eq!(resolved.witness, expected.witness);
        assert!(resolved.aggregator < Duration::from_millis(100));
    }

    #[test]
    fn resolve_npos_timeouts_preserve_small_overrides() {
        let mut fallback = SumeragiNpos::default();
        fallback.timeouts_overrides.propose = Some(Duration::from_millis(30));
        fallback.timeouts_overrides.prevote = Some(Duration::from_millis(40));
        fallback.timeouts_overrides.precommit = Some(Duration::from_millis(50));
        fallback.timeouts_overrides.commit = Some(Duration::from_millis(60));
        fallback.timeouts_overrides.da = Some(Duration::from_millis(70));
        fallback.timeouts_overrides.aggregator = Some(Duration::from_millis(5));

        let state = state_with_sumeragi_block_time(333);
        let view = state.view();
        let resolved = resolve_npos_timeouts(&view, &fallback);
        assert_eq!(resolved.propose, Duration::from_millis(30));
        assert_eq!(resolved.prevote, Duration::from_millis(40));
        assert_eq!(resolved.precommit, Duration::from_millis(50));
        assert_eq!(resolved.commit, Duration::from_millis(60));
        assert_eq!(resolved.da, Duration::from_millis(70));
        assert_eq!(resolved.aggregator, Duration::from_millis(5));
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
    fn resolve_npos_election_params_from_world_matches_view_loader() {
        let state = state_with_npos_params(SumeragiNposParameters {
            max_validators: 21,
            min_self_bond: 22,
            min_nomination_bond: 23,
            max_nominator_concentration_pct: 24,
            seat_band_pct: 25,
            max_entity_correlation_pct: 26,
            finality_margin_blocks: 27,
            ..SumeragiNposParameters::default()
        });
        let fallback = SumeragiNpos::default();
        let view = state.view();
        let from_view = resolve_npos_election_params(&view, &fallback);
        let world_view = state.world_view();
        let from_world = resolve_npos_election_params_from_world(&world_view, &fallback);
        assert_eq!(from_world, from_view);
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
    fn resolve_npos_activation_lag_blocks_from_world_matches_view_loader() {
        let mut fallback = SumeragiNpos::default();
        fallback.reconfig.activation_lag_blocks = 18;
        let state = state_with_npos_params(SumeragiNposParameters {
            activation_lag_blocks: 9,
            ..SumeragiNposParameters::default()
        });

        let view = state.view();
        let from_view = resolve_npos_activation_lag_blocks(&view, &fallback);
        drop(view);

        let world_view = state.world_view();
        let from_world = resolve_npos_activation_lag_blocks_from_world(&world_view, &fallback);
        assert_eq!(from_world, from_view);
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
    fn resolve_npos_slashing_delay_blocks_from_world_matches_view_loader() {
        let mut fallback = SumeragiNpos::default();
        fallback.reconfig.slashing_delay_blocks = 21;
        let state = state_with_npos_params(SumeragiNposParameters {
            slashing_delay_blocks: 11,
            ..SumeragiNposParameters::default()
        });

        let view = state.view();
        let from_view = resolve_npos_slashing_delay_blocks(&view, &fallback);
        drop(view);

        let world_view = state.world_view();
        let from_world = resolve_npos_slashing_delay_blocks_from_world(&world_view, &fallback);
        assert_eq!(from_world, from_view);
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
            prev_roster_evidence_hash: None,
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

        let received: Vec<_> = rbc_chunk_rx.try_iter().collect();
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
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
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

        let received: Vec<_> = rbc_chunk_rx.try_iter().collect();
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
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
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

        let received: Vec<_> = rbc_chunk_rx.try_iter().collect();
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
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_block_created_via_rbc_ingress_queue() {
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

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);

        handle.incoming_block_message(BlockMessage::BlockCreated(message::BlockCreated {
            block,
            frontier: None,
        }));

        let received = rbc_chunk_rx
            .try_recv()
            .expect("BlockCreated should be enqueued to the RBC ingress channel");
        let (queue_kind, _latency_ms) = received
            .queue_latency_ms()
            .expect("BlockCreated should record enqueue metadata");
        assert_eq!(queue_kind, status::WorkerQueueKind::RbcChunks);
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
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_block_created() {
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
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

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let msg = BlockMessage::BlockCreated(message::BlockCreated {
            block,
            frontier: None,
        });

        handle.incoming_block_message(msg.clone());
        handle.incoming_block_message(msg);

        let received: Vec<_> = rbc_chunk_rx.try_iter().collect();
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received.first(),
            Some(InboundBlockMessage {
                message: BlockMessage::BlockCreated(_),
                ..
            })
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
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
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
        let (queue_kind, _latency_ms) = received
            .queue_latency_ms()
            .expect("BlockSyncUpdate should record enqueue metadata");
        assert_eq!(queue_kind, status::WorkerQueueKind::BlockPayload);
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
    fn incoming_block_message_routes_fetch_pending_block_via_block_queue() {
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

        let requester = PeerId::new(KeyPair::random().public_key().clone());
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
        let request = message::FetchPendingBlock {
            requester,
            block_hash,
            height: 0,
            view: 0,
            priority: None,
            requester_roster_proof_known: None,
        };
        handle.incoming_block_message(BlockMessage::FetchPendingBlock(request.clone()));

        let received = block_rx
            .try_recv()
            .expect("FetchPendingBlock should be enqueued to block channel");
        let (queue_kind, _latency_ms) = received
            .queue_latency_ms()
            .expect("FetchPendingBlock should record enqueue metadata");
        assert_eq!(queue_kind, status::WorkerQueueKind::Blocks);
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::FetchPendingBlock(_),
                ..
            }
        ));
        handle.incoming_block_message(BlockMessage::FetchPendingBlock(request));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
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
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
    fn incoming_block_message_waits_when_rbc_ingress_queue_full_for_block_created() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(CAP);
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

        let header_one = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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

        rbc_chunk_tx_fill
            .send(inbound(BlockMessage::BlockCreated(message::BlockCreated {
                block: block_one,
                frontier: None,
            })))
            .expect("fill RBC ingress channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.incoming_block_message(BlockMessage::BlockCreated(
                message::BlockCreated {
                    block: block_two,
                    frontier: None,
                },
            ));
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "BlockCreated should wait for RBC ingress queue capacity"
        );
        let _ = rbc_chunk_rx
            .recv()
            .expect("drain RBC ingress queue to unblock sender");
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("BlockCreated should be enqueued after space is available");
        join.join().expect("join BlockCreated sender");

        let received = rbc_chunk_rx
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
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn try_incoming_block_message_waits_when_rbc_ingress_queue_full_for_block_created() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(CAP);
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

        let header_one = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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

        rbc_chunk_tx_fill
            .send(inbound(BlockMessage::BlockCreated(message::BlockCreated {
                block: block_one,
                frontier: None,
            })))
            .expect("fill RBC ingress channel");
        let (done_tx, done_rx) = mpsc::channel();
        let handle_clone = handle.clone();
        let join = std::thread::spawn(move || {
            handle_clone.try_incoming_block_message(BlockMessage::BlockCreated(
                message::BlockCreated {
                    block: block_two,
                    frontier: None,
                },
            ));
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "BlockCreated should wait for RBC ingress queue capacity"
        );
        let _ = rbc_chunk_rx
            .recv()
            .expect("drain RBC ingress queue to unblock sender");
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("BlockCreated should be enqueued after space is available");
        join.join().expect("join BlockCreated sender");

        let received = rbc_chunk_rx
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
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_drops_duplicate_rbc_init() {
        const CAP: usize = 4;
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

        assert!(handle.try_incoming_block_message(init.clone()));
        assert!(!handle.try_incoming_block_message(init));

        let received = rbc_chunk_rx.try_recv().expect("RbcInit should be enqueued");
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
    fn try_incoming_block_message_waits_when_rbc_ingress_queue_full_for_rbc_ready() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(CAP);
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

        let chunk_fill = BlockMessage::RbcChunk(crate::sumeragi::consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0x55],
        });
        rbc_chunk_tx_fill
            .send(inbound(chunk_fill))
            .expect("fill RBC ingress channel");

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
            "RbcReady should wait for RBC ingress queue capacity"
        );
        let _ = rbc_chunk_rx
            .recv()
            .expect("drain RBC ingress queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RbcReady should be enqueued after space is available");
        assert!(
            accepted,
            "RbcReady should be accepted after space is available"
        );
        join.join().expect("join RbcReady sender");

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcReady should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcReady(_),
                ..
            }
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_rbc_chunk_queue_full_for_rbc_init() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(CAP);
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

        let chunk_fill = BlockMessage::RbcChunk(crate::sumeragi::consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9u8; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0x42],
        });
        rbc_chunk_tx_fill
            .send(inbound(chunk_fill))
            .expect("fill RBC chunk channel");

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
            .expect("drain RBC chunk queue to unblock sender");
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
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn try_incoming_block_message_waits_when_rbc_ingress_queue_full_for_rbc_deliver() {
        const CAP: usize = 1;
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(CAP);
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

        let chunk_fill = BlockMessage::RbcChunk(crate::sumeragi::consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x10; 32])),
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0x66],
        });
        rbc_chunk_tx_fill
            .send(inbound(chunk_fill))
            .expect("fill RBC ingress channel");

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
            "RbcDeliver should wait for RBC ingress queue capacity"
        );
        let _ = rbc_chunk_rx
            .recv()
            .expect("drain RBC ingress queue to unblock sender");
        let accepted = done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RbcDeliver should be enqueued after space is available");
        assert!(
            accepted,
            "RbcDeliver should be accepted after space is available"
        );
        join.join().expect("join RbcDeliver sender");

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcDeliver should be enqueued after space is freed");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcDeliver(_),
                ..
            }
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
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
    fn incoming_block_message_waits_when_rbc_chunk_queue_full() {
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

        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "blocking RBC chunk ingress should wait for queue capacity"
        );
        let received = rbc_chunk_rx
            .recv()
            .expect("drain RBC chunk queue to unblock sender");
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcChunk(_),
                ..
            }
        ));
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("RBC chunk enqueue should complete after capacity is freed");
        join.join().expect("join RBC chunk sender");
        let received = rbc_chunk_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("queued RBC chunk should be delivered after the wait");
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
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_rbc_ready_via_rbc_ingress_queue() {
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

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcReady should be enqueued to the RBC ingress channel");
        let (queue_kind, _latency_ms) = received
            .queue_latency_ms()
            .expect("RbcReady should record enqueue metadata");
        assert_eq!(queue_kind, status::WorkerQueueKind::RbcChunks);
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcReady(_),
                ..
            }
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn incoming_block_message_routes_rbc_deliver_via_rbc_ingress_queue() {
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
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([6u8; 32])),
            height: 2,
            view: 0,
            epoch: 0,
            roster_hash: Hash::prehashed([0x33; 32]),
            chunk_root: Hash::prehashed([0x44; 32]),
            sender: 0,
            signature: Vec::new(),
            ready_signatures: Vec::new(),
        });

        handle.incoming_block_message(msg);

        let received = rbc_chunk_rx
            .try_recv()
            .expect("RbcDeliver should be enqueued to the RBC ingress channel");
        let (queue_kind, _latency_ms) = received
            .queue_latency_ms()
            .expect("RbcDeliver should record enqueue metadata");
        assert_eq!(queue_kind, status::WorkerQueueKind::RbcChunks);
        assert!(matches!(
            received,
            InboundBlockMessage {
                message: BlockMessage::RbcDeliver(_),
                ..
            }
        ));
        assert!(matches!(vote_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            block_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(matches!(
            block_payload_rx.try_recv(),
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
    fn incoming_block_message_routes_rbc_init_via_rbc_chunk_queue() {
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
        assert!(matches!(
            block_payload_rx.try_recv(),
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
                BlockMessage::BlockCreated(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcChunk(_)
                | BlockMessage::RbcChunkCompact(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_) => "rbc",
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
                BlockMessage::BlockCreated(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcChunk(_)
                | BlockMessage::RbcChunkCompact(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_) => "rbc",
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
                BlockMessage::BlockCreated(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcChunk(_)
                | BlockMessage::RbcChunkCompact(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_) => {
                    self.events.push("rbc");
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

    #[derive(Default)]
    struct QueuePriorityRecordingActor {
        events: Vec<&'static str>,
    }

    impl WorkerActor for QueuePriorityRecordingActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::BlockCreated(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcChunk(_)
                | BlockMessage::RbcChunkCompact(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_) => self.events.push("rbc"),
                BlockMessage::ConsensusParams(_) | BlockMessage::FetchPendingBlock(_) => {
                    self.events.push("block");
                }
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

    #[derive(Default)]
    struct BatchRecordingActor {
        events: Vec<u8>,
    }

    impl WorkerActor for BatchRecordingActor {
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

        fn tick(&mut self) -> bool {
            false
        }
    }

    struct SlowVoteTickActor {
        events: Vec<&'static str>,
        vote_sleep: Duration,
        tick_delay: Duration,
        tick_deadline: Cell<Option<Instant>>,
        tick_calls: usize,
    }

    impl WorkerActor for SlowVoteTickActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::QcVote(_) => {
                    std::thread::sleep(self.vote_sleep);
                    self.events.push("vote");
                }
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

        fn next_tick_deadline(&self, now: Instant) -> Option<Instant> {
            match self.tick_deadline.get() {
                Some(deadline) => Some(deadline),
                None => {
                    let deadline = now + self.tick_delay;
                    self.tick_deadline.set(Some(deadline));
                    Some(deadline)
                }
            }
        }

        fn tick(&mut self) -> bool {
            self.tick_calls = self.tick_calls.saturating_add(1);
            self.events.push("tick");
            true
        }
    }

    struct SlowDrainActor {
        vote_sleep: Duration,
        payload_sleep: Duration,
    }

    impl WorkerActor for SlowDrainActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::QcVote(_) => std::thread::sleep(self.vote_sleep),
                BlockMessage::Proposal(_) => std::thread::sleep(self.payload_sleep),
                _ => {}
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

    struct TickEnqueueActorWithDelays {
        events: Vec<&'static str>,
        block_payload_tx: mpsc::SyncSender<InboundBlockMessage>,
        block_tx: mpsc::SyncSender<InboundBlockMessage>,
        block_created: message::BlockCreated,
        payload_sleep: Duration,
        block_sleep: Duration,
    }

    impl WorkerActor for TickEnqueueActorWithDelays {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::Proposal(_) => {
                    std::thread::sleep(self.payload_sleep);
                    self.events.push("payload");
                }
                BlockMessage::BlockCreated(_) => {
                    std::thread::sleep(self.block_sleep);
                    self.events.push("block");
                }
                _ => {}
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

            self.block_tx
                .send(InboundBlockMessage::new(
                    BlockMessage::BlockCreated(self.block_created.clone()),
                    None,
                ))
                .expect("send block created");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);
            true
        }
    }

    struct PreTickDrainActor {
        vote_sleep: Duration,
        payload_sleep: Duration,
        block_sleep: Duration,
    }

    impl WorkerActor for PreTickDrainActor {
        fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()> {
            match msg.message {
                BlockMessage::QcVote(_) => std::thread::sleep(self.vote_sleep),
                BlockMessage::Proposal(_) => std::thread::sleep(self.payload_sleep),
                BlockMessage::BlockCreated(_) => std::thread::sleep(self.block_sleep),
                _ => {}
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

        fn should_tick(&self) -> bool {
            false
        }

        fn tick(&mut self) -> bool {
            false
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        assert_eq!(actor.events, vec!["vote", "rbc", "block", "payload"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(stats.blocks_handled, 1);
        assert!(stats.progress);
        assert!(!stats.budget_exceeded);
        assert_eq!(actor.tick_calls, 1);
    }

    #[test]
    fn run_worker_iteration_adapts_payload_drain_caps_on_vote_backlog() {
        let _guard = status::worker_queue_test_guard();
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
        for _ in 0..8 {
            block_payload_tx
                .send(inbound(BlockMessage::Proposal(proposal.clone())))
                .expect("send proposal");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::BlockPayload);
        }

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        let expected_cap = (config.block_payload_rx_drain_max_messages
            / BLOCK_PAYLOAD_DRAIN_BACKLOG_DIVISOR)
            .max(BLOCK_PAYLOAD_DRAIN_BACKLOG_MIN)
            .min(config.block_payload_rx_drain_max_messages);
        assert_eq!(stats.block_payloads_handled, expected_cap);
    }

    #[test]
    fn run_worker_iteration_adapts_block_drain_caps_on_block_backlog() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        for _ in 0..20 {
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
        }

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        let expected_cap = config
            .block_rx_drain_max_messages
            .min(block_backlog_drain_cap(20));
        assert_eq!(stats.blocks_handled, expected_cap);
    }

    #[test]
    fn block_backlog_drain_cap_uses_depth_tiers() {
        assert_eq!(block_backlog_drain_cap(0), 0);
        assert_eq!(block_backlog_drain_cap(1), BLOCK_RX_BACKLOG_DRAIN_CAP_SMALL);
        assert_eq!(block_backlog_drain_cap(3), BLOCK_RX_BACKLOG_DRAIN_CAP_SMALL);
        assert_eq!(
            block_backlog_drain_cap(4),
            BLOCK_RX_BACKLOG_DRAIN_CAP_MEDIUM
        );
        assert_eq!(
            block_backlog_drain_cap(15),
            BLOCK_RX_BACKLOG_DRAIN_CAP_MEDIUM
        );
        assert_eq!(
            block_backlog_drain_cap(16),
            BLOCK_RX_BACKLOG_DRAIN_CAP_LARGE
        );
        assert_eq!(
            block_backlog_drain_cap(63),
            BLOCK_RX_BACKLOG_DRAIN_CAP_LARGE
        );
        assert_eq!(block_backlog_drain_cap(64), BLOCK_RX_BACKLOG_DRAIN_CAP_HUGE);
    }

    #[test]
    fn apply_adaptive_drain_caps_scales_payload_and_rbc_from_block_backlog() {
        let mut cfg = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 32,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 32,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 32,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let depths = status::WorkerQueueDepthSnapshot {
            block_rx: 20,
            ..status::WorkerQueueDepthSnapshot::default()
        };
        apply_adaptive_drain_caps(&mut cfg, depths);

        assert_eq!(cfg.block_rx_drain_max_messages, 16);
        assert_eq!(cfg.block_payload_rx_drain_max_messages, 12);
        assert_eq!(cfg.rbc_chunk_rx_drain_max_messages, 12);
    }

    #[test]
    fn run_worker_iteration_limits_vote_burst_when_blocks_pending() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let vote_total = VOTE_BURST_CAP_WITH_BLOCKS + 1;
        let vote_cap = vote_total.saturating_add(1);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_cap);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..vote_total {
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
        }

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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_secs(1),
            tick_busy_gap: Duration::from_secs(1),
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

        let block_index = actor
            .events
            .iter()
            .position(|entry| *entry == "block")
            .expect("block should be drained");
        assert_eq!(block_index, VOTE_BURST_CAP_WITH_BLOCKS);
        assert!(
            actor.events[..block_index]
                .iter()
                .all(|entry| *entry == "vote")
        );
        assert_eq!(actor.events.len(), vote_total + 1);
        assert_eq!(stats.votes_handled, vote_total);
        assert_eq!(stats.blocks_handled, 1);
        assert_eq!(actor.tick_calls, 0);
    }

    #[test]
    fn run_worker_iteration_rotates_to_payload_after_vote_burst() {
        status::reset_worker_loop_snapshot_for_tests();

        let vote_total = VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG + 1;
        let vote_cap = vote_total.saturating_add(1);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_cap);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..vote_total {
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
        }

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
            time_budget: Duration::from_secs(2),
            drain_budget_cap: Duration::from_secs(2),
            vote_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: vote_total,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(2),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(2),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_secs(1),
            tick_busy_gap: Duration::from_secs(1),
            tick_max_gap: Duration::from_secs(2),
            block_rx_starve_max: Duration::from_secs(2),
            non_vote_starve_max: Duration::from_secs(2),
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

        let payload_index = actor
            .events
            .iter()
            .position(|entry| *entry == "payload")
            .expect("payload should be drained");
        assert_eq!(payload_index, VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG);
        assert!(
            actor.events[..payload_index]
                .iter()
                .all(|entry| *entry == "vote")
        );
        assert_eq!(actor.events.len(), vote_total + 1);
        assert_eq!(stats.votes_handled, vote_total);
        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(actor.tick_calls, 0);
    }

    #[test]
    fn run_worker_iteration_rotates_to_rbc_after_vote_burst() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let vote_total = VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG + 1;
        let vote_cap = vote_total.saturating_add(1);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_cap);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..vote_total {
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
            time_budget: Duration::from_secs(2),
            drain_budget_cap: Duration::from_secs(2),
            vote_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: vote_total,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(2),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(2),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(2),
            block_rx_starve_max: Duration::from_secs(2),
            non_vote_starve_max: Duration::from_secs(2),
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

        let rbc_index = actor
            .events
            .iter()
            .position(|entry| *entry == "rbc")
            .expect("rbc chunk should be drained");
        assert_eq!(rbc_index, VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG);
        assert!(
            actor.events[..rbc_index]
                .iter()
                .all(|entry| *entry == "vote")
        );
        assert_eq!(actor.events.len(), vote_total + 1);
        assert_eq!(stats.votes_handled, vote_total);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert_eq!(actor.tick_calls, 0);
    }

    #[test]
    fn run_worker_iteration_treats_block_created_as_rbc_payload_after_vote_burst() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let vote_total = VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG + 1;
        let vote_cap = vote_total.saturating_add(1);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_cap);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..vote_total {
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
        }

        let block_created = message::BlockCreated {
            block: test_signed_block(1, 0),
            frontier: None,
        };
        rbc_chunk_tx
            .send(inbound(BlockMessage::BlockCreated(block_created)))
            .expect("send BlockCreated on unified RBC ingress lane");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::RbcChunks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(2),
            drain_budget_cap: Duration::from_secs(2),
            vote_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_budget: Duration::from_secs(2),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: vote_total,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(2),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(2),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(2),
            block_rx_starve_max: Duration::from_secs(2),
            non_vote_starve_max: Duration::from_secs(2),
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

        let rbc_index = actor
            .events
            .iter()
            .position(|entry| *entry == "rbc")
            .expect("BlockCreated on the RBC ingress lane should be drained");
        assert_eq!(rbc_index, VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG);
        assert!(
            actor.events[..rbc_index]
                .iter()
                .all(|entry| *entry == "vote")
        );
        assert_eq!(actor.events.len(), vote_total + 1);
        assert_eq!(stats.votes_handled, vote_total);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert_eq!(actor.tick_calls, 0);
    }

    #[test]
    fn run_worker_iteration_tracks_drain_times_for_votes_and_payloads() {
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let past = backdate(now, Duration::from_secs(1));
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [now; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = SlowDrainActor {
            vote_sleep: Duration::from_millis(5),
            payload_sleep: Duration::from_millis(7),
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

        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert!(stats.vote_drain_ms > 0);
        assert!(stats.block_payload_drain_ms > 0);
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        assert_eq!(actor.events, vec!["rbc", "block", "payload", "vote"]);
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

        let _stats = run_worker_iteration(
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
            vec!["vote", "rbc", "block", "payload", "tick"]
        );
        assert_eq!(actor.tick_calls, 1);
    }

    #[test]
    fn run_worker_iteration_ticks_when_backlogged_before_max_gap() {
        status::reset_worker_loop_snapshot_for_tests();

        let vote_total = VOTE_BURST_CAP_WITH_BLOCKS.saturating_add(2);
        let vote_cap = vote_total.saturating_add(1);
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_cap);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..vote_total {
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
        }

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 1,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(100),
            tick_busy_gap: Duration::from_millis(100),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let now = Instant::now();
        let past = backdate(now, Duration::from_millis(200));
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = RecordingActorWithTick::default();

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
        assert!(stats.vote_rx_budget_exhausted);
        assert!(actor.events.contains(&"tick"));
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            .send(inbound(BlockMessage::QcVote(vote.clone())))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);
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
            drain_budget_cap: Duration::from_millis(5),
            vote_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_millis(50),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_millis(50),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
    fn run_worker_iteration_caps_total_drain_budget_grants_rbc_overtime_turn() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
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
            .send(inbound(BlockMessage::QcVote(vote.clone())))
            .expect("send prevote");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Votes);
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

        let config = WorkerLoopConfig {
            time_budget: Duration::from_millis(5),
            drain_budget_cap: Duration::from_millis(5),
            vote_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_budget: Duration::from_millis(50),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_millis(50),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_millis(50),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_millis(50),
            block_rx_starve_max: Duration::from_millis(50),
            non_vote_starve_max: Duration::from_millis(50),
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

        assert_eq!(actor.events, vec!["vote", "rbc"]);
        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.rbc_chunks_handled, 1);
        assert!(stats.budget_exceeded);
        assert!(matches!(
            rbc_chunk_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn run_worker_iteration_caps_drain_at_config_cap() {
        let _guard = status::worker_queue_test_guard();
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
            time_budget: Duration::from_millis(100),
            drain_budget_cap: Duration::from_millis(10),
            vote_rx_drain_budget: Duration::from_millis(100),
            block_payload_rx_drain_budget: Duration::from_millis(100),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_millis(100),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_millis(100),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_millis(100),
            block_rx_starve_max: Duration::ZERO,
            non_vote_starve_max: Duration::ZERO,
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

        assert!(!actor.events.is_empty());
        assert!(actor.events.iter().all(|event| *event == "vote"));
        assert!(stats.votes_handled >= 1);
        assert_eq!(stats.block_payloads_handled, 0);
        assert!(stats.budget_exceeded);
    }

    #[test]
    fn run_worker_iteration_caps_drain_at_tick_gap() {
        let _guard = status::worker_queue_test_guard();
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
    fn run_worker_iteration_preempts_drain_for_tick_deadline() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        for _ in 0..2 {
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
        }

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_millis(15),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let past = Instant::now();
        let mut loop_state = WorkerLoopState {
            last_tick: past,
            last_served: [past; PRIORITY_TIER_COUNT],
            mailbox: WorkerMailboxState::new(),
        };
        let mut actor = SlowVoteTickActor {
            events: Vec::new(),
            vote_sleep: Duration::from_millis(20),
            tick_delay: Duration::from_millis(5),
            tick_deadline: Cell::new(None),
            tick_calls: 0,
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

        assert_eq!(actor.events, vec!["vote", "tick"]);
        assert_eq!(actor.tick_calls, 1);
        assert_eq!(stats.votes_handled, 1);
        assert!(
            loop_state.mailbox.slots[PriorityTier::Votes.idx()].is_some(),
            "vote should remain buffered for the next iteration"
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 2,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
    fn run_worker_iteration_records_post_tick_tier_timings() {
        status::reset_worker_loop_snapshot_for_tests();

        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let block_created = message::BlockCreated {
            block,
            frontier: None,
        };

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
        let mut actor = TickEnqueueActorWithDelays {
            events: Vec::new(),
            block_payload_tx,
            block_tx,
            block_created,
            payload_sleep: Duration::from_millis(5),
            block_sleep: Duration::from_millis(5),
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

        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(stats.blocks_handled, 1);
        assert!(stats.post_tick_block_payload_drain_ms > 0);
        assert!(stats.post_tick_blocks_drain_ms > 0);
    }

    #[test]
    fn run_worker_iteration_records_pre_tick_tier_timings() {
        status::reset_worker_loop_snapshot_for_tests();

        let (vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
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
            .expect("send vote");
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

        let header = BlockHeader {
            height: NonZeroU64::new(1).expect("non-zero"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();
        let signature = SignatureOf::from_hash(&private_key, header.hash());
        let block_signature = BlockSignature::new(0, signature);
        let block = SignedBlock::presigned_with_da(block_signature, header, Vec::new(), None);
        let block_created = message::BlockCreated {
            block,
            frontier: None,
        };
        block_tx
            .send(inbound(BlockMessage::BlockCreated(block_created)))
            .expect("send block");
        status::record_worker_queue_enqueue(status::WorkerQueueKind::Blocks);

        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
        let mut actor = PreTickDrainActor {
            vote_sleep: Duration::from_millis(5),
            payload_sleep: Duration::from_millis(5),
            block_sleep: Duration::from_millis(5),
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

        assert_eq!(stats.votes_handled, 1);
        assert_eq!(stats.block_payloads_handled, 1);
        assert_eq!(stats.blocks_handled, 1);
        assert!(stats.pre_tick_votes_drain_ms > 0);
        assert!(stats.pre_tick_block_payload_drain_ms > 0);
        assert!(stats.pre_tick_blocks_drain_ms > 0);
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
                    msg: BlockMessageWire::new(BlockMessage::ConsensusParams(
                        message::ConsensusParamsAdvert {
                            collectors_k: 2,
                            redundant_send_r: 2,
                            membership: None,
                        },
                    )),
                })
                .expect("send background request");
            status::record_worker_queue_enqueue(status::WorkerQueueKind::Background);

            let config = WorkerLoopConfig {
                time_budget: Duration::from_secs(1),
                drain_budget_cap: Duration::from_secs(1),
                vote_rx_drain_budget: Duration::from_secs(1),
                block_payload_rx_drain_budget: Duration::from_secs(1),
                block_payload_rx_drain_max_messages: 16,
                vote_rx_drain_max_messages: 16,
                vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
                block_rx_drain_budget: Duration::from_secs(1),
                block_rx_drain_max_messages: 16,
                rbc_chunk_rx_drain_budget: Duration::from_secs(1),
                rbc_chunk_rx_drain_max_messages: 16,
                consensus_rx_drain_max_messages: 16,
                lane_relay_rx_drain_max_messages: 16,
                background_rx_drain_max_messages: 16,
                tick_min_gap: Duration::from_millis(1),
                tick_busy_gap: Duration::from_millis(1),
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
        let _guard = status::worker_queue_test_guard();
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
            prev_roster_evidence_hash: None,
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
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

    #[test]
    fn drain_queue_batch_drains_follow_up_messages_up_to_limit() {
        let _guard = status::worker_queue_test_guard();
        status::reset_worker_loop_snapshot_for_tests();

        let (tx, rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        tx.send(1_u8).expect("send first message");
        tx.send(2_u8).expect("send second message");
        tx.send(3_u8).expect("send third message");

        let first = rx.try_recv().expect("receive first message");
        let mut actor = BatchRecordingActor::default();
        let mut handler = |actor: &mut BatchRecordingActor, msg: u8| -> Result<()> {
            actor.events.push(msg);
            Ok(())
        };

        let drained = drain_queue_batch(
            &mut actor,
            &rx,
            first,
            2,
            status::WorkerQueueKind::RbcChunks,
            "test",
            &mut handler,
        );

        assert_eq!(drained, 2);
        assert_eq!(actor.events, vec![1, 2]);
        assert_eq!(rx.try_recv().expect("remaining queued message"), 3);
    }

    #[test]
    fn actor_gate_serializes_access() {
        let gate = Arc::new(ActorGate::new(Vec::<u8>::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let violation = Arc::new(AtomicBool::new(false));
        let mut joins = Vec::new();
        for idx in 0..4_u8 {
            let gate = Arc::clone(&gate);
            let active = Arc::clone(&active);
            let violation = Arc::clone(&violation);
            joins.push(std::thread::spawn(move || {
                for _ in 0..4 {
                    let mut guard = gate.enter(GatePriority::Regular);
                    let in_flight = active.fetch_add(1, Ordering::SeqCst) + 1;
                    if in_flight > 1 {
                        violation.store(true, Ordering::SeqCst);
                    }
                    guard.actor_mut().push(idx);
                    std::thread::sleep(Duration::from_millis(1));
                    active.fetch_sub(1, Ordering::SeqCst);
                }
            }));
        }
        for join in joins {
            join.join().expect("actor gate thread");
        }
        let mut guard = gate.enter(GatePriority::Regular);
        assert_eq!(guard.actor_mut().len(), 16);
        assert!(!violation.load(Ordering::SeqCst));
    }

    #[test]
    fn actor_gate_prioritizes_urgent_waiters() {
        let gate = Arc::new(ActorGate::new(Vec::<&'static str>::new()));
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let start = Arc::new(Barrier::new(3));

        {
            // Ensure waiters queue through the gate's condition variable instead of racing on the
            // mutex lock used by `ActorGuard`.
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = true;
        }

        let regular_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let mut guard = gate.enter(GatePriority::Regular);
                order.lock().expect("order lock poisoned").push("regular");
                guard.actor_mut().push("regular");
            })
        };

        let urgent_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let mut guard = gate.enter(GatePriority::Urgent);
                order.lock().expect("order lock poisoned").push("urgent");
                guard.actor_mut().push("urgent");
            })
        };

        start.wait();
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while Instant::now() < deadline {
            let (waiting_urgent, waiting_regular) = {
                let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
                (guard.waiting_urgent, guard.waiting_regular)
            };
            if waiting_urgent > 0 && waiting_regular > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        {
            let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            assert!(
                guard.waiting_urgent > 0 && guard.waiting_regular > 0,
                "waiters should queue while gate is held"
            );
        }
        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = false;
        }
        gate.cvar.notify_all();

        regular_join.join().expect("regular waiter thread");
        urgent_join.join().expect("urgent waiter thread");

        let order = order.lock().expect("order lock poisoned");
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], "urgent");
        assert_eq!(order[1], "regular");
    }

    #[test]
    fn actor_gate_regular_waiter_not_starved_by_urgent_burst() {
        let gate = Arc::new(ActorGate::new(Vec::<&'static str>::new()));
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let start = Arc::new(Barrier::new(3));

        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = true;
        }

        let regular_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let mut guard = gate.enter(GatePriority::Regular);
                order.lock().expect("order lock poisoned").push("regular");
                guard.actor_mut().push("regular");
            })
        };

        let urgent_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for _ in 0..(MAX_URGENT_GATE_STREAK + 2) {
                    let mut guard = gate.enter(GatePriority::Urgent);
                    order.lock().expect("order lock poisoned").push("urgent");
                    guard.actor_mut().push("urgent");
                }
            })
        };

        start.wait();
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while Instant::now() < deadline {
            let (waiting_urgent, waiting_regular) = {
                let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
                (guard.waiting_urgent, guard.waiting_regular)
            };
            if waiting_urgent > 0 && waiting_regular > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        {
            let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            assert!(
                guard.waiting_urgent > 0 && guard.waiting_regular > 0,
                "waiters should queue while gate is held"
            );
        }
        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = false;
        }
        gate.cvar.notify_all();

        regular_join.join().expect("regular waiter thread");
        urgent_join.join().expect("urgent burst thread");

        let order = order.lock().expect("order lock poisoned");
        let regular_idx = order
            .iter()
            .position(|label| *label == "regular")
            .expect("regular waiter should have run");
        let burst_len =
            usize::try_from(MAX_URGENT_GATE_STREAK.saturating_add(2)).unwrap_or(usize::MAX);
        assert!(
            regular_idx <= burst_len,
            "regular waiter was starved for too many urgent turns: index={regular_idx}, cap={burst_len}"
        );
    }

    #[test]
    fn actor_gate_da_critical_waiter_not_starved_by_urgent_burst() {
        let gate = Arc::new(ActorGate::new(Vec::<&'static str>::new()));
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let start = Arc::new(Barrier::new(3));

        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = true;
        }

        let da_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let mut guard = gate.enter(GatePriority::DaCritical);
                order
                    .lock()
                    .expect("order lock poisoned")
                    .push("da_critical");
                guard.actor_mut().push("da_critical");
            })
        };

        let urgent_join = {
            let gate = Arc::clone(&gate);
            let order = Arc::clone(&order);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for _ in 0..(MAX_URGENT_BEFORE_DA_CRITICAL + 2) {
                    let mut guard = gate.enter(GatePriority::Urgent);
                    order.lock().expect("order lock poisoned").push("urgent");
                    guard.actor_mut().push("urgent");
                }
            })
        };

        start.wait();
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while Instant::now() < deadline {
            let (waiting_urgent, waiting_da_critical) = {
                let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
                (guard.waiting_urgent, guard.waiting_da_critical)
            };
            if waiting_urgent > 0 && waiting_da_critical > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        {
            let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            assert!(
                guard.waiting_urgent > 0 && guard.waiting_da_critical > 0,
                "waiters should queue while gate is held"
            );
        }
        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = false;
        }
        gate.cvar.notify_all();

        da_join.join().expect("da-critical waiter thread");
        urgent_join.join().expect("urgent burst thread");

        let order = order.lock().expect("order lock poisoned");
        let da_idx = order
            .iter()
            .position(|label| *label == "da_critical")
            .expect("da-critical waiter should have run");
        let cap =
            usize::try_from(MAX_URGENT_BEFORE_DA_CRITICAL.saturating_add(1)).unwrap_or(usize::MAX);
        assert!(
            da_idx <= cap,
            "da-critical waiter was starved for too many urgent turns: index={da_idx}, cap={cap}"
        );
    }

    #[test]
    fn run_parallel_worker_prioritizes_rbc_ingress_before_block_fallback() {
        let gate = Arc::new(ActorGate::new(QueuePriorityRecordingActor::default()));
        let active = Arc::new(AtomicUsize::new(0));
        let shutdown_signal = ShutdownSignal::new();
        let (rbc_tx, rbc_rx) = mpsc::sync_channel(1);
        let (block_tx, block_rx) = mpsc::sync_channel(1);

        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = true;
        }

        let rbc_join = spawn_queue_worker(
            "test-sumeragi-rbc",
            rbc_rx,
            Arc::clone(&gate),
            GatePriority::Urgent,
            1,
            Arc::clone(&active),
            shutdown_signal.clone(),
            PriorityTier::RbcChunks.stage(),
            PriorityTier::RbcChunks.queue_kind(),
            "block_message",
            |actor, msg| actor.on_block_message(msg),
        );
        let block_join = spawn_queue_worker(
            "test-sumeragi-blocks",
            block_rx,
            Arc::clone(&gate),
            GatePriority::DaCritical,
            1,
            Arc::clone(&active),
            shutdown_signal.clone(),
            PriorityTier::Blocks.stage(),
            PriorityTier::Blocks.queue_kind(),
            "block_message",
            |actor, msg| actor.on_block_message(msg),
        );

        let rbc_msg = InboundBlockMessage::new(
            BlockMessage::BlockCreated(message::BlockCreated {
                block: test_signed_block(1, 0),
                frontier: None,
            }),
            None,
        )
        .with_enqueue_metadata(status::WorkerQueueKind::RbcChunks);
        rbc_tx
            .send(rbc_msg)
            .expect("queue BlockCreated on RBC worker");

        let requester = PeerId::new(KeyPair::random().public_key().clone());
        let block_msg = InboundBlockMessage::new(
            BlockMessage::FetchPendingBlock(message::FetchPendingBlock {
                requester,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0x99; Hash::LENGTH])),
                height: 0,
                view: 0,
                priority: None,
                requester_roster_proof_known: None,
            }),
            None,
        )
        .with_enqueue_metadata(status::WorkerQueueKind::Blocks);
        block_tx
            .send(block_msg)
            .expect("queue fallback block message on block worker");

        let wait_deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while Instant::now() < wait_deadline {
            let (waiting_urgent, waiting_da_critical) = {
                let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
                (guard.waiting_urgent, guard.waiting_da_critical)
            };
            if waiting_urgent > 0 && waiting_da_critical > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        {
            let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            assert!(
                guard.waiting_urgent > 0 && guard.waiting_da_critical > 0,
                "RBC and block workers should both be queued while the gate is held"
            );
        }

        {
            let mut guard = gate.state.lock().expect("sumeragi actor gate poisoned");
            guard.in_flight = false;
        }
        gate.cvar.notify_all();

        let processed_deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        while Instant::now() < processed_deadline {
            let processed = {
                let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
                guard.actor.events.len()
            };
            if processed == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        shutdown_signal.send();
        rbc_join.join().expect("RBC queue worker thread");
        block_join.join().expect("block queue worker thread");

        let guard = gate.state.lock().expect("sumeragi actor gate poisoned");
        assert_eq!(guard.actor.events, vec!["rbc", "block"]);
    }

    #[test]
    fn run_parallel_worker_exits_when_shutdown_is_sent() {
        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_wake_tx, wake_rx) = mpsc::sync_channel::<()>(WORKER_WAKE_CHANNEL_CAP);
        let actor = RecordingActor::default();
        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let shutdown_signal = ShutdownSignal::new();
        shutdown_signal.send();

        run_parallel_worker(
            actor,
            config,
            vote_rx,
            block_payload_rx,
            rbc_chunk_rx,
            block_rx,
            consensus_rx,
            lane_rx,
            background_rx,
            wake_rx,
            shutdown_signal,
            MAX_URGENT_BEFORE_DA_CRITICAL,
        );
    }

    #[test]
    fn run_parallel_worker_refreshes_config_each_iteration() {
        let (_vote_tx, vote_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_payload_tx, block_payload_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_rbc_chunk_tx, rbc_chunk_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_block_tx, block_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_consensus_tx, consensus_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_lane_tx, lane_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (_background_tx, background_rx) = mpsc::sync_channel(TEST_CHANNEL_CAP);
        let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(WORKER_WAKE_CHANNEL_CAP);
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let actor = RefreshingActor {
            refresh_count: Arc::clone(&refresh_count),
        };
        let config = WorkerLoopConfig {
            time_budget: Duration::from_secs(1),
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 16,
            vote_rx_drain_max_messages: 16,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 16,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 16,
            consensus_rx_drain_max_messages: 16,
            lane_relay_rx_drain_max_messages: 16,
            background_rx_drain_max_messages: 16,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
        };
        let shutdown_signal = ShutdownSignal::new();
        let shutdown_worker = shutdown_signal.clone();

        let join = std::thread::spawn(move || {
            run_parallel_worker(
                actor,
                config,
                vote_rx,
                block_payload_rx,
                rbc_chunk_rx,
                block_rx,
                consensus_rx,
                lane_rx,
                background_rx,
                wake_rx,
                shutdown_worker,
                MAX_URGENT_BEFORE_DA_CRITICAL,
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
        join.join().expect("parallel worker thread");

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
pub(crate) mod pacing_governor;
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
        msg: BlockMessageWire,
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
        msg: BlockMessageWire,
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
        msg: BlockMessageWire,
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
        msg: BlockMessageWire,
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
    FetchBlockBody {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        requester_hash: CryptoHash,
    },
    BlockBodyResponse {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    },
    Proposal {
        height: u64,
        view: u64,
        payload_hash: CryptoHash,
    },
    RbcInit {
        height: u64,
        view: u64,
        init_hash: CryptoHash,
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
    FetchPendingBlock {
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        requester_hash: CryptoHash,
        priority: message::FetchPendingBlockPriority,
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
const BLOCK_PAYLOAD_DEDUP_KIND_COUNT: usize = 10;
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
    fetch_block_body: DedupCache<BlockPayloadDedupKey>,
    block_body_response: DedupCache<BlockPayloadDedupKey>,
    proposal: DedupCache<BlockPayloadDedupKey>,
    rbc_init: DedupCache<BlockPayloadDedupKey>,
    rbc_ready: DedupCache<BlockPayloadDedupKey>,
    rbc_deliver: DedupCache<BlockPayloadDedupKey>,
    block_sync_update: DedupCache<BlockPayloadDedupKey>,
    fetch_pending_block: DedupCache<BlockPayloadDedupKey>,
    rbc_chunk: DedupCache<BlockPayloadDedupKey>,
}

impl BlockPayloadDedupCache {
    fn new(cap_per_kind: usize, ttl: Duration) -> Self {
        Self {
            block_created: DedupCache::new(cap_per_kind, ttl),
            fetch_block_body: DedupCache::new(cap_per_kind, ttl),
            block_body_response: DedupCache::new(cap_per_kind, ttl),
            proposal: DedupCache::new(cap_per_kind, ttl),
            rbc_init: DedupCache::new(cap_per_kind, ttl),
            rbc_ready: DedupCache::new(cap_per_kind, ttl),
            rbc_deliver: DedupCache::new(cap_per_kind, ttl),
            block_sync_update: DedupCache::new(cap_per_kind, ttl),
            fetch_pending_block: DedupCache::new(cap_per_kind, ttl),
            rbc_chunk: DedupCache::new(cap_per_kind, ttl),
        }
    }

    fn insert(&mut self, key: BlockPayloadDedupKey, now: Instant) -> DedupInsertOutcome {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.insert(key, now),
            BlockPayloadDedupKey::FetchBlockBody { .. } => self.fetch_block_body.insert(key, now),
            BlockPayloadDedupKey::BlockBodyResponse { .. } => {
                self.block_body_response.insert(key, now)
            }
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.insert(key, now),
            BlockPayloadDedupKey::RbcInit { .. } => self.rbc_init.insert(key, now),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.insert(key, now),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.insert(key, now),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.insert(key, now),
            BlockPayloadDedupKey::FetchPendingBlock { .. } => {
                self.fetch_pending_block.insert(key, now)
            }
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.insert(key, now),
        }
    }

    fn remove(&mut self, key: &BlockPayloadDedupKey) -> bool {
        match *key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.remove(key),
            BlockPayloadDedupKey::FetchBlockBody { .. } => self.fetch_block_body.remove(key),
            BlockPayloadDedupKey::BlockBodyResponse { .. } => self.block_body_response.remove(key),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.remove(key),
            BlockPayloadDedupKey::RbcInit { .. } => self.rbc_init.remove(key),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.remove(key),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.remove(key),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.remove(key),
            BlockPayloadDedupKey::FetchPendingBlock { .. } => self.fetch_pending_block.remove(key),
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.remove(key),
        }
    }

    #[cfg(test)]
    fn contains(&self, key: &BlockPayloadDedupKey) -> bool {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.contains(key),
            BlockPayloadDedupKey::FetchBlockBody { .. } => self.fetch_block_body.contains(key),
            BlockPayloadDedupKey::BlockBodyResponse { .. } => {
                self.block_body_response.contains(key)
            }
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.contains(key),
            BlockPayloadDedupKey::RbcInit { .. } => self.rbc_init.contains(key),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.contains(key),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.contains(key),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.contains(key),
            BlockPayloadDedupKey::FetchPendingBlock { .. } => {
                self.fetch_pending_block.contains(key)
            }
            BlockPayloadDedupKey::RbcChunk { .. } => self.rbc_chunk.contains(key),
        }
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.block_created.clear();
        self.fetch_block_body.clear();
        self.block_body_response.clear();
        self.proposal.clear();
        self.rbc_init.clear();
        self.rbc_ready.clear();
        self.rbc_deliver.clear();
        self.block_sync_update.clear();
        self.fetch_pending_block.clear();
        self.rbc_chunk.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.block_created.len()
            + self.fetch_block_body.len()
            + self.block_body_response.len()
            + self.proposal.len()
            + self.rbc_init.len()
            + self.rbc_ready.len()
            + self.rbc_deliver.len()
            + self.block_sync_update.len()
            + self.fetch_pending_block.len()
            + self.rbc_chunk.len()
    }

    #[cfg(test)]
    fn len_for_key(&self, key: &BlockPayloadDedupKey) -> usize {
        match key {
            BlockPayloadDedupKey::BlockCreated { .. } => self.block_created.len(),
            BlockPayloadDedupKey::FetchBlockBody { .. } => self.fetch_block_body.len(),
            BlockPayloadDedupKey::BlockBodyResponse { .. } => self.block_body_response.len(),
            BlockPayloadDedupKey::Proposal { .. } => self.proposal.len(),
            BlockPayloadDedupKey::RbcInit { .. } => self.rbc_init.len(),
            BlockPayloadDedupKey::RbcReady { .. } => self.rbc_ready.len(),
            BlockPayloadDedupKey::RbcDeliver { .. } => self.rbc_deliver.len(),
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => self.block_sync_update.len(),
            BlockPayloadDedupKey::FetchPendingBlock { .. } => self.fetch_pending_block.len(),
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
    enqueued_at: Option<Instant>,
    queue: Option<status::WorkerQueueKind>,
}

impl InboundBlockMessage {
    pub(crate) fn new(message: BlockMessage, sender: Option<PeerId>) -> Self {
        Self {
            message: message.normalize(),
            sender,
            enqueued_at: None,
            queue: None,
        }
    }

    fn with_enqueue_metadata(mut self, queue: status::WorkerQueueKind) -> Self {
        self.enqueued_at = Some(Instant::now());
        self.queue = Some(queue);
        self
    }

    fn queue_latency_ms(&self) -> Option<(status::WorkerQueueKind, u64)> {
        let enqueued_at = self.enqueued_at?;
        let queue = self.queue?;
        let elapsed_ms = u64::try_from(enqueued_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        Some((queue, elapsed_ms))
    }
}

#[derive(Debug, Default)]
struct FrontierBlockSyncHint {
    contiguous_frontier_pressure_active: AtomicBool,
    frontier_lane_active: AtomicBool,
}

impl FrontierBlockSyncHint {
    fn set_contiguous_frontier_pressure_active(&self, active: bool) {
        self.contiguous_frontier_pressure_active
            .store(active, Ordering::Relaxed);
    }

    fn set_frontier_lane_active(&self, active: bool) {
        self.frontier_lane_active.store(active, Ordering::Relaxed);
    }

    fn should_pause_latest_gossip(&self) -> bool {
        self.contiguous_frontier_pressure_active
            .load(Ordering::Relaxed)
            || self.frontier_lane_active.load(Ordering::Relaxed)
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
    frontier_block_sync_hint: Arc<FrontierBlockSyncHint>,
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
        let frontier_block_sync_hint = Arc::new(FrontierBlockSyncHint::default());
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
            frontier_block_sync_hint,
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

    fn frontier_block_sync_hint(&self) -> Arc<FrontierBlockSyncHint> {
        Arc::clone(&self.frontier_block_sync_hint)
    }

    pub(crate) fn should_pause_block_sync_latest_gossip(&self) -> bool {
        self.frontier_block_sync_hint.should_pause_latest_gossip()
    }

    #[cfg(test)]
    pub(crate) fn set_contiguous_frontier_pressure_active_for_tests(&self, active: bool) {
        self.frontier_block_sync_hint
            .set_contiguous_frontier_pressure_active(active);
    }

    #[cfg(test)]
    pub(crate) fn set_frontier_lane_active_for_tests(&self, active: bool) {
        self.frontier_block_sync_hint
            .set_frontier_lane_active(active);
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
            BlockPayloadDedupKey::FetchBlockBody { .. } => {
                status::DedupEvictionKind::FetchBlockBody
            }
            BlockPayloadDedupKey::BlockBodyResponse { .. } => {
                status::DedupEvictionKind::BlockBodyResponse
            }
            BlockPayloadDedupKey::Proposal { .. } => status::DedupEvictionKind::Proposal,
            BlockPayloadDedupKey::RbcInit { .. } => status::DedupEvictionKind::RbcInit,
            BlockPayloadDedupKey::RbcReady { .. } => status::DedupEvictionKind::RbcReady,
            BlockPayloadDedupKey::RbcDeliver { .. } => status::DedupEvictionKind::RbcDeliver,
            BlockPayloadDedupKey::BlockSyncUpdate { .. } => {
                status::DedupEvictionKind::BlockSyncUpdate
            }
            BlockPayloadDedupKey::FetchPendingBlock { .. } => {
                status::DedupEvictionKind::FetchPendingBlock
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
    /// (block creation, proposals, and RBC INIT), plus RBC READY/DELIVER and QC votes/certificates,
    /// always use blocking semantics because dropping them can stall consensus recovery.
    pub fn try_incoming_block_message(&self, msg: BlockMessage) -> bool {
        let msg = msg.normalize();
        let blocking = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(_)
                | BlockMessage::BlockCreated(_)
                | BlockMessage::BlockBodyResponse(_)
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
        let msg = msg.normalize();
        let blocking = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(_)
                | BlockMessage::BlockCreated(_)
                | BlockMessage::BlockBodyResponse(_)
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
                BlockMessage::BlockBodyResponse(response) => {
                    iroha_logger::warn!(
                        height = response.height,
                        view = response.view,
                        block = %response.block_hash,
                        queue = ?queue,
                        reason,
                        "dropping BlockBodyResponse message"
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
                                 mode: IngressMode| {
            let msg = msg.with_enqueue_metadata(queue);
            match mode {
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
            }
        };
        let InboundBlockMessage {
            message: msg,
            sender,
            ..
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
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::RbcReady(message), sender),
                    "RbcReady",
                    status::WorkerQueueKind::RbcChunks,
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
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::RbcDeliver(message), sender),
                    "RbcDeliver",
                    status::WorkerQueueKind::RbcChunks,
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
                // Keep authoritative block-body ingress on the protected RBC lane so the
                // payload-led happy path is not delayed behind advisory proposal traffic.
                enqueue_with_mode(
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::BlockCreated(created), sender),
                    "BlockCreated",
                    status::WorkerQueueKind::RbcChunks,
                    mode,
                )
            }
            BlockMessage::BlockBodyResponse(response) => {
                let duplicate =
                    !self.dedup_block_payload(BlockPayloadDedupKey::BlockBodyResponse {
                        height: response.height,
                        view: response.view,
                        block_hash: response.block_hash,
                    });
                if duplicate {
                    iroha_logger::debug!(
                        height = response.height,
                        view = response.view,
                        block = %response.block_hash,
                        "dropping duplicate BlockBodyResponse from network"
                    );
                    return false;
                }
                enqueue_with_mode(
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::BlockBodyResponse(response), sender),
                    "BlockBodyResponse",
                    status::WorkerQueueKind::RbcChunks,
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
                // Keep Proposal on the legacy payload lane for compatibility and rebroadcast, but
                // it is advisory-only; healthy-path payload ownership begins at BlockCreated.
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
            BlockMessage::RbcInit(init) => {
                let init_hash = CryptoHash::new(init.encode());
                let dedup_key = BlockPayloadDedupKey::RbcInit {
                    height: init.height,
                    view: init.view,
                    init_hash,
                };
                let duplicate = !self.dedup_block_payload(dedup_key);
                if duplicate {
                    iroha_logger::debug!(
                        height = init.height,
                        view = init.view,
                        block = %init.block_hash,
                        "dropping duplicate RBC INIT from network"
                    );
                    return false;
                }
                // Keep INIT on the same protected session ingress lane as
                // BlockCreated/chunks/READY/DELIVER so authoritative payload recovery is not
                // split behind fallback block queue pressure or advisory proposal handling.
                let accepted = enqueue_with_mode(
                    &self.rbc_chunks,
                    {
                        iroha_logger::debug!(
                            height = init.height,
                            view = init.view,
                            block = %init.block_hash,
                            total_chunks = init.total_chunks,
                            mode = ?mode,
                            queue = ?status::WorkerQueueKind::RbcChunks,
                            "enqueueing RBC INIT"
                        );
                        InboundBlockMessage::new(BlockMessage::RbcInit(init), sender)
                    },
                    "RbcInit",
                    status::WorkerQueueKind::RbcChunks,
                    mode,
                );
                if !accepted {
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::RbcChunkCompact(chunk) => {
                let chunk = chunk.into_chunk();
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
                // Respect the caller-selected ingress mode so targeted missing-block recovery
                // chunks can use the blocking path while best-effort callers still drop on
                // saturation via `try_incoming_block_message`.
                let accepted = enqueue_with_mode(
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::RbcChunk(chunk), sender),
                    "RbcChunk",
                    status::WorkerQueueKind::RbcChunks,
                    mode,
                );
                if !accepted {
                    // Allow rebroadcasts to be enqueued if the queue was full.
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
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
                // Respect the caller-selected ingress mode so targeted missing-block recovery
                // chunks can use the blocking path while best-effort callers still drop on
                // saturation via `try_incoming_block_message`.
                let accepted = enqueue_with_mode(
                    &self.rbc_chunks,
                    InboundBlockMessage::new(BlockMessage::RbcChunk(chunk), sender),
                    "RbcChunk",
                    status::WorkerQueueKind::RbcChunks,
                    mode,
                );
                if !accepted {
                    // Allow rebroadcasts to be enqueued if the queue was full.
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::FetchPendingBlock(request) => {
                let requester_hash = CryptoHash::new(request.requester.encode());
                let request_priority = request
                    .priority
                    .unwrap_or(message::FetchPendingBlockPriority::Background);
                let dedup_key = BlockPayloadDedupKey::FetchPendingBlock {
                    height: request.height,
                    view: request.view,
                    block_hash: request.block_hash,
                    requester_hash,
                    priority: request_priority,
                };
                let duplicate = !self.dedup_block_payload(dedup_key);
                if duplicate {
                    iroha_logger::debug!(
                        height = request.height,
                        view = request.view,
                        block = %request.block_hash,
                        requester = %request.requester,
                        "dropping duplicate FetchPendingBlock from network"
                    );
                    return false;
                }
                let accepted = enqueue_with_mode(
                    &self.block,
                    InboundBlockMessage::new(BlockMessage::FetchPendingBlock(request), sender),
                    "FetchPendingBlock",
                    status::WorkerQueueKind::Blocks,
                    mode,
                );
                if !accepted {
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
            BlockMessage::FetchBlockBody(request) => {
                let requester_hash = CryptoHash::new(request.requester.encode());
                let dedup_key = BlockPayloadDedupKey::FetchBlockBody {
                    height: request.height,
                    view: request.view,
                    block_hash: request.block_hash,
                    requester_hash,
                };
                let duplicate = !self.dedup_block_payload(dedup_key);
                if duplicate {
                    iroha_logger::debug!(
                        height = request.height,
                        view = request.view,
                        block = %request.block_hash,
                        requester = %request.requester,
                        "dropping duplicate FetchBlockBody from network"
                    );
                    return false;
                }
                let accepted = enqueue_with_mode(
                    &self.block,
                    InboundBlockMessage::new(BlockMessage::FetchBlockBody(request), sender),
                    "FetchBlockBody",
                    status::WorkerQueueKind::Blocks,
                    mode,
                );
                if !accepted {
                    self.release_block_payload_dedup(&dedup_key);
                }
                accepted
            }
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
        match self.background.send(BackgroundRequest::Post {
            peer,
            msg: BlockMessageWire::new(msg),
        }) {
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
        match self.background.send(BackgroundRequest::Broadcast {
            msg: BlockMessageWire::new(msg),
        }) {
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

/// Build a lightweight Sumeragi handle with configurable payload/block queues for unit tests.
#[cfg(test)]
pub(crate) fn test_sumeragi_handle_with_payload_cap(
    block_payload_cap: usize,
    block_channel_cap: usize,
) -> (SumeragiHandle, mpsc::Receiver<InboundBlockMessage>) {
    let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(block_payload_cap);
    let (block_tx, _block_rx) = mpsc::sync_channel(block_channel_cap);
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
    (handle, block_payload_rx)
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

        status::set_commit_cert_history_cap(config.finality.commit_cert_history_cap);
        status::set_commit_inflight_timeout(config.persistence.commit_inflight_timeout);

        let vote_channel_cap = config.queues.votes.max(1);
        let block_payload_channel_cap = config.queues.block_payload.max(1);
        let rbc_chunk_channel_cap = config.queues.rbc_chunks.max(1);
        let block_channel_cap = config.queues.blocks.max(1);
        let control_msg_channel_cap = config.queues.control.max(1);
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
        let frontier_block_sync_hint = handle.frontier_block_sync_hint();

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
            frontier_block_sync_hint,
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
            sumeragi_thread_builder("sumeragi"),
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
    frontier_block_sync_hint: Arc<FrontierBlockSyncHint>,
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

/// Rate-limit ticks by the minimum gap so queue backlogs do not throttle consensus progress.
fn should_run_tick(now: Instant, last_tick: Instant, min_tick_gap: Duration) -> bool {
    now.saturating_duration_since(last_tick) >= min_tick_gap
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
    params: &iroha_data_model::parameter::system::SumeragiParameters,
    fallback: &iroha_data_model::parameter::system::SumeragiParameters,
) -> (Duration, Duration) {
    let min_finality_ms = if params.min_finality_ms() == 0 {
        fallback.min_finality_ms().max(1)
    } else {
        params.min_finality_ms().max(1)
    };

    let base_block_time_ms = if params.block_time_ms() == 0 {
        fallback.block_time_ms()
    } else {
        params.block_time_ms()
    }
    .max(min_finality_ms);

    let base_commit_time_ms = if params.commit_time_ms() == 0 {
        fallback.commit_time_ms()
    } else {
        params.commit_time_ms()
    }
    .max(base_block_time_ms);

    let pacing_factor_bps = params.effective_pacing_factor_bps();
    let scale_ms = |value: u64| {
        u64::try_from(
            u128::from(value)
                .saturating_mul(u128::from(pacing_factor_bps))
                .saturating_div(10_000),
        )
        .unwrap_or(u64::MAX)
    };
    let block_time_ms = scale_ms(base_block_time_ms).max(min_finality_ms);
    let commit_time_ms = scale_ms(base_commit_time_ms).max(block_time_ms);

    (
        Duration::from_millis(block_time_ms),
        Duration::from_millis(commit_time_ms),
    )
}

fn worker_time_budget(
    block_time: Duration,
    commit_time: Duration,
    _da_enabled: bool,
    _da_quorum_timeout_multiplier: u32,
    budget_cap: Duration,
) -> Duration {
    // Keep loop responsiveness anchored to proposal/commit cadence.
    // DA-extended quorum windows can be seconds long and should not be used as a
    // per-iteration drain budget because that delays QC replay/commit under load.
    let window = match (block_time.is_zero(), commit_time.is_zero()) {
        (true, true) => Duration::from_millis(1),
        (false, true) => block_time,
        (true, false) => commit_time,
        (false, false) => block_time.min(commit_time),
    };
    let scaled = window.checked_div(4).unwrap_or(Duration::from_millis(1));
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

const TIME_BUDGET_FLOOR_MS: u64 = 50;
const TIME_BUDGET_CAP_MS: u64 = 2_000;
const IDLE_TICK_GAP_FLOOR_MS: u64 = 50;
const BUSY_TICK_GAP_FLOOR_MS: u64 = 10;
const BUSY_TICK_GAP_DIVISOR: u32 = 16;
// Keep idle poll cadence short so missed/coalesced wake signals do not introduce ~1s
// reaction latency in steady-state consensus ticking.
const IDLE_SHUTDOWN_POLL_MS: u64 = 100;
const DRAIN_BUDGET_CAP_MS: u64 = 2_000;
const VOTE_DRAIN_BUDGET_CAP_MS: u64 = 2_000;
const RBC_DRAIN_BUDGET_CAP_MS: u64 = 2_000;

fn idle_tick_gap(block_time: Duration, commit_time: Duration, max_tick_gap: Duration) -> Duration {
    let base = block_time.min(commit_time);
    let raw = if base == Duration::ZERO {
        max_tick_gap
    } else {
        base / 4
    };
    let gap = raw
        .max(Duration::from_millis(IDLE_TICK_GAP_FLOOR_MS))
        .min(max_tick_gap);
    gap
}

fn busy_tick_gap(block_time: Duration, commit_time: Duration, idle_tick_gap: Duration) -> Duration {
    let base = block_time.min(commit_time);
    let raw = if base == Duration::ZERO {
        idle_tick_gap
    } else {
        base / BUSY_TICK_GAP_DIVISOR
    };
    raw.max(Duration::from_millis(BUSY_TICK_GAP_FLOOR_MS))
        .min(idle_tick_gap)
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

fn block_backlog_drain_cap(queue_depth: u64) -> usize {
    match queue_depth {
        0 => 0,
        1..=3 => BLOCK_RX_BACKLOG_DRAIN_CAP_SMALL,
        4..=15 => BLOCK_RX_BACKLOG_DRAIN_CAP_MEDIUM,
        16..=63 => BLOCK_RX_BACKLOG_DRAIN_CAP_LARGE,
        _ => BLOCK_RX_BACKLOG_DRAIN_CAP_HUGE,
    }
}

fn apply_adaptive_drain_caps(
    cfg: &mut WorkerLoopConfig,
    queue_depths: status::WorkerQueueDepthSnapshot,
) {
    let block_payload_base = cfg.block_payload_rx_drain_max_messages.max(1);
    let rbc_chunk_base = cfg.rbc_chunk_rx_drain_max_messages.max(1);
    if queue_depths.vote_rx > 0 {
        let reduced = (block_payload_base / BLOCK_PAYLOAD_DRAIN_BACKLOG_DIVISOR)
            .max(BLOCK_PAYLOAD_DRAIN_BACKLOG_MIN);
        let cap = reduced.min(block_payload_base);
        if cap != block_payload_base {
            cfg.block_payload_rx_drain_max_messages = cap;
            iroha_logger::debug!(
                vote_rx = queue_depths.vote_rx,
                block_payload_rx = queue_depths.block_payload_rx,
                base_cap = block_payload_base,
                adaptive_cap = cap,
                "adaptive block payload drain cap applied due to vote backlog"
            );
        }
    }
    if queue_depths.block_rx > 0 {
        let target_block_cap = block_backlog_drain_cap(queue_depths.block_rx).max(1);
        let block_base = cfg.block_rx_drain_max_messages.max(1);
        let block_cap = block_base.min(target_block_cap);
        if block_cap != block_base {
            cfg.block_rx_drain_max_messages = block_cap;
            iroha_logger::debug!(
                block_rx = queue_depths.block_rx,
                base_cap = block_base,
                adaptive_cap = block_cap,
                "adaptive block drain cap applied due to block backlog"
            );
        }
        let payload_target = target_block_cap
            .saturating_mul(3)
            .saturating_div(4)
            .max(BLOCK_BACKLOG_PAYLOAD_RBC_MIN_CAP);
        let payload_base = cfg.block_payload_rx_drain_max_messages.max(1);
        let payload_cap = block_payload_base.min(payload_target);
        if payload_cap != payload_base {
            cfg.block_payload_rx_drain_max_messages = payload_cap;
            iroha_logger::debug!(
                block_rx = queue_depths.block_rx,
                block_payload_rx = queue_depths.block_payload_rx,
                base_cap = payload_base,
                adaptive_cap = payload_cap,
                "adaptive block payload drain cap applied due to block backlog"
            );
        }
        let rbc_base = cfg.rbc_chunk_rx_drain_max_messages.max(1);
        let rbc_cap = rbc_chunk_base.min(payload_target);
        if rbc_cap != rbc_base {
            cfg.rbc_chunk_rx_drain_max_messages = rbc_cap;
            iroha_logger::debug!(
                block_rx = queue_depths.block_rx,
                rbc_chunk_rx = queue_depths.rbc_chunk_rx,
                base_cap = rbc_base,
                adaptive_cap = rbc_cap,
                "adaptive RBC chunk drain cap applied due to block backlog"
            );
        }
    }
}

trait WorkerActor {
    fn on_block_message(&mut self, msg: InboundBlockMessage) -> Result<()>;
    fn on_consensus_control(&mut self, msg: ControlFlow) -> Result<()>;
    fn on_lane_relay(&mut self, message: LaneRelayMessage) -> Result<()>;
    fn on_background_request(&mut self, request: BackgroundRequest) -> Result<()>;
    fn sync_external_hints(&mut self) {}
    fn refresh_worker_loop_config(&mut self, _cfg: &mut WorkerLoopConfig) {}
    fn poll_commit_results(&mut self) -> bool {
        false
    }
    fn poll_validation_results(&mut self) -> bool {
        false
    }
    fn poll_qc_verify_results(&mut self) -> bool {
        false
    }
    fn poll_vote_verify_results(&mut self) -> bool {
        false
    }
    fn poll_rbc_persist_results(&mut self) -> bool {
        false
    }
    fn should_bypass_tick_gap(&self) -> bool {
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

    fn sync_external_hints(&mut self) {
        crate::sumeragi::main_loop::Actor::sync_external_hints(self);
    }

    fn poll_commit_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_commit_results(self)
    }

    fn poll_validation_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_validation_results(self)
    }

    fn poll_qc_verify_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_qc_verify_results(self)
    }

    fn poll_vote_verify_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_vote_verify_results(self)
    }

    fn poll_rbc_persist_results(&mut self) -> bool {
        crate::sumeragi::main_loop::Actor::poll_rbc_persist_results_inner(self)
    }

    fn should_bypass_tick_gap(&self) -> bool {
        crate::sumeragi::main_loop::Actor::commit_pipeline_wakeup_pending(self)
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

struct ActorGate<A> {
    state: Mutex<ActorGateState<A>>,
    cvar: Condvar,
    max_urgent_before_da_critical: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GatePriority {
    Urgent,
    DaCritical,
    Regular,
}

const MAX_URGENT_GATE_STREAK: u32 = 32;
#[cfg(test)]
const MAX_URGENT_BEFORE_DA_CRITICAL: u32 =
    iroha_config::parameters::defaults::sumeragi::WORKER_MAX_URGENT_BEFORE_DA_CRITICAL;

struct ActorGateState<A> {
    actor: A,
    in_flight: bool,
    waiting_urgent: u32,
    waiting_da_critical: u32,
    waiting_regular: u32,
    urgent_streak: u32,
}

struct ActorGuard<'a, A> {
    gate: &'a ActorGate<A>,
    priority: GatePriority,
    guard: Option<std::sync::MutexGuard<'a, ActorGateState<A>>>,
}

impl<A> ActorGate<A> {
    #[cfg(test)]
    fn new(actor: A) -> Self {
        Self::with_limits(actor, MAX_URGENT_BEFORE_DA_CRITICAL)
    }

    fn with_limits(actor: A, max_urgent_before_da_critical: u32) -> Self {
        Self {
            state: Mutex::new(ActorGateState {
                actor,
                in_flight: false,
                waiting_urgent: 0,
                waiting_da_critical: 0,
                waiting_regular: 0,
                urgent_streak: 0,
            }),
            cvar: Condvar::new(),
            max_urgent_before_da_critical: max_urgent_before_da_critical.max(1),
        }
    }

    fn can_enter(
        priority: GatePriority,
        state: &ActorGateState<A>,
        max_urgent_before_da_critical: u32,
    ) -> bool {
        if state.in_flight {
            return false;
        }
        match priority {
            GatePriority::Urgent => {
                if state.waiting_da_critical > 0 {
                    state.urgent_streak < max_urgent_before_da_critical
                } else {
                    !(state.waiting_regular > 0 && state.urgent_streak >= MAX_URGENT_GATE_STREAK)
                }
            }
            GatePriority::DaCritical => {
                state.waiting_urgent == 0 || state.urgent_streak >= max_urgent_before_da_critical
            }
            GatePriority::Regular => {
                state.waiting_da_critical == 0
                    && (state.waiting_urgent == 0 || state.urgent_streak >= MAX_URGENT_GATE_STREAK)
            }
        }
    }

    fn enter(&self, priority: GatePriority) -> ActorGuard<'_, A> {
        let mut guard = self.state.lock().expect("sumeragi actor gate poisoned");
        match priority {
            GatePriority::Urgent => {
                guard.waiting_urgent = guard.waiting_urgent.saturating_add(1);
            }
            GatePriority::DaCritical => {
                guard.waiting_da_critical = guard.waiting_da_critical.saturating_add(1);
            }
            GatePriority::Regular => {
                guard.waiting_regular = guard.waiting_regular.saturating_add(1);
            }
        }
        while !Self::can_enter(priority, &guard, self.max_urgent_before_da_critical) {
            guard = self.cvar.wait(guard).expect("sumeragi actor gate poisoned");
        }
        guard.in_flight = true;
        match priority {
            GatePriority::Urgent => {
                guard.waiting_urgent = guard.waiting_urgent.saturating_sub(1);
                guard.urgent_streak = guard.urgent_streak.saturating_add(1);
            }
            GatePriority::DaCritical => {
                guard.waiting_da_critical = guard.waiting_da_critical.saturating_sub(1);
                guard.urgent_streak = 0;
            }
            GatePriority::Regular => {
                guard.waiting_regular = guard.waiting_regular.saturating_sub(1);
                guard.urgent_streak = 0;
            }
        }
        ActorGuard {
            gate: self,
            priority,
            guard: Some(guard),
        }
    }
}

impl<A> ActorGuard<'_, A> {
    fn actor_mut(&mut self) -> &mut A {
        &mut self
            .guard
            .as_mut()
            .expect("sumeragi actor guard missing")
            .actor
    }
}

impl<A> Drop for ActorGuard<'_, A> {
    fn drop(&mut self) {
        let Some(mut guard) = self.guard.take() else {
            return;
        };
        if guard.in_flight {
            guard.in_flight = false;
            if !matches!(self.priority, GatePriority::Urgent) {
                guard.urgent_streak = 0;
            }
        }
        self.gate.cvar.notify_all();
    }
}

fn poll_worker_results<A: WorkerActor>(actor: &mut A) -> bool {
    let mut progress = false;
    progress |= actor.poll_commit_results();
    progress |= actor.poll_validation_results();
    progress |= actor.poll_qc_verify_results();
    progress |= actor.poll_vote_verify_results();
    progress |= actor.poll_rbc_persist_results();
    actor.sync_external_hints();
    progress
}

const PRIORITY_TIER_COUNT: usize = 7;
// Keep vote processing ahead of payload tiers; drain fast-path block messages before heavy payloads.
const VOTE_BURST_CAP: usize = 32;
// When payloads are already queued, shrink vote preference to reduce QC-without-payload gaps.
#[cfg(test)]
const VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG: usize =
    iroha_config::parameters::defaults::sumeragi::WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG;
// Keep votes prioritized under block backlog, but bound burst size so block progress messages
// are not starved behind long vote runs.
const VOTE_BURST_CAP_WITH_BLOCKS: usize =
    iroha_config::parameters::defaults::sumeragi::WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG;
const BLOCK_PAYLOAD_DRAIN_BACKLOG_DIVISOR: usize = 4;
const BLOCK_PAYLOAD_DRAIN_BACKLOG_MIN: usize = 2;
const BLOCK_RX_URGENT_FRACTION: u32 = 4;
const BLOCK_RX_URGENT_FLOOR_MS: u64 = 200;
const BLOCK_RX_BACKLOG_DRAIN_CAP_SMALL: usize = 4;
const BLOCK_RX_BACKLOG_DRAIN_CAP_MEDIUM: usize = 8;
const BLOCK_RX_BACKLOG_DRAIN_CAP_LARGE: usize = 16;
const BLOCK_RX_BACKLOG_DRAIN_CAP_HUGE: usize = 32;
const BLOCK_BACKLOG_PAYLOAD_RBC_MIN_CAP: usize = 8;
// Keep the full healthy-path RBC session ingress chain together under parallel
// ingress so BlockCreated/INIT/chunk/READY/DELIVER work is not delayed behind
// a fresh vote/control burst.
const RBC_PARALLEL_BATCH_LIMIT: usize = 4;

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
        PriorityTier::Blocks,
        PriorityTier::BlockPayload,
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
    PriorityTier::Blocks,
    PriorityTier::BlockPayload,
];
const NON_VOTE_PAYLOAD_TIERS: [PriorityTier; 2] =
    [PriorityTier::RbcChunks, PriorityTier::BlockPayload];
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
    drain_budget_cap: Duration,
    vote_rx_drain_budget: Duration,
    block_payload_rx_drain_budget: Duration,
    block_payload_rx_drain_max_messages: usize,
    vote_rx_drain_max_messages: usize,
    vote_burst_cap_with_payload_backlog: usize,
    block_rx_drain_budget: Duration,
    block_rx_drain_max_messages: usize,
    rbc_chunk_rx_drain_budget: Duration,
    rbc_chunk_rx_drain_max_messages: usize,
    consensus_rx_drain_max_messages: usize,
    lane_relay_rx_drain_max_messages: usize,
    background_rx_drain_max_messages: usize,
    tick_min_gap: Duration,
    tick_busy_gap: Duration,
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
    vote_drain_ms: u64,
    block_payload_drain_ms: u64,
    pre_tick_votes_drain_ms: u64,
    pre_tick_block_payload_drain_ms: u64,
    pre_tick_blocks_drain_ms: u64,
    post_tick_votes_drain_ms: u64,
    post_tick_block_payload_drain_ms: u64,
    post_tick_blocks_drain_ms: u64,
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

fn tier_budget_exhausted(
    mailbox: &mut WorkerMailbox<'_>,
    budgets: &TierBudgets,
    tier: PriorityTier,
) -> bool {
    if budgets.remaining(tier) > 0 {
        return false;
    }
    if mailbox.has_pending(tier) {
        return true;
    }
    mailbox.fill_slot(tier);
    mailbox.has_pending(tier)
}

fn refresh_budget_exhaustion_flags(
    mailbox: &mut WorkerMailbox<'_>,
    budgets: &TierBudgets,
    stats: &mut WorkerIterationStats,
) {
    stats.vote_rx_budget_exhausted |= tier_budget_exhausted(mailbox, budgets, PriorityTier::Votes);
    stats.block_payload_rx_budget_exhausted |=
        tier_budget_exhausted(mailbox, budgets, PriorityTier::BlockPayload);
    stats.rbc_chunk_rx_budget_exhausted |=
        tier_budget_exhausted(mailbox, budgets, PriorityTier::RbcChunks);
    stats.block_rx_budget_exhausted |=
        tier_budget_exhausted(mailbox, budgets, PriorityTier::Blocks);
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

fn should_warn_slow_iteration_with_budget(
    cfg: &WorkerLoopConfig,
    stats: &WorkerIterationStats,
    iter_elapsed: Duration,
) -> bool {
    iter_elapsed >= cfg.time_budget && should_warn_slow_iteration(stats)
}

#[derive(Clone, Copy, Debug)]
enum DrainPhase {
    PreTick,
    PostTick,
}

fn drain_mailbox<A: WorkerActor>(
    actor: &mut A,
    cfg: &WorkerLoopConfig,
    _tick_gap: Duration,
    iter_start: Instant,
    mailbox: &mut WorkerMailbox<'_>,
    budgets: &mut TierBudgets,
    stats: &mut WorkerIterationStats,
    last_served: &mut [Instant; PRIORITY_TIER_COUNT],
    phase: DrainPhase,
    tick_deadline: Option<Instant>,
) {
    let vote_burst = if has_pending_non_vote_payload(mailbox, budgets) {
        cfg.vote_burst_cap_with_payload_backlog.max(1)
    } else if mailbox.has_pending(PriorityTier::Blocks) {
        VOTE_BURST_CAP_WITH_BLOCKS
    } else {
        VOTE_BURST_CAP
    };
    // Cap per-iteration draining so ticks cannot be starved by long queue backlogs.
    let drain_budget = cfg
        .time_budget
        .min(cfg.tick_max_gap)
        .min(cfg.drain_budget_cap);
    let drain_budget_is_time_limited = drain_budget == cfg.time_budget;
    let drain_budget_deadline = iter_start.checked_add(drain_budget).unwrap_or(iter_start);
    let tick_deadline = tick_deadline
        .filter(|deadline| *deadline > iter_start)
        .filter(|deadline| *deadline <= drain_budget_deadline);
    let mut overtime_non_vote_turn = false;
    loop {
        if !mailbox.any_pending() {
            break;
        }
        let now = Instant::now();
        if let Some(deadline) = tick_deadline {
            if now >= deadline {
                break;
            }
        }
        if now >= drain_budget_deadline {
            let payload_pending =
                oldest_pending_non_vote_payload(now, mailbox, budgets, last_served).is_some();
            let grant_overtime_turn = !overtime_non_vote_turn
                && matches!(phase, DrainPhase::PreTick)
                && drain_budget_is_time_limited
                && stats.votes_handled > 0
                && stats.block_payloads_handled == 0
                && stats.rbc_chunks_handled == 0
                && payload_pending;
            if grant_overtime_turn {
                overtime_non_vote_turn = true;
            } else {
                stats.budget_exceeded = true;
                break;
            }
        }
        let payload_turn = oldest_pending_non_vote_payload(now, mailbox, budgets, last_served);
        let votes_pending =
            budgets.remaining(PriorityTier::Votes) > 0 && mailbox.has_pending(PriorityTier::Votes);
        let force_payload_turn = overtime_non_vote_turn
            && stats.rbc_chunks_handled == 0
            && stats.block_payloads_handled == 0
            && payload_turn.is_some();
        let suppress_starved_payload_preemption = stats.votes_handled == 0
            && stats.rbc_chunks_handled == 0
            && stats.block_payloads_handled == 0
            && stats.blocks_handled == 0
            && stats.consensus_handled == 0
            && stats.lane_relays_handled == 0
            && stats.background_handled == 0;
        let prefer_votes = votes_pending && !force_payload_turn && stats.votes_handled < vote_burst;
        let tier = if force_payload_turn {
            payload_turn
        } else {
            select_next_tier(
                now,
                mailbox,
                budgets,
                last_served,
                cfg,
                prefer_votes,
                suppress_starved_payload_preemption,
            )
        };
        let Some(tier) = tier else {
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
                let drain_start = matches!(
                    tier,
                    PriorityTier::Votes | PriorityTier::BlockPayload | PriorityTier::Blocks
                )
                .then(Instant::now);
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
                if let Some(start) = drain_start {
                    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    match tier {
                        PriorityTier::Votes => {
                            stats.vote_drain_ms = stats.vote_drain_ms.saturating_add(elapsed_ms);
                            if matches!(phase, DrainPhase::PreTick) {
                                stats.pre_tick_votes_drain_ms =
                                    stats.pre_tick_votes_drain_ms.saturating_add(elapsed_ms);
                            }
                            if matches!(phase, DrainPhase::PostTick) {
                                stats.post_tick_votes_drain_ms =
                                    stats.post_tick_votes_drain_ms.saturating_add(elapsed_ms);
                            }
                        }
                        PriorityTier::BlockPayload => {
                            stats.block_payload_drain_ms =
                                stats.block_payload_drain_ms.saturating_add(elapsed_ms);
                            if matches!(phase, DrainPhase::PreTick) {
                                stats.pre_tick_block_payload_drain_ms = stats
                                    .pre_tick_block_payload_drain_ms
                                    .saturating_add(elapsed_ms);
                            }
                            if matches!(phase, DrainPhase::PostTick) {
                                stats.post_tick_block_payload_drain_ms = stats
                                    .post_tick_block_payload_drain_ms
                                    .saturating_add(elapsed_ms);
                            }
                        }
                        PriorityTier::Blocks => {
                            if matches!(phase, DrainPhase::PreTick) {
                                stats.pre_tick_blocks_drain_ms =
                                    stats.pre_tick_blocks_drain_ms.saturating_add(elapsed_ms);
                            }
                            if matches!(phase, DrainPhase::PostTick) {
                                stats.post_tick_blocks_drain_ms =
                                    stats.post_tick_blocks_drain_ms.saturating_add(elapsed_ms);
                            }
                        }
                        _ => {}
                    }
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
        vote_drain_ms: 0,
        block_payload_drain_ms: 0,
        pre_tick_votes_drain_ms: 0,
        pre_tick_block_payload_drain_ms: 0,
        pre_tick_blocks_drain_ms: 0,
        post_tick_votes_drain_ms: 0,
        post_tick_block_payload_drain_ms: 0,
        post_tick_blocks_drain_ms: 0,
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
    let mut cfg = *cfg;
    let queue_depths = status::worker_queue_depth_snapshot();
    apply_adaptive_drain_caps(&mut cfg, queue_depths);
    if queue_depths.block_rx > 0 {
        cfg.vote_rx_drain_max_messages = cfg
            .vote_rx_drain_max_messages
            .max(VOTE_BURST_CAP_WITH_BLOCKS.saturating_add(1));
    }
    let pre_tick_gap = if has_pending_queue_depths(queue_depths) {
        cfg.tick_busy_gap
    } else {
        cfg.tick_min_gap
    };
    let mut budgets = TierBudgets::new(&cfg);
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
    let pre_tick_deadline = actor.next_tick_deadline(iter_start);
    let drain_start = Instant::now();
    drain_mailbox(
        actor,
        &cfg,
        pre_tick_gap,
        iter_start,
        &mut mailbox,
        &mut budgets,
        &mut stats,
        last_served,
        DrainPhase::PreTick,
        pre_tick_deadline,
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
    if actor.poll_qc_verify_results() {
        stats.progress = true;
    }
    if actor.poll_vote_verify_results() {
        stats.progress = true;
    }
    if actor.poll_rbc_persist_results() {
        stats.progress = true;
    }

    let pre_tick_depths = status::worker_queue_depth_snapshot();
    refresh_budget_exhaustion_flags(&mut mailbox, &budgets, &mut stats);

    let tick_now = Instant::now();
    let tick_deadline = actor.next_tick_deadline(tick_now);
    let tick_due = tick_deadline.is_some_and(|deadline| deadline <= tick_now);
    let busy = has_pending_queue_depths(pre_tick_depths)
        || stats.vote_rx_budget_exhausted
        || stats.block_payload_rx_budget_exhausted
        || stats.rbc_chunk_rx_budget_exhausted
        || stats.block_rx_budget_exhausted;
    let tick_gap = if busy {
        cfg.tick_busy_gap
    } else {
        cfg.tick_min_gap
    };
    let bypass_tick_gap = actor.should_bypass_tick_gap();
    if tick_due && (bypass_tick_gap || should_run_tick(tick_now, *last_tick, tick_gap)) {
        status::set_worker_stage(status::WorkerLoopStage::Tick);
        let tick_start = Instant::now();
        stats.progress |= actor.tick();
        stats.tick_elapsed_ms = u64::try_from(tick_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        actor.sync_external_hints();
        *last_tick = tick_now;
    }

    if !stats.budget_exceeded && iter_start.elapsed() < cfg.time_budget {
        mailbox.fill_slots(&budgets);
        let drain_start = Instant::now();
        drain_mailbox(
            actor,
            &cfg,
            cfg.tick_min_gap,
            iter_start,
            &mut mailbox,
            &mut budgets,
            &mut stats,
            last_served,
            DrainPhase::PostTick,
            None,
        );
        stats.post_tick_drain_ms =
            u64::try_from(drain_start.elapsed().as_millis()).unwrap_or(u64::MAX);
    }

    let post_tick_depths = status::worker_queue_depth_snapshot();
    refresh_budget_exhaustion_flags(&mut mailbox, &budgets, &mut stats);

    stats.queue_depths = post_tick_depths;
    actor.sync_external_hints();
    if stats.votes_handled > 0 || stats.block_payloads_handled > 0 || stats.blocks_handled > 0 {
        iroha_logger::debug!(
            votes_handled = stats.votes_handled,
            vote_drain_ms = stats.vote_drain_ms,
            block_payloads_handled = stats.block_payloads_handled,
            block_payload_drain_ms = stats.block_payload_drain_ms,
            blocks_handled = stats.blocks_handled,
            pre_tick_votes_drain_ms = stats.pre_tick_votes_drain_ms,
            pre_tick_block_payload_drain_ms = stats.pre_tick_block_payload_drain_ms,
            pre_tick_blocks_drain_ms = stats.pre_tick_blocks_drain_ms,
            post_tick_votes_drain_ms = stats.post_tick_votes_drain_ms,
            post_tick_block_payload_drain_ms = stats.post_tick_block_payload_drain_ms,
            post_tick_blocks_drain_ms = stats.post_tick_blocks_drain_ms,
            "sumeragi worker drain timings by tier"
        );
    }
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

fn block_rx_urgent_gap(cfg: &WorkerLoopConfig) -> Duration {
    let base = cfg.block_rx_starve_max / BLOCK_RX_URGENT_FRACTION;
    let floor = Duration::from_millis(BLOCK_RX_URGENT_FLOOR_MS);
    base.max(floor).min(cfg.block_rx_starve_max)
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

fn has_pending_non_vote_payload(mailbox: &WorkerMailbox<'_>, budgets: &TierBudgets) -> bool {
    NON_VOTE_PAYLOAD_TIERS
        .iter()
        .copied()
        .any(|tier| budgets.remaining(tier) > 0 && mailbox.has_pending(tier))
}

fn oldest_pending_non_vote_payload(
    now: Instant,
    mailbox: &WorkerMailbox<'_>,
    budgets: &TierBudgets,
    last_served: &[Instant; PRIORITY_TIER_COUNT],
) -> Option<PriorityTier> {
    select_oldest_pending(now, mailbox, budgets, last_served, &NON_VOTE_PAYLOAD_TIERS)
}

fn select_next_tier(
    now: Instant,
    mailbox: &WorkerMailbox<'_>,
    budgets: &TierBudgets,
    last_served: &[Instant; PRIORITY_TIER_COUNT],
    cfg: &WorkerLoopConfig,
    prefer_votes: bool,
    suppress_starved_payload_preemption: bool,
) -> Option<PriorityTier> {
    let votes_pending =
        budgets.remaining(PriorityTier::Votes) > 0 && mailbox.has_pending(PriorityTier::Votes);
    let non_vote_payload_pending = (budgets.remaining(PriorityTier::RbcChunks) > 0
        && mailbox.has_pending(PriorityTier::RbcChunks))
        || (budgets.remaining(PriorityTier::BlockPayload) > 0
            && mailbox.has_pending(PriorityTier::BlockPayload));
    let blocks_pending =
        budgets.remaining(PriorityTier::Blocks) > 0 && mailbox.has_pending(PriorityTier::Blocks);
    if blocks_pending && !non_vote_payload_pending {
        let urgent_gap = block_rx_urgent_gap(cfg);
        if now.saturating_duration_since(last_served[PriorityTier::Blocks.idx()]) >= urgent_gap {
            return Some(PriorityTier::Blocks);
        }
    }
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
        let starve_threshold = max_starve.saturating_add(max_starve / 2);
        if elapsed > starve_threshold {
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
        if !(suppress_starved_payload_preemption
            && tier == PriorityTier::BlockPayload
            && votes_pending)
        {
            return Some(tier);
        }
    }
    if prefer_votes && votes_pending {
        return Some(PriorityTier::Votes);
    }
    // After the vote burst, rotate to the oldest pending tier while keeping votes ahead of
    // heavy payload drains when block payloads are backlogged.
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
        drain_budget_cap = ?cfg.drain_budget_cap,
        vote_rx_drain_budget = ?cfg.vote_rx_drain_budget,
        block_payload_rx_drain_budget = ?cfg.block_payload_rx_drain_budget,
        rbc_chunk_rx_drain_budget = ?cfg.rbc_chunk_rx_drain_budget,
        block_rx_drain_budget = ?cfg.block_rx_drain_budget,
        tick_min_gap = ?cfg.tick_min_gap,
        tick_busy_gap = ?cfg.tick_busy_gap,
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
            let bypass_tick_gap = actor.should_bypass_tick_gap();
            let wait = match tick_deadline {
                None => Some(Duration::from_millis(IDLE_SHUTDOWN_POLL_MS)),
                Some(deadline) => {
                    let due_wait = deadline.saturating_duration_since(now);
                    let min_gap_wait = if bypass_tick_gap && deadline <= now {
                        None
                    } else {
                        idle_wait_duration(now, loop_state.last_tick, cfg.tick_min_gap)
                    };
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
        if should_warn_slow_iteration_with_budget(&cfg, &stats, iter_elapsed) {
            iroha_logger::warn!(
                elapsed_ms = iter_elapsed.as_millis(),
                votes_handled = stats.votes_handled,
                block_payloads_handled = stats.block_payloads_handled,
                blocks_handled = stats.blocks_handled,
                rbc_chunks_handled = stats.rbc_chunks_handled,
                consensus_handled = stats.consensus_handled,
                lane_relays_handled = stats.lane_relays_handled,
                background_handled = stats.background_handled,
                vote_drain_ms = stats.vote_drain_ms,
                block_payload_drain_ms = stats.block_payload_drain_ms,
                pre_tick_drain_ms = stats.pre_tick_drain_ms,
                tick_elapsed_ms = stats.tick_elapsed_ms,
                post_tick_drain_ms = stats.post_tick_drain_ms,
                pre_tick_votes_drain_ms = stats.pre_tick_votes_drain_ms,
                pre_tick_block_payload_drain_ms = stats.pre_tick_block_payload_drain_ms,
                pre_tick_blocks_drain_ms = stats.pre_tick_blocks_drain_ms,
                post_tick_votes_drain_ms = stats.post_tick_votes_drain_ms,
                post_tick_block_payload_drain_ms = stats.post_tick_block_payload_drain_ms,
                post_tick_blocks_drain_ms = stats.post_tick_blocks_drain_ms,
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

#[allow(clippy::too_many_arguments)]
fn spawn_queue_worker<A, T, F>(
    name: &'static str,
    rx: mpsc::Receiver<T>,
    gate: Arc<ActorGate<A>>,
    gate_priority: GatePriority,
    max_batch_messages: usize,
    active: Arc<AtomicUsize>,
    shutdown_signal: ShutdownSignal,
    stage: status::WorkerLoopStage,
    queue_kind: status::WorkerQueueKind,
    handler_label: &'static str,
    mut handler: F,
) -> std::thread::JoinHandle<()>
where
    A: WorkerActor + Send + 'static,
    T: Send + 'static,
    F: FnMut(&mut A, T) -> Result<()> + Send + 'static,
{
    sumeragi_thread_builder(name)
        .spawn(move || {
            loop {
                if shutdown_signal.is_sent() {
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(IDLE_SHUTDOWN_POLL_MS)) {
                    Ok(msg) => {
                        let iter_start = Instant::now();
                        active.fetch_add(1, Ordering::Relaxed);
                        let mut guard = gate.enter(gate_priority);
                        status::set_worker_stage(stage);
                        let _drained = drain_queue_batch(
                            guard.actor_mut(),
                            &rx,
                            msg,
                            max_batch_messages,
                            queue_kind,
                            handler_label,
                            &mut handler,
                        );
                        status::record_worker_iteration(
                            u64::try_from(iter_start.elapsed().as_millis()).unwrap_or(u64::MAX),
                        );
                        drop(guard);
                        if active.fetch_sub(1, Ordering::Relaxed) == 1 {
                            status::set_worker_stage(status::WorkerLoopStage::Idle);
                        }
                        std::thread::yield_now();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .expect("sumeragi queue worker thread spawn failed")
}

fn drain_queue_batch<A, T, F>(
    actor: &mut A,
    rx: &mpsc::Receiver<T>,
    first_msg: T,
    max_batch_messages: usize,
    queue_kind: status::WorkerQueueKind,
    handler_label: &'static str,
    handler: &mut F,
) -> usize
where
    A: WorkerActor,
    F: FnMut(&mut A, T) -> Result<()>,
{
    let mut drained = 0_usize;
    let max_batch_messages = max_batch_messages.max(1);
    let mut next_msg = Some(first_msg);

    while let Some(msg) = next_msg.take() {
        if let Err(err) = handler(actor, msg) {
            iroha_logger::error!(
                ?err,
                queue = ?queue_kind,
                handler = handler_label,
                "Sumeragi worker handler failed"
            );
        }
        poll_worker_results(actor);
        drained = drained.saturating_add(1);
        if drained >= max_batch_messages {
            break;
        }
        next_msg = rx.try_recv().ok();
    }

    status::record_worker_queue_drain(queue_kind, drained);
    drained
}

fn spawn_tick_worker<A: WorkerActor + Send + 'static>(
    gate: Arc<ActorGate<A>>,
    active: Arc<AtomicUsize>,
    mut cfg: WorkerLoopConfig,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
) -> std::thread::JoinHandle<()> {
    sumeragi_thread_builder("sumeragi-tick")
        .spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                if shutdown_signal.is_sent() {
                    break;
                }
                let iter_start = Instant::now();
                active.fetch_add(1, Ordering::Relaxed);
                let (next_deadline, tick_gap, bypass_tick_gap) = {
                    let mut guard = gate.enter(GatePriority::Urgent);
                    status::set_worker_stage(status::WorkerLoopStage::Tick);
                    guard.actor_mut().refresh_worker_loop_config(&mut cfg);
                    let now = Instant::now();
                    poll_worker_results(guard.actor_mut());
                    let queue_depths = status::worker_queue_depth_snapshot();
                    let tick_gap = if has_pending_queue_depths(queue_depths) {
                        cfg.tick_busy_gap
                    } else {
                        cfg.tick_min_gap
                    };
                    let next_deadline = guard.actor_mut().next_tick_deadline(now);
                    if next_deadline.is_some_and(|deadline| deadline <= now)
                        && (guard.actor_mut().should_bypass_tick_gap()
                            || should_run_tick(now, last_tick, tick_gap))
                    {
                        if guard.actor_mut().tick() {
                            last_tick = now;
                        }
                        guard.actor_mut().sync_external_hints();
                    }
                    status::record_worker_iteration(
                        u64::try_from(iter_start.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                    (
                        next_deadline,
                        tick_gap,
                        guard.actor_mut().should_bypass_tick_gap(),
                    )
                };
                if active.fetch_sub(1, Ordering::Relaxed) == 1 {
                    status::set_worker_stage(status::WorkerLoopStage::Idle);
                }
                std::thread::yield_now();

                if shutdown_signal.is_sent() {
                    break;
                }
                let now = Instant::now();
                let wait = match next_deadline {
                    None => Some(Duration::from_millis(IDLE_SHUTDOWN_POLL_MS)),
                    Some(deadline) => {
                        let due_wait = deadline.saturating_duration_since(now);
                        let min_gap_wait = if bypass_tick_gap && deadline <= now {
                            None
                        } else {
                            idle_wait_duration(now, last_tick, tick_gap)
                        };
                        let mut wait =
                            min_gap_wait.map_or(due_wait, |min_gap| min_gap.max(due_wait));
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
                    while wake_rx.try_recv().is_ok() {}
                }
            }
        })
        .expect("sumeragi tick worker thread spawn failed")
}

#[allow(clippy::too_many_arguments)]
fn run_parallel_worker<A: WorkerActor + Send + 'static>(
    actor: A,
    cfg: WorkerLoopConfig,
    vote_rx: mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: mpsc::Receiver<InboundBlockMessage>,
    rbc_chunk_rx: mpsc::Receiver<InboundBlockMessage>,
    block_rx: mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: mpsc::Receiver<ControlFlow>,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    background_rx: mpsc::Receiver<BackgroundRequest>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
    max_urgent_before_da_critical: u32,
) {
    let gate = Arc::new(ActorGate::with_limits(actor, max_urgent_before_da_critical));
    let active = Arc::new(AtomicUsize::new(0));

    let mut joins = Vec::new();
    joins.push(spawn_queue_worker(
        "sumeragi-votes",
        vote_rx,
        Arc::clone(&gate),
        GatePriority::Urgent,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::Votes.stage(),
        PriorityTier::Votes.queue_kind(),
        "block_message",
        move |actor, msg| {
            if let BlockMessage::QcVote(vote) = &msg.message {
                iroha_logger::debug!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    queue = ?PriorityTier::Votes.queue_kind(),
                    "received precommit vote"
                );
            }
            actor.on_block_message(msg)
        },
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-rbc",
        rbc_chunk_rx,
        Arc::clone(&gate),
        GatePriority::Urgent,
        RBC_PARALLEL_BATCH_LIMIT,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::RbcChunks.stage(),
        PriorityTier::RbcChunks.queue_kind(),
        "block_message",
        move |actor, msg| actor.on_block_message(msg),
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-blocks",
        block_rx,
        Arc::clone(&gate),
        GatePriority::DaCritical,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::Blocks.stage(),
        PriorityTier::Blocks.queue_kind(),
        "block_message",
        move |actor, msg| actor.on_block_message(msg),
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-payloads",
        block_payload_rx,
        Arc::clone(&gate),
        GatePriority::DaCritical,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::BlockPayload.stage(),
        PriorityTier::BlockPayload.queue_kind(),
        "block_message",
        move |actor, msg| actor.on_block_message(msg),
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-consensus",
        consensus_rx,
        Arc::clone(&gate),
        GatePriority::Urgent,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::Consensus.stage(),
        PriorityTier::Consensus.queue_kind(),
        "consensus_control",
        move |actor, msg| actor.on_consensus_control(msg),
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-lane-relay",
        lane_relay_rx,
        Arc::clone(&gate),
        GatePriority::Urgent,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::LaneRelay.stage(),
        PriorityTier::LaneRelay.queue_kind(),
        "lane_relay",
        move |actor, msg| actor.on_lane_relay(msg),
    ));
    joins.push(spawn_queue_worker(
        "sumeragi-background",
        background_rx,
        Arc::clone(&gate),
        GatePriority::Regular,
        1,
        Arc::clone(&active),
        shutdown_signal.clone(),
        PriorityTier::Background.stage(),
        PriorityTier::Background.queue_kind(),
        "background",
        move |actor, msg| actor.on_background_request(msg),
    ));
    joins.push(spawn_tick_worker(
        Arc::clone(&gate),
        Arc::clone(&active),
        cfg,
        wake_rx,
        shutdown_signal,
    ));

    for join in joins {
        if let Err(err) = join.join() {
            iroha_logger::warn!(?err, "sumeragi worker thread exited with error");
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
            vote_drain_ms: 0,
            block_payload_drain_ms: 0,
            pre_tick_votes_drain_ms: 0,
            pre_tick_block_payload_drain_ms: 0,
            pre_tick_blocks_drain_ms: 0,
            post_tick_votes_drain_ms: 0,
            post_tick_block_payload_drain_ms: 0,
            post_tick_blocks_drain_ms: 0,
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

    fn warn_config(time_budget: Duration) -> WorkerLoopConfig {
        WorkerLoopConfig {
            time_budget,
            drain_budget_cap: Duration::from_secs(1),
            vote_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_budget: Duration::from_secs(1),
            block_payload_rx_drain_max_messages: 1,
            vote_rx_drain_max_messages: 1,
            vote_burst_cap_with_payload_backlog: VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG,
            block_rx_drain_budget: Duration::from_secs(1),
            block_rx_drain_max_messages: 1,
            rbc_chunk_rx_drain_budget: Duration::from_secs(1),
            rbc_chunk_rx_drain_max_messages: 1,
            consensus_rx_drain_max_messages: 1,
            lane_relay_rx_drain_max_messages: 1,
            background_rx_drain_max_messages: 1,
            tick_min_gap: Duration::from_millis(1),
            tick_busy_gap: Duration::from_millis(1),
            tick_max_gap: Duration::from_secs(1),
            block_rx_starve_max: Duration::from_secs(1),
            non_vote_starve_max: Duration::from_secs(1),
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

    #[test]
    fn slow_iteration_under_budget_does_not_warn_with_budget_gate() {
        let cfg = warn_config(Duration::from_secs(1));
        let mut stats = empty_stats();
        stats.queue_depths.vote_rx = 1;
        let iter_elapsed = Duration::from_millis(500);
        assert!(!should_warn_slow_iteration_with_budget(
            &cfg,
            &stats,
            iter_elapsed
        ));
    }

    #[test]
    fn slow_iteration_over_budget_warns_with_budget_gate() {
        let cfg = warn_config(Duration::from_millis(300));
        let mut stats = empty_stats();
        stats.queue_depths.vote_rx = 1;
        let iter_elapsed = Duration::from_millis(301);
        assert!(should_warn_slow_iteration_with_budget(
            &cfg,
            &stats,
            iter_elapsed
        ));
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
            frontier_block_sync_hint,
        } = self;
        let fallback_params = iroha_data_model::parameter::system::SumeragiParameters::default();
        let msg_channel_cap_block_payload = config.queues.block_payload;
        let msg_channel_cap_votes = config.queues.votes;
        let msg_channel_cap_blocks = config.queues.blocks;
        let msg_channel_cap_rbc_chunks = config.queues.rbc_chunks;
        let control_msg_channel_cap = config.queues.control;
        let worker_iteration_budget_cap = config.worker.iteration_budget_cap;
        let worker_iteration_drain_budget_cap = config.worker.iteration_drain_budget_cap;
        let vote_burst_cap_with_payload_backlog =
            config.worker.vote_burst_cap_with_payload_backlog.max(1);
        let max_urgent_before_da_critical = config.worker.max_urgent_before_da_critical.max(1);
        let (block_time, commit_time, da_enabled) = {
            let view = state.view();
            let params = view.world.parameters().sumeragi();
            let mode = effective_consensus_mode(&view, config.consensus_mode);
            let (block_time, commit_time) = match mode {
                ConsensusMode::Permissioned => resolve_sumeragi_timeouts(params, &fallback_params),
                ConsensusMode::Npos => {
                    let block_time = resolve_npos_block_time(&view);
                    let stage_commit = resolve_npos_timeouts(&view, &config.npos).commit;
                    // Keep worker/quorum budgets aligned with canonical Sumeragi commit timing.
                    let commit_time = stage_commit.max(params.effective_commit_time());
                    (block_time, commit_time)
                }
            };
            let da_enabled = params.da_enabled();
            (block_time, commit_time, da_enabled)
        };
        let da_quorum_timeout_multiplier = config.da.quorum_timeout_multiplier;
        let time_budget = worker_time_budget(
            block_time,
            commit_time,
            da_enabled,
            da_quorum_timeout_multiplier,
            worker_iteration_budget_cap,
        );
        let parallel_ingress = config.worker.parallel_ingress;
        let mut actor = match crate::sumeragi::main_loop::Actor::new_with_block_sync_hint(
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
            frontier_block_sync_hint,
            rbc_status_handle,
        ) {
            Ok(actor) => Box::new(actor),
            Err(err) => {
                iroha_logger::error!(?err, "Failed to initialise Sumeragi actor");
                return;
            }
        };
        let validation_worker_joins = actor.attach_validation_worker();
        let qc_verify_worker_joins = actor.attach_qc_verify_worker();
        let vote_verify_worker_joins = actor.attach_vote_verify_worker();
        let rbc_persist_worker_join: Option<std::thread::JoinHandle<()>> =
            actor.attach_rbc_persist_worker();
        let rbc_seed_worker_join: Option<std::thread::JoinHandle<()>> =
            actor.attach_rbc_seed_worker();
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
        let tick_busy_gap = busy_tick_gap(block_time, commit_time, tick_min_gap);
        let mut tick_max_gap = if block_time.is_zero() {
            time_budget
        } else {
            block_time.min(time_budget)
        };
        tick_max_gap = tick_max_gap.max(tick_min_gap);
        let loop_config = WorkerLoopConfig {
            time_budget,
            drain_budget_cap: worker_iteration_drain_budget_cap,
            vote_rx_drain_budget,
            block_payload_rx_drain_budget: non_vote_drain_budget,
            block_payload_rx_drain_max_messages: msg_channel_cap_block_payload.max(1),
            vote_rx_drain_max_messages: msg_channel_cap_votes.max(1),
            vote_burst_cap_with_payload_backlog,
            block_rx_drain_budget,
            block_rx_drain_max_messages: msg_channel_cap_blocks.max(1),
            rbc_chunk_rx_drain_budget: rbc_chunk_drain_budget,
            rbc_chunk_rx_drain_max_messages: msg_channel_cap_rbc_chunks.max(1),
            consensus_rx_drain_max_messages: control_msg_channel_cap.max(1),
            lane_relay_rx_drain_max_messages: control_msg_channel_cap.max(1),
            background_rx_drain_max_messages: control_msg_channel_cap.max(1),
            tick_min_gap,
            tick_busy_gap,
            tick_max_gap,
            block_rx_starve_max: starve_max,
            non_vote_starve_max: starve_max,
        };
        status::set_worker_stage(status::WorkerLoopStage::Idle);
        if parallel_ingress {
            run_parallel_worker(
                *actor,
                loop_config,
                vote_rx,
                block_payload_rx,
                rbc_chunk_rx,
                block_rx,
                consensus_rx,
                lane_relay_rx,
                background_rx,
                wake_rx,
                shutdown_signal,
                max_urgent_before_da_critical,
            );
        } else {
            let now = Instant::now();
            let loop_state = WorkerLoopState {
                last_tick: now,
                last_served: [now; PRIORITY_TIER_COUNT],
                mailbox: WorkerMailboxState::new(),
            };
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
        }
        for join in validation_worker_joins {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi validation worker thread exited with error");
            }
        }
        for join in qc_verify_worker_joins {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi QC verify worker thread exited with error");
            }
        }
        for join in vote_verify_worker_joins {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi vote verify worker thread exited with error");
            }
        }
        if let Some(join) = rbc_persist_worker_join {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi RBC persist worker thread exited with error");
            }
        }
        if let Some(join) = rbc_seed_worker_join {
            if let Err(err) = join.join() {
                iroha_logger::warn!(?err, "sumeragi RBC seed worker thread exited with error");
            }
        }
    }
}
