//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use eyre::Result;
use iroha_config::parameters::{
    actual::{Common as CommonConfig, Sumeragi as SumeragiConfig},
    defaults::{
        concurrency as concurrency_defaults,
        sumeragi::{
            BODY_ENVELOPE_HEADROOM_BYTES, CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            TIMEOUT_VOTE_RESERVE_BYTES, npos::EPOCH_LENGTH_BLOCKS,
        },
    },
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    block::consensus_v2::{ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode},
    consensus::VrfEpochRecord,
    merge::{
        MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES, MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES,
        MergeCommitteeSignature,
    },
    nexus::LaneRelayEnvelope,
    peer::PeerId,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, try_spawn_os_thread_as_future};
use iroha_genesis::GenesisBlock;
use iroha_p2p::network::{
    NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteSourceUpdate, NetworkReplyRoutes,
    NetworkReplyRoutesObservedMergeReceipt, NetworkReplyRoutesPruneReceipt,
    NetworkReplyRoutesStrictMergeReceipt,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;

use crate::{
    merge_sidecar::{CertifiedMergeSidecarMessage, MAX_CERTIFIED_MERGE_CHUNK_BYTES},
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};

static CONFIGURED_SUMERAGI_STACK_SIZE_BYTES: AtomicUsize = AtomicUsize::new(0);
const WORKER_WAKE_CHANNEL_CAP: usize = 1;
// The valid v2 timeout-vote envelope has at most 128 signers and two bounded BLS signatures.
// Keep this conservative ceiling aligned with the formal refinement and maximal fixture below;
// the production byte reserve is intentionally much larger.
const MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES: usize = 4 * 1024;
// Lane-owned completions fit the independently reviewed source bundle from
// which they are reconstructed. Canonical historical-body recovery is instead
// charged to the configured global transport-completion partition below.
const MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES: usize = MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES;
const MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES: usize = MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES;
const _: () = assert!(TIMEOUT_VOTE_RESERVE_BYTES >= MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES);
const _: () = assert!(iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES == 256);
type SumeragiThreadWork = Box<dyn FnOnce() + Send + 'static>;
type SumeragiThreadCompletion = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type SumeragiThreadSpawner =
    fn(std::thread::Builder, SumeragiThreadWork) -> std::io::Result<SumeragiThreadCompletion>;

fn normalized_sumeragi_stack_size_bytes(bytes: usize) -> Option<usize> {
    (concurrency_defaults::SUMERAGI_STACK_BYTES_MIN
        ..=concurrency_defaults::SUMERAGI_STACK_BYTES_MAX)
        .contains(&bytes)
        .then_some(bytes)
}

/// Override the stack size used for Sumeragi helper threads.
///
/// `irohad` applies this from the validated `concurrency.sumeragi_stack_bytes`
/// configuration before spawning consensus workers. Embedders that do not call
/// this setter receive the same deterministic configuration default.
pub fn set_sumeragi_stack_size_bytes(bytes: usize) {
    let bytes = normalized_sumeragi_stack_size_bytes(bytes)
        .unwrap_or(concurrency_defaults::SUMERAGI_STACK_BYTES);
    CONFIGURED_SUMERAGI_STACK_SIZE_BYTES.store(bytes, Ordering::Relaxed);
}

fn sumeragi_stack_size_bytes() -> usize {
    let configured = CONFIGURED_SUMERAGI_STACK_SIZE_BYTES.load(Ordering::Relaxed);
    normalized_sumeragi_stack_size_bytes(configured)
        .unwrap_or(concurrency_defaults::SUMERAGI_STACK_BYTES)
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

pub(crate) fn is_bls_normal_public_key(public_key: &PublicKey) -> bool {
    public_key
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}

#[cfg(test)]
mod thread_builder_tests {
    use std::sync::{Mutex, atomic::Ordering, mpsc};

    use iroha_crypto::KeyPair;

    use super::{
        Algorithm, CONFIGURED_SUMERAGI_STACK_SIZE_BYTES, concurrency_defaults,
        is_bls_normal_public_key, normalized_sumeragi_stack_size_bytes,
        set_sumeragi_stack_size_bytes, sumeragi_stack_size_bytes, sumeragi_thread_builder,
    };

    static STACK_CONFIG_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct RestoreSumeragiStackSize(usize);

    impl Drop for RestoreSumeragiStackSize {
        fn drop(&mut self) {
            CONFIGURED_SUMERAGI_STACK_SIZE_BYTES.store(self.0, Ordering::Relaxed);
        }
    }

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

    #[test]
    fn sumeragi_stack_size_is_bounded() {
        assert_eq!(
            normalized_sumeragi_stack_size_bytes(concurrency_defaults::SUMERAGI_STACK_BYTES_MIN),
            Some(concurrency_defaults::SUMERAGI_STACK_BYTES_MIN)
        );
        assert_eq!(
            normalized_sumeragi_stack_size_bytes(concurrency_defaults::SUMERAGI_STACK_BYTES_MAX),
            Some(concurrency_defaults::SUMERAGI_STACK_BYTES_MAX)
        );
        assert_eq!(
            normalized_sumeragi_stack_size_bytes(
                concurrency_defaults::SUMERAGI_STACK_BYTES_MIN - 1
            ),
            None
        );
        assert_eq!(
            normalized_sumeragi_stack_size_bytes(
                concurrency_defaults::SUMERAGI_STACK_BYTES_MAX + 1
            ),
            None
        );
    }

    #[test]
    fn sumeragi_stack_size_uses_configured_value() {
        let _guard = STACK_CONFIG_TEST_LOCK.lock().expect("stack test lock");
        let previous = CONFIGURED_SUMERAGI_STACK_SIZE_BYTES.swap(0, Ordering::Relaxed);
        let _restore = RestoreSumeragiStackSize(previous);

        set_sumeragi_stack_size_bytes(concurrency_defaults::SUMERAGI_STACK_BYTES_MIN);
        assert_eq!(
            sumeragi_stack_size_bytes(),
            concurrency_defaults::SUMERAGI_STACK_BYTES_MIN
        );

        set_sumeragi_stack_size_bytes(usize::MAX);
        assert_eq!(
            sumeragi_stack_size_bytes(),
            concurrency_defaults::SUMERAGI_STACK_BYTES
        );
    }

    #[test]
    fn bls_normal_public_key_check_uses_checked_algorithm_access() {
        let bls_key = KeyPair::try_from_seed(b"checked-bls-key".to_vec(), Algorithm::BlsNormal)
            .expect("derive BLS fixture key");
        let ed25519_key =
            KeyPair::try_from_seed(b"checked-ed25519-key".to_vec(), Algorithm::Ed25519)
                .expect("derive Ed25519 fixture key");

        assert!(is_bls_normal_public_key(bls_key.public_key()));
        assert!(!is_bls_normal_public_key(ed25519_key.public_key()));
    }
}

/// Build the initial validator topology from trusted peers.
/// Enforces BLS-normal keys and, when configured with a `PoP` map, treats peers
/// with valid PoPs as the validator subset. BLS trusted peers without a PoP are
/// kept as network-trusted peers but are not included in consensus.
pub fn filter_validators_from_trusted(
    tp: &iroha_config::parameters::actual::TrustedPeers,
) -> Vec<PeerId> {
    let mut baseline: BTreeSet<PeerId> = BTreeSet::new();
    let iter = std::iter::once(tp.myself.clone()).chain(tp.others.clone());
    for peer in iter {
        let pk = peer.id().public_key();
        if !is_bls_normal_public_key(pk) {
            iroha_logger::warn!(?pk, "excluding peer: validator identity must be BLS-normal");
            continue;
        }
        baseline.insert(PeerId::new(pk.clone()));
    }

    let mut out = if tp.pops.is_empty() {
        baseline.clone()
    } else {
        let mut filtered: BTreeSet<PeerId> = BTreeSet::new();
        let mut missing = 0usize;
        for peer_id in &baseline {
            let pk = peer_id.public_key();
            let Some(pop) = tp.pops.get(pk) else {
                missing = missing.saturating_add(1);
                continue;
            };
            if let Err(e) = iroha_crypto::bls_normal_pop_verify(pk, pop) {
                iroha_logger::warn!(?pk, ?e, "invalid PoP; excluding peer from consensus");
                continue;
            }
            filtered.insert(peer_id.clone());
        }
        if missing > 0 {
            iroha_logger::info!(
                missing,
                baseline = baseline.len(),
                pops = tp.pops.len(),
                validators = filtered.len(),
                "excluding trusted peers without validator PoPs from consensus roster"
            );
        }
        filtered
    };

    // If PoP filtering leaves the bootstrap roster empty but the configuration
    // still includes PoP records, fall back to those so startup does not silently
    // collapse into a single-node topology when addresses were omitted.
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

/// Return the caller's genesis/height-context selected mode for a height.
pub fn effective_consensus_mode_for_height(
    view: &StateView<'_>,
    height: u64,
    frozen_mode: ConsensusMode,
) -> ConsensusMode {
    effective_consensus_mode_for_height_from_world(view.world(), height, frozen_mode)
}

/// Return the already frozen consensus mode for a specific height.
///
/// Runtime mode staging was retired by protocol v2. Callers must supply the
/// genesis/height-context mode as `frozen_mode`; mutable world parameters and the
/// queried height cannot change it.
pub fn effective_consensus_mode_for_height_from_world(
    _world: &impl WorldReadOnly,
    _height: u64,
    frozen_mode: ConsensusMode,
) -> ConsensusMode {
    frozen_mode
}

/// Return the caller's genesis/height-context selected consensus mode.
pub fn effective_consensus_mode(view: &StateView<'_>, frozen_mode: ConsensusMode) -> ConsensusMode {
    let height = u64::try_from(view.height()).unwrap_or(0);
    effective_consensus_mode_for_height(view, height, frozen_mode)
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
            .map(|params| params.epoch_length_blocks().get())
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
}

/// Resolve the signed on-chain delay before consensus-evidence penalties apply.
pub(crate) fn resolve_npos_slashing_delay_blocks_from_world(
    world: &impl WorldReadOnly,
) -> Option<u64> {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.slashing_delay_blocks())
}

fn chain_epoch_seed(chain_id: &ChainId) -> [u8; 32] {
    let chain = chain_id.clone().into_inner();
    let hash = CryptoHash::new(chain.as_bytes());
    <[u8; 32]>::from(hash)
}

fn npos_base_epoch_seed(world: &impl WorldReadOnly, chain_id: &ChainId) -> [u8; 32] {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.epoch_seed())
        .unwrap_or_else(|| chain_epoch_seed(chain_id))
}

fn next_epoch_seed_from_seed_and_reveals(
    seed: [u8; 32],
    reveals: impl IntoIterator<Item = (u32, [u8; 32])>,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let mut h = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut h, &seed);
    for (signer, reveal) in reveals {
        iroha_crypto::blake2::digest::Update::update(&mut h, &signer.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut h, &reveal);
    }
    let digest = iroha_crypto::blake2::Digest::finalize(h);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn next_epoch_seed_from_seed(seed: [u8; 32]) -> [u8; 32] {
    next_epoch_seed_from_seed_and_reveals(seed, [])
}

fn next_epoch_seed_from_record(record: &VrfEpochRecord) -> [u8; 32] {
    let mut reveals: Vec<(u32, [u8; 32])> = record
        .participants
        .iter()
        .filter_map(|p| p.reveal.map(|reveal| (p.signer, reveal)))
        .collect();
    reveals.sort_by_key(|(signer, _)| *signer);
    next_epoch_seed_from_seed_and_reveals(record.seed, reveals)
}

pub(crate) fn deterministic_npos_seed_for_epoch_from_world(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    epoch: u64,
) -> [u8; 32] {
    let mut seed = npos_base_epoch_seed(world, chain_id);
    for _ in 0..epoch {
        seed = next_epoch_seed_from_seed(seed);
    }
    seed
}

fn latest_epoch_seed_from_world(world: &impl WorldReadOnly, chain_id: &ChainId) -> [u8; 32] {
    if let Some((_epoch, record)) = world.vrf_epochs().iter().last() {
        return if record.finalized {
            next_epoch_seed_from_record(record)
        } else {
            record.seed
        };
    }
    npos_base_epoch_seed(world, chain_id)
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
    let epoch = epoch_for_height_from_world(world, height);
    npos_seed_for_epoch_from_world(world, chain_id, epoch)
}

/// Resolve the `NPoS` PRF seed for one exact authenticated epoch.
///
/// Callers that already carry a finalized epoch number must use this rather
/// than re-deriving an epoch from a possibly different height schedule.
pub(crate) fn npos_seed_for_epoch_from_world(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    epoch: u64,
) -> [u8; 32] {
    if let Some(record) = world.vrf_epochs().get(&epoch) {
        return record.seed;
    }
    if let Some((last_epoch, record)) = world
        .vrf_epochs()
        .iter()
        .filter(|(record_epoch, record)| **record_epoch < epoch && record.finalized)
        .last()
    {
        // Crash recovery: derive missing epoch seeds if in-progress seed-only snapshots
        // were not persisted before restart.
        let mut seed = next_epoch_seed_from_record(record);
        for _ in last_epoch.saturating_add(1)..epoch {
            seed = next_epoch_seed_from_seed(seed);
        }
        return seed;
    }
    if world.sumeragi_npos_parameters().is_some() {
        deterministic_npos_seed_for_epoch_from_world(world, chain_id, epoch)
    } else {
        latest_epoch_seed_from_world(world, chain_id)
    }
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
mod exact_epoch_seed_tests {
    use iroha_data_model::parameter::{Parameter, system::SumeragiNposParameters};

    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};

    #[test]
    fn exact_authenticated_epoch_seed_is_not_rederived_from_height() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let authenticated_seed = [0xA7; 32];
        {
            let mut parameters = state.world.parameters.block();
            parameters.set_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ));
            parameters.commit();
        }
        {
            let mut world = state.world.block();
            world.vrf_epochs.insert(
                7,
                VrfEpochRecord {
                    epoch: 7,
                    seed: authenticated_seed,
                    epoch_length: 100,
                    commit_deadline_offset: 1,
                    reveal_deadline_offset: 2,
                    roster_len: 0,
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
            world.commit();
        }

        let view = state.world_view();
        assert_eq!(epoch_for_height_from_world(&view, 2), 0);
        assert_ne!(
            npos_seed_for_height_from_world(&view, state.chain_id_ref(), 2),
            authenticated_seed
        );
        assert_eq!(
            npos_seed_for_epoch_from_world(&view, state.chain_id_ref(), 7),
            authenticated_seed,
            "a parent-authenticated epoch number is authoritative"
        );
    }
}

/// QC-based consensus message types and helpers (single-chain).
pub mod consensus;
pub mod da;
pub mod epoch_report;
pub(crate) mod evidence;
pub(crate) mod exec;
pub(crate) mod lane_planner;
pub mod message;
pub mod network_topology;
pub(crate) mod output_guard;
pub(crate) mod penalties;
pub(crate) mod safety_wal;
pub(crate) mod serviced_candidate_store;
pub(crate) mod smt;
pub(crate) mod stake_snapshot;
pub mod status;
pub(crate) mod v2;
pub(crate) mod v2_apply;
pub(crate) mod v2_block_sync;
pub(crate) mod v2_body_store;
pub(crate) mod v2_candidate;
pub(crate) mod v2_chunks;
pub(crate) mod v2_context;
pub(crate) mod v2_context_store;
pub(crate) mod v2_core;
pub use v2_context::{
    GenesisV2Bootstrap, V2GenesisBootstrapError, freeze_staged_genesis_v2,
    signed_genesis_voting_peers, staged_genesis_execution_policy_hash,
    staged_genesis_nexus_amx_context_hash, validate_signed_genesis_v2_authority,
};
pub use v2_core::{
    CheckedProductionTransition, ProductionTwoStageRelayRetryTraceProjection,
    check_production_two_stage_relay_retry_transition,
    production_two_stage_relay_retry_trace_refines_source_fairness_kernel,
};
pub(crate) mod v2_effects;
pub(crate) mod v2_lane_work;
pub(crate) mod v2_lifecycle_recovery;
pub(crate) mod v2_npos;
pub(crate) mod v2_recovery;
pub use v2_recovery::{
    AuthenticatedV2SnapshotStartup, V2StartupReplayError, V2StartupReplayPlan,
    authenticate_v2_snapshot_replay_boundary, authenticate_v2_snapshot_startup,
    authenticated_v2_snapshot_startup_mode, plan_v2_startup_replay,
};
pub(crate) mod v2_runner;
pub(crate) mod v2_runtime;
pub(crate) mod v2_transport;
pub(crate) mod v2_worker;
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

/// Public snapshot of leader index and `HighestQC` tuple for status endpoints.
pub use status::StatusSnapshot;

/// Return the latest consensus status snapshot (leader, QCs, drop counters).
pub fn status_snapshot() -> StatusSnapshot {
    status::snapshot()
}

#[cfg(not(test))]
use self::output_guard::process_consensus_output_guard;
use self::{message::*, output_guard::ConsensusOutputGuard};
use crate::{EventsSender, IrohaNetwork, kura::Kura, queue::Queue};

/// Bundle of genesis block and its publishing key.
#[derive(Clone)]
pub struct GenesisWithPubKey {
    /// Optional genesis block to seed the chain; `None` when submitted elsewhere.
    pub genesis: Option<GenesisBlock>,
    /// Public key used to sign the genesis payload.
    pub public_key: PublicKey,
    /// Signed, authenticated cadence frozen for this consensus process.
    ///
    /// Fresh startup must not read this value from the pre-genesis world, whose
    /// placeholder parameters have not yet been replaced by signed genesis.
    pub block_cadence: Duration,
    /// Verified, uncommitted height-one context derived from fresh genesis.
    /// Absent only on the preserved non-empty-storage restart path.
    pub v2_bootstrap: Option<GenesisV2Bootstrap>,
}

/// Authenticated lane-local traffic accepted alongside global v2 consensus.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum LaneRelayMessage {
    /// Lane settlement envelope admitted from the authenticated relay.
    Envelope(LaneRelayEnvelope),
    /// Merge-committee signature share admitted from the authenticated relay.
    MergeSignature(MergeCommitteeSignature),
    /// Certified merge-sidecar request or chunk with its transport sender.
    CertifiedMergeSidecar {
        /// Semantic protocol origin.
        sender: PeerId,
        /// Exact authenticated return route for request-induced responses.
        reply_route: Option<NetworkReplyRoute>,
        /// Certified sidecar protocol message.
        message: CertifiedMergeSidecarMessage,
    },
    /// Native AMX control message with its authenticated transport sender.
    NativeAmx {
        /// Semantic protocol origin.
        sender: PeerId,
        /// Exact authenticated return route for request-induced votes.
        reply_route: Option<NetworkReplyRoute>,
        /// Native AMX protocol message.
        message: crate::native_amx::NativeAmxMessage,
    },
    /// Lane-drain vote with its authenticated transport identity.
    DrainVote {
        /// Semantic protocol origin authenticated by transport.
        sender: PeerId,
        /// Exact committee-signed drain frontier vote.
        vote: crate::lane_consensus::LaneDrainVoteV1,
    },
}

/// One normalized live-consensus message plus its protocol and transport identities.
#[derive(Clone, Debug)]
pub struct InboundBlockMessage {
    message: BlockMessage,
    /// Semantic protocol origin used for validation and response routing.
    sender: Option<PeerId>,
    /// Authenticated transport hop used exclusively for resource isolation.
    via: Option<PeerId>,
    /// Exact authenticated route which may carry a response to this occurrence.
    reply_routes: Option<NetworkReplyRoutes>,
    /// Process-local exact ownership evidence retained across fair admission.
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
}

impl InboundBlockMessage {
    /// Normalize one direct or synthetic message.
    ///
    /// Direct messages use the same identity as their semantic sender and
    /// authenticated transport source. Synthetic messages leave both unset.
    pub fn new(message: BlockMessage, sender: Option<PeerId>) -> Self {
        Self {
            message: message.normalize(),
            via: sender.clone(),
            sender,
            reply_routes: None,
            ingress_ownership: None,
        }
    }

    /// Normalize one transport message while preserving a relayed protocol origin.
    ///
    /// `sender` remains visible to consensus validation and response routing;
    /// `via` is the authenticated hop charged for every bounded ingress owner.
    pub fn from_transport(message: BlockMessage, sender: PeerId, via: PeerId) -> Self {
        Self {
            message: message.normalize(),
            sender: Some(sender),
            via: Some(via),
            reply_routes: None,
            ingress_ownership: None,
        }
    }

    /// Normalize one transport message and retain its exact authenticated return route.
    ///
    /// # Errors
    ///
    /// Returns the precise route-capability error when the route is inactive,
    /// addresses another semantic sender, belongs to another authenticated
    /// delivery peer, or cannot form a bounded route set.
    pub fn try_from_transport_with_reply_route(
        message: BlockMessage,
        sender: PeerId,
        via: PeerId,
        reply_route: NetworkReplyRoute,
    ) -> Result<Self, NetworkReplyRouteError> {
        if reply_route.semantic_target() != &sender {
            return Err(NetworkReplyRouteError::Retargeted);
        }
        if !reply_route.is_authenticated_via(&via) {
            return Err(NetworkReplyRouteError::DifferentSource);
        }
        let reply_routes = NetworkReplyRoutes::try_from_route(reply_route)?;
        Ok(Self {
            message: message.normalize(),
            sender: Some(sender),
            via: Some(via),
            reply_routes: Some(reply_routes),
            ingress_ownership: None,
        })
    }

    /// Consume the envelope and return the normalized message and semantic origin.
    #[cfg(test)]
    pub(crate) fn into_message_and_sender(self) -> (BlockMessage, Option<PeerId>) {
        (self.message, self.sender)
    }

    /// Consume the envelope without losing its local-only authenticated reply authority.
    pub(crate) fn into_message_sender_and_reply_routes(
        self,
    ) -> (BlockMessage, Option<PeerId>, Option<NetworkReplyRoutes>) {
        (self.message, self.sender, self.reply_routes)
    }

    /// Borrow the bounded fair-ingress ownership carrier attached at
    /// `FairV2Ingress::try_push_at`.
    pub(crate) const fn ingress_ownership(&self) -> Option<&FairV2IngressOwnershipEvidence> {
        self.ingress_ownership.as_ref()
    }

    /// Move the exact fair-ingress ownership carrier into the downstream
    /// runtime-admission bridge without exposing or serializing capabilities.
    pub(crate) fn take_ingress_ownership(&mut self) -> Option<FairV2IngressOwnershipEvidence> {
        self.ingress_ownership.take()
    }

    /// Borrow the normalized message without removing it from its ingress lane.
    ///
    /// The serialized runner uses this view to make downstream admission and
    /// fair-ingress removal one atomic operation.
    pub(crate) fn message(&self) -> &BlockMessage {
        &self.message
    }

    /// Borrow the semantic protocol origin, when one was supplied.
    pub(crate) fn sender(&self) -> Option<&PeerId> {
        self.sender.as_ref()
    }

    /// Borrow the exact authenticated return-route set before fair removal.
    ///
    /// The serialized v2 runner uses this only to validate and reserve an
    /// exact certified-body Serve lifecycle while this ingress owner is still
    /// locked in its source lane.
    pub(crate) fn reply_routes(&self) -> Option<&NetworkReplyRoutes> {
        self.reply_routes.as_ref()
    }

    /// Borrow the authenticated transport hop used for resource isolation.
    pub(crate) fn via(&self) -> Option<&PeerId> {
        self.via.as_ref()
    }
}

/// Ownership-aware result of one non-blocking Sumeragi ingress attempt.
#[derive(Debug)]
#[must_use = "retryable and fail-stop dispositions retain the exact ingress item"]
pub enum SumeragiIngressDisposition<T> {
    /// The serialized Sumeragi owner accepted the exact item.
    Accepted,
    /// An identical queued item from the same source already owns delivery.
    Coalesced,
    /// The item belongs to a retired or decode-only protocol surface.
    Obsolete,
    /// The exact item is permanently invalid for the active ingress geometry.
    Rejected(T),
    /// Capacity or height readiness is temporary; retry this exact item.
    Retry(T),
    /// The serialized ingress channel disconnected permanently.
    Closed(T),
    /// Consensus has entered process-lifetime fail-stop mode.
    FailStop(T),
}

impl<T> SumeragiIngressDisposition<T> {
    fn accepted_or_coalesced(&self) -> bool {
        matches!(self, Self::Accepted | Self::Coalesced)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FairV2IngressSource {
    Validator(PeerId),
    Authenticated(PeerId),
    Anonymous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressSourceClass {
    Validator,
    Authenticated,
    Anonymous,
}

impl FairV2IngressSource {
    const fn class(&self) -> FairV2IngressSourceClass {
        match self {
            Self::Validator(_) => FairV2IngressSourceClass::Validator,
            Self::Authenticated(_) => FairV2IngressSourceClass::Authenticated,
            Self::Anonymous => FairV2IngressSourceClass::Anonymous,
        }
    }
}

struct FairV2IngressState {
    roster: BTreeSet<PeerId>,
    lanes: BTreeMap<FairV2IngressSource, FairV2IngressLane>,
    /// Canonical semantic request identity mapped to its first owning source lane.
    pending_wire_owners: BTreeMap<FairV2IngressWireKey, FairV2IngressSource>,
    /// Bounded receiver-local lifecycle for productive v2 leader wires.
    ///
    /// Slots are roster-source/kind/chunk addresses. Subject is part of the
    /// immutable identity, not the slot, so a Byzantine source cannot grow an
    /// unbounded tombstone table by rotating subjects. A terminal owner may be
    /// replaced only by a strictly newer view in the same height context.
    leader_wire_lifecycles: BTreeMap<FairV2IngressLeaderWireSlot, FairV2IngressLeaderWireRecord>,
    /// Frozen chunk-index geometry for the configured height.
    leader_wire_max_chunk_count: u32,
    /// Last immutable occurrence ordinal assigned by this ingress instance.
    ///
    /// This deliberately survives roster reconfiguration so rollover never
    /// reuses a process-local occurrence position.
    last_admission_ordinal: u64,
    ready: VecDeque<FairV2IngressSource>,
    len: usize,
    bytes: usize,
    nonempty_since: Option<Instant>,
    last_service_attempt_at: Option<Instant>,
    required_ordinary_bytes: usize,
    required_certified_fence_escape_bytes: usize,
    required_transport_completion_bytes: usize,
    required_consensus_frame_bytes: usize,
    required_control_frame_bytes: usize,
    required_block_sync_frame_bytes: usize,
    required_outbound_high_frame_bytes: usize,
    requires_certified_serve_gate: bool,
    certified_serve_gate: Option<v2_worker::CertifiedServeIngressGate>,
    requires_leader_wire_lifecycle_gate: bool,
    leader_wire_lifecycle_gate: Option<Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>>,
    leader_wire_lifecycle_ordinals: Option<v2_runtime::RuntimeLifecycleOrdinalSource>,
    leader_wire_context: Option<(
        iroha_data_model::block::consensus_v2::HeightContextId,
        iroha_data_model::block::consensus_v2::Height,
    )>,
    open: bool,
}

#[derive(Default)]
struct FairV2IngressLane {
    entries: VecDeque<FairV2IngressEntry>,
    pending_wire: BTreeSet<FairV2IngressWireKey>,
    progress_len: usize,
    certified_fence_escape_len: usize,
    timeout_vote_len: usize,
    transport_completion_len: usize,
    bytes: usize,
    certified_fence_escape_bytes: usize,
    timeout_vote_bytes: usize,
    transport_completion_bytes: usize,
}

struct FairV2IngressEntry {
    /// Immutable service snapshot shared outside `state` while one consumer
    /// evaluates admission. Duplicate coalescing uses `Arc::make_mut`, so the
    /// queued owner remains independently mutable without invalidating that
    /// snapshot.
    inbound: Arc<InboundBlockMessage>,
    enqueued_at: Instant,
    admission_ordinal: u64,
    certified_serve_reservation: Option<v2_worker::CertifiedServeIngressReservation>,
    class: FairV2IngressClass,
    wire_key: Option<FairV2IngressWireKey>,
    leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    encoded_bytes: Arc<[u8]>,
    encoded_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FairV2IngressWireKey {
    origin: Option<PeerId>,
    hash: CryptoHash,
}

/// Closed productive v2 ingress class carried only in node-local metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum FairV2IngressLeaderWireSourceClass {
    /// Proposal, vote, certificate, or timeout control.
    Control,
    /// One manifest-bound data-availability chunk.
    Chunk,
    /// One authenticated certified-body response.
    CertifiedResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum FairV2IngressLeaderWirePhase {
    Proposal,
    PrepareVote,
    CommitVote,
    PrepareQc,
    CommitQc,
    TimeoutVote,
    TimeoutCertificate,
    Chunk,
    CertifiedResponse,
}

impl FairV2IngressLeaderWirePhase {
    const fn source_class(self) -> FairV2IngressLeaderWireSourceClass {
        match self {
            Self::Proposal
            | Self::PrepareVote
            | Self::CommitVote
            | Self::PrepareQc
            | Self::CommitQc
            | Self::TimeoutVote
            | Self::TimeoutCertificate => FairV2IngressLeaderWireSourceClass::Control,
            Self::Chunk => FairV2IngressLeaderWireSourceClass::Chunk,
            Self::CertifiedResponse => FairV2IngressLeaderWireSourceClass::CertifiedResponse,
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Proposal => 0,
            Self::PrepareVote => 1,
            Self::CommitVote => 2,
            Self::PrepareQc => 3,
            Self::CommitQc => 4,
            Self::TimeoutVote => 5,
            Self::TimeoutCertificate => 6,
            Self::Chunk => 7,
            Self::CertifiedResponse => 8,
        }
    }
}

/// Finite semantic owner address for one productive leader wire.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireSlot {
    semantic_origin: PeerId,
    phase: FairV2IngressLeaderWirePhase,
    chunk_index: Option<u32>,
}

/// Full immutable identity retained across queue, runtime, and durable cuts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireIdentity {
    context_id: iroha_data_model::block::consensus_v2::HeightContextId,
    height: iroha_data_model::block::consensus_v2::Height,
    view: iroha_data_model::block::consensus_v2::View,
    subject_hash: CryptoHash,
    manifest_hash: Option<CryptoHash>,
    phase: FairV2IngressLeaderWirePhase,
    semantic_origin: PeerId,
    canonical_wire_hash: CryptoHash,
}

impl FairV2IngressLeaderWireIdentity {
    /// Stable route-neutral projection persisted by the downstream owner.
    pub(crate) fn projection_hash(&self) -> CryptoHash {
        let mut projection = Vec::new();
        projection.extend_from_slice(b"iroha:sumeragi:v2:leader-wire-lifecycle:v1");
        let context = self.context_id.encode();
        projection.extend_from_slice(
            &u64::try_from(context.len())
                .expect("height-context identity length fits u64")
                .to_le_bytes(),
        );
        projection.extend_from_slice(&context);
        projection.extend_from_slice(&self.height.to_le_bytes());
        projection.extend_from_slice(&self.view.to_le_bytes());
        projection.extend_from_slice(self.subject_hash.as_ref());
        projection.push(self.phase.code());
        let origin = self.semantic_origin.encode();
        projection.extend_from_slice(
            &u64::try_from(origin.len())
                .expect("semantic-origin identity length fits u64")
                .to_le_bytes(),
        );
        projection.extend_from_slice(&origin);
        match self.manifest_hash {
            None => projection.push(0),
            Some(hash) => {
                projection.push(1);
                projection.extend_from_slice(hash.as_ref());
            }
        }
        projection.extend_from_slice(self.canonical_wire_hash.as_ref());
        CryptoHash::new(projection)
    }
}

/// Exact internal reservation token attached to fair-ingress ownership.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireToken {
    identity: FairV2IngressLeaderWireIdentity,
    slot: FairV2IngressLeaderWireSlot,
    /// Immutable first-reservation position for this logical lifecycle.
    ///
    /// A restart retry retains this identity ordinal while its new physical
    /// fair-ingress carrier receives a fresh `FairV2IngressEntry` ordinal.
    admission_ordinal: u64,
    /// Actor-global producer/runtime scheduler position.
    scheduler_ordinal: u128,
    source_class: FairV2IngressLeaderWireSourceClass,
}

impl FairV2IngressLeaderWireToken {
    /// Stable route-neutral identity used by durable consumer receipts.
    pub(crate) fn identity_hash(&self) -> CryptoHash {
        self.identity.projection_hash()
    }

    /// Immutable first reservation ordinal.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn admission_ordinal(&self) -> u64 {
        self.admission_ordinal
    }

    /// Immutable shared scheduler position retained through restart.
    pub(crate) const fn scheduler_ordinal(&self) -> u128 {
        self.scheduler_ordinal
    }

    /// Proposal view retained by this exact productive wire.
    pub(crate) const fn view(&self) -> iroha_data_model::block::consensus_v2::View {
        self.identity.view
    }

    /// Whether this token is the exact chunk lifecycle for one manifest hash.
    pub(crate) fn matches_chunk_manifest(
        &self,
        manifest_hash: HashOf<iroha_data_model::block::consensus_v2::PayloadManifest>,
    ) -> bool {
        self.identity.phase == FairV2IngressLeaderWirePhase::Chunk
            && self.source_class == FairV2IngressLeaderWireSourceClass::Chunk
            && self.identity.manifest_hash == Some(manifest_hash.into())
    }

    /// Whether this chunk token names the exact proposal coordinates.
    pub(crate) fn matches_body_coordinates(
        &self,
        round: iroha_data_model::block::consensus_v2::ConsensusRound,
        subject: iroha_data_model::block::consensus_v2::BlockSubject,
    ) -> bool {
        self.identity.phase == FairV2IngressLeaderWirePhase::Chunk
            && self.source_class == FairV2IngressLeaderWireSourceClass::Chunk
            && self.identity.context_id == round.context_id
            && self.identity.height == round.height
            && self.identity.view == round.view
            && self.identity.subject_hash == fair_v2_ingress_subject_hash(Some(&subject))
    }

    /// Whether this chunk token names one exact proposal body.
    pub(crate) fn matches_exact_body(
        &self,
        round: iroha_data_model::block::consensus_v2::ConsensusRound,
        subject: iroha_data_model::block::consensus_v2::BlockSubject,
        manifest_hash: HashOf<iroha_data_model::block::consensus_v2::PayloadManifest>,
    ) -> bool {
        self.matches_body_coordinates(round, subject) && self.matches_chunk_manifest(manifest_hash)
    }

    /// Validate the complete context-bound token against configured geometry.
    pub(crate) fn validate_exact(
        &self,
        context_id: iroha_data_model::block::consensus_v2::HeightContextId,
        height: iroha_data_model::block::consensus_v2::Height,
        roster: &BTreeSet<PeerId>,
        max_chunk_count: u32,
    ) -> bool {
        let manifest_shape_exact = match self.identity.phase {
            FairV2IngressLeaderWirePhase::Proposal
            | FairV2IngressLeaderWirePhase::CertifiedResponse => {
                self.identity.manifest_hash.is_some() && self.slot.chunk_index.is_none()
            }
            FairV2IngressLeaderWirePhase::Chunk => {
                self.identity.manifest_hash.is_some()
                    && self
                        .slot
                        .chunk_index
                        .is_some_and(|index| index < max_chunk_count)
            }
            FairV2IngressLeaderWirePhase::PrepareVote
            | FairV2IngressLeaderWirePhase::CommitVote
            | FairV2IngressLeaderWirePhase::PrepareQc
            | FairV2IngressLeaderWirePhase::CommitQc
            | FairV2IngressLeaderWirePhase::TimeoutVote
            | FairV2IngressLeaderWirePhase::TimeoutCertificate => {
                self.identity.manifest_hash.is_none() && self.slot.chunk_index.is_none()
            }
        };
        self.admission_ordinal != 0
            && self.scheduler_ordinal != 0
            && self.identity.context_id == context_id
            && self.identity.height == height
            && roster.contains(&self.identity.semantic_origin)
            && self.slot.semantic_origin == self.identity.semantic_origin
            && self.slot.phase == self.identity.phase
            && self.source_class == self.identity.phase.source_class()
            && manifest_shape_exact
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressLeaderWireStatus {
    /// Restart-restored durable owner with no surviving physical carrier.
    ///
    /// The exact retry retains this record's token, but the record does not
    /// enter the global selector until that packet passes current capacity
    /// checks and the durable gate marks it Ingress. A later WAL-durable view
    /// or Decision cut instead retires a view-scoped owner without waiting for
    /// that retry. Request-bound certified-body recovery survives both cuts.
    Dormant,
    Ingress,
    Runtime,
    /// Consumer retirement whose evidence is intentionally reopened on crash.
    VolatileTerminal,
    Terminal,
}

impl FairV2IngressLeaderWireStatus {
    const fn blocks_replacement(self) -> bool {
        matches!(self, Self::Dormant | Self::Ingress | Self::Runtime)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FairV2IngressLeaderWireRecord {
    token: FairV2IngressLeaderWireToken,
    status: FairV2IngressLeaderWireStatus,
    /// Runtime owner retained by a reopened pre-crash lifecycle, if any.
    restored_runtime_owner: Option<serviced_candidate_store::LeaderWireRuntimeOwner>,
    /// Number of pre-carrier physical owners remaining in every source.
    ingress_predecessors: BTreeMap<FairV2IngressSource, usize>,
}

#[derive(Clone)]
enum FairV2IngressLeaderWireDerivation {
    /// The message belongs to a separate bounded producer protocol.
    NotApplicable,
    /// The exact immutable leader-wire identity and its bounded owner slot.
    Exact {
        identity: FairV2IngressLeaderWireIdentity,
        slot: FairV2IngressLeaderWireSlot,
    },
    /// A reordered chunk has no retained Proposal coordinates yet.
    ///
    /// It is a bounded proofless producer episode, not an exact rank owner:
    /// the worker's manifest/source/index orphan lifecycle retains the bytes
    /// and coalesces retries until Proposal processing supplies the immutable
    /// round and subject. Giving this unauthenticated-against-manifest packet
    /// a global scheduler barrier would let a Byzantine roster peer reserve
    /// that barrier forever by withholding the matching Proposal.
    UnboundChunk,
    /// The wire cannot belong to the frozen height geometry.
    Reject,
}

enum FairV2IngressLeaderWireAdmission {
    NotApplicable,
    Coalesced,
    Admitted(FairV2IngressLeaderWireToken),
    /// No live selector owner exists for this exact slot yet. The slot is
    /// either wholly vacant or replay-dormant without a physical carrier. The
    /// caller must first prove physical ingress capacity, then repeat the
    /// operation with `publish_ingress = true` while still holding the ingress
    /// lock.
    Ready,
}

enum FairV2IngressLeaderWireAdmissionError {
    Busy,
    Exhausted,
    Rejected,
}

fn fair_v2_ingress_leader_wire_lifecycle_capacity(
    roster_len: usize,
    max_chunk_count: u32,
) -> Option<usize> {
    const NON_CHUNK_PHASES: usize = 8;
    let phases_per_origin = usize::try_from(max_chunk_count)
        .ok()?
        .checked_add(NON_CHUNK_PHASES)?;
    roster_len.checked_mul(phases_per_origin)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressClass {
    Auxiliary,
    Progress,
    TransportCompletion,
}

/// Exact action applied to one canonical fair-ingress semantic owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FairV2IngressOwnershipAction {
    /// A new semantic request acquired its first bounded queue owner.
    New,
    /// The exact same authenticated delivery was observed again.
    ExactDuplicate,
    /// The same source delivered a later occurrence on the retained tenure.
    SameSourceLaterDelivery,
    /// The same source delivered a later occurrence on a new live tenure.
    Reconnect,
    /// A previously unseen authenticated source attached an independent route.
    NewAlternateSource,
}

impl FairV2IngressOwnershipAction {
    const COUNT: usize = 5;

    const fn index(self) -> usize {
        match self {
            Self::New => 0,
            Self::ExactDuplicate => 1,
            Self::SameSourceLaterDelivery => 2,
            Self::Reconnect => 3,
            Self::NewAlternateSource => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressMessageKind {
    V2Proposal,
    V2Vote,
    V2QuorumCertificate,
    V2TimeoutVote,
    V2TimeoutCertificate,
    V2PayloadManifest,
    V2PayloadChunk,
    V2CertifiedBodyRequest,
    V2CertifiedBodyResponse,
    V2CommitCertificateRequest,
    V2CommitCertificateResponse,
    V2VrfCommit,
    V2VrfReveal,
    KuraReplicaAdvert,
    LaneBlockProposal,
    LaneExecutablePayload,
    LaneBlockNewViewVote,
    LaneBlockNewViewCertificate,
    LaneBlockVote,
    LaneBlockQc,
    LaneBlockCertificate,
    LaneHistoricalRecoveryRequest,
    LaneHistoricalRecoveryResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FairV2IngressControlKind {
    Proposal,
    PrepareVote,
    CommitVote,
    PrepareQc,
    CommitQc,
    TimeoutVote,
    TimeoutCertificate,
}

fn fair_v2_ingress_control_kind(message: &BlockMessage) -> Option<FairV2IngressControlKind> {
    use iroha_data_model::block::consensus_v2::{ConsensusMessageV2Payload, GlobalPhase};

    let BlockMessage::V2(message) = message else {
        return None;
    };
    Some(match &message.payload {
        ConsensusMessageV2Payload::Proposal(_) => FairV2IngressControlKind::Proposal,
        ConsensusMessageV2Payload::Vote(vote) => match vote.phase {
            GlobalPhase::Prepare => FairV2IngressControlKind::PrepareVote,
            GlobalPhase::Commit => FairV2IngressControlKind::CommitVote,
        },
        ConsensusMessageV2Payload::QuorumCertificate(certificate) => match certificate.phase {
            GlobalPhase::Prepare => FairV2IngressControlKind::PrepareQc,
            GlobalPhase::Commit => FairV2IngressControlKind::CommitQc,
        },
        ConsensusMessageV2Payload::TimeoutVote(_) => FairV2IngressControlKind::TimeoutVote,
        ConsensusMessageV2Payload::TimeoutCertificate(_) => {
            FairV2IngressControlKind::TimeoutCertificate
        }
        ConsensusMessageV2Payload::PayloadManifest(_)
        | ConsensusMessageV2Payload::PayloadChunk(_)
        | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | ConsensusMessageV2Payload::VrfCommit(_)
        | ConsensusMessageV2Payload::VrfReveal(_) => return None,
    })
}

fn fair_v2_ingress_same_control_slot(
    left: &InboundBlockMessage,
    right: &InboundBlockMessage,
) -> bool {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

    let control_round = |inbound: &InboundBlockMessage| {
        let BlockMessage::V2(message) = inbound.message() else {
            return None;
        };
        Some(match &message.payload {
            ConsensusMessageV2Payload::Proposal(proposal) => proposal.round,
            ConsensusMessageV2Payload::Vote(vote) => vote.round,
            ConsensusMessageV2Payload::QuorumCertificate(certificate) => certificate.round,
            ConsensusMessageV2Payload::TimeoutVote(vote) => vote.round,
            ConsensusMessageV2Payload::TimeoutCertificate(certificate) => certificate.round,
            ConsensusMessageV2Payload::PayloadManifest(_)
            | ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | ConsensusMessageV2Payload::VrfCommit(_)
            | ConsensusMessageV2Payload::VrfReveal(_) => return None,
        })
    };
    let (Some(left_kind), Some(right_kind), Some(left_round), Some(right_round)) = (
        fair_v2_ingress_control_kind(left.message()),
        fair_v2_ingress_control_kind(right.message()),
        control_round(left),
        control_round(right),
    ) else {
        return false;
    };
    left.sender() == right.sender()
        && left_kind == right_kind
        && left_round.context_id == right_round.context_id
        && left_round.height == right_round.height
}

/// Whether timeout control can advance past the selected control owner's view.
///
/// A direct Vote may deliberately remain in fair ingress until its Proposal
/// binds the execution commitment. Requiring that blocked Vote to cross before
/// same-view timeout shares creates a circular dependency: those shares must
/// assemble the TC which retires the view's proposal and vote work. An already
/// assembled TC for that view or a later one has the same dependency. Every
/// candidate still crosses normal downstream authentication and quorum checks;
/// this helper only allows the verifier to observe it when the immutable
/// control owner is currently inadmissible.
fn fair_v2_ingress_timeout_control_advances_owner(
    owner: &FairV2IngressLeaderWireToken,
    inbound: &InboundBlockMessage,
) -> bool {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

    if !matches!(
        owner.identity.phase,
        FairV2IngressLeaderWirePhase::Proposal
            | FairV2IngressLeaderWirePhase::PrepareVote
            | FairV2IngressLeaderWirePhase::CommitVote
            | FairV2IngressLeaderWirePhase::PrepareQc
            | FairV2IngressLeaderWirePhase::CommitQc
            | FairV2IngressLeaderWirePhase::TimeoutVote
    ) {
        return false;
    }
    let BlockMessage::V2(message) = inbound.message() else {
        return false;
    };
    let (round, view_advances) = match &message.payload {
        ConsensusMessageV2Payload::TimeoutVote(vote) => (
            vote.round,
            owner.identity.phase != FairV2IngressLeaderWirePhase::TimeoutVote
                && v2_core::timeout_vote_view_is_admissible(owner.identity.view, vote.round.view),
        ),
        ConsensusMessageV2Payload::TimeoutCertificate(certificate) => (
            certificate.round,
            certificate.round.view >= owner.identity.view,
        ),
        _ => return false,
    };
    round.context_id == owner.identity.context_id
        && round.height == owner.identity.height
        && view_advances
}

/// Whether a certified reducer input can retire the selected control owner.
///
/// Fair ingress observes only authenticated transport provenance at this
/// point; the reducer still verifies the certificate and sender before any
/// state transition. This dependency edge merely prevents a retained
/// Proposal/Prepare/signing owner from hiding the TC or CommitQC that can
/// supersede it.
fn fair_v2_ingress_certified_fence_escape_advances_owner(
    owner: &FairV2IngressLeaderWireToken,
    inbound: &InboundBlockMessage,
) -> bool {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

    let BlockMessage::V2(message) = inbound.message() else {
        return false;
    };
    let round = match &message.payload {
        ConsensusMessageV2Payload::TimeoutCertificate(certificate) => certificate.round,
        ConsensusMessageV2Payload::QuorumCertificate(certificate)
            if certificate.phase == iroha_data_model::block::consensus_v2::GlobalPhase::Commit =>
        {
            certificate.round
        }
        ConsensusMessageV2Payload::CommitCertificateResponse(response)
            if response.certificate.phase
                == iroha_data_model::block::consensus_v2::GlobalPhase::Commit =>
        {
            response.certificate.round
        }
        _ => return false,
    };
    round.context_id == owner.identity.context_id
        && round.height == owner.identity.height
        && round.view >= owner.identity.view
}

/// Whether one envelope carries the exact certified authority which may cross
/// a retained fair-ingress reservation without replacing its owner.
///
/// The shared classifier is deliberately closed over TC, direct CommitQC, and
/// a discovery response containing CommitQC. Version validation happens here
/// as well as at runtime admission so a malformed envelope cannot acquire the
/// dependency-bypass position merely from its payload discriminant.
fn fair_v2_ingress_is_certified_fence_escape(inbound: &InboundBlockMessage) -> bool {
    fair_v2_ingress_message_is_certified_fence_escape(inbound.message())
}

fn fair_v2_ingress_message_is_certified_fence_escape(message: &BlockMessage) -> bool {
    let BlockMessage::V2(message) = message else {
        return false;
    };
    message.validate_version().is_ok()
        && v2_effects::network_ingress_is_certified_fence_escape(&message.payload)
}

fn fair_v2_ingress_subject_hash(
    subject: Option<&iroha_data_model::block::consensus_v2::BlockSubject>,
) -> CryptoHash {
    subject.map_or_else(
        || CryptoHash::new([]),
        |subject| CryptoHash::new(subject.encode()),
    )
}

fn fair_v2_ingress_is_productive_leader_wire(message: &BlockMessage) -> bool {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

    matches!(
        message,
        BlockMessage::V2(ConsensusMessageV2 {
            payload: ConsensusMessageV2Payload::Proposal(_)
                | ConsensusMessageV2Payload::Vote(_)
                | ConsensusMessageV2Payload::QuorumCertificate(_)
                | ConsensusMessageV2Payload::TimeoutVote(_)
                | ConsensusMessageV2Payload::TimeoutCertificate(_)
                | ConsensusMessageV2Payload::PayloadChunk(_)
                | ConsensusMessageV2Payload::CertifiedBodyResponse(_),
            ..
        })
    )
}

/// Derive the exact bounded lifecycle identity after authenticated-source and
/// structural-size policy checks, but before any physical queue capacity gate.
///
/// A chunk does not repeat its round or subject on the wire. It is therefore
/// productive at this boundary only after an exact retained Proposal owner
/// binds its manifest hash to those coordinates. Reordered orphan chunks
/// remain retryable transport work and cannot reserve an unbounded guessed
/// subject slot.
fn fair_v2_ingress_leader_wire_identity(
    state: &FairV2IngressState,
    message: &BlockMessage,
    semantic_origin: &PeerId,
    canonical_wire_hash: CryptoHash,
) -> FairV2IngressLeaderWireDerivation {
    use iroha_data_model::block::consensus_v2::{ConsensusMessageV2Payload, GlobalPhase};

    let BlockMessage::V2(message) = message else {
        return FairV2IngressLeaderWireDerivation::NotApplicable;
    };
    let (context_id, height, view, subject_hash, manifest_hash, phase, chunk_index) = match &message
        .payload
    {
        ConsensusMessageV2Payload::Proposal(proposal) => (
            proposal.round.context_id,
            proposal.round.height,
            proposal.round.view,
            fair_v2_ingress_subject_hash(Some(&proposal.subject)),
            Some(CryptoHash::new(proposal.manifest.encode())),
            FairV2IngressLeaderWirePhase::Proposal,
            None,
        ),
        ConsensusMessageV2Payload::Vote(vote) => (
            vote.round.context_id,
            vote.round.height,
            vote.round.view,
            fair_v2_ingress_subject_hash(Some(&vote.subject)),
            None,
            match vote.phase {
                GlobalPhase::Prepare => FairV2IngressLeaderWirePhase::PrepareVote,
                GlobalPhase::Commit => FairV2IngressLeaderWirePhase::CommitVote,
            },
            None,
        ),
        ConsensusMessageV2Payload::QuorumCertificate(certificate) => (
            certificate.round.context_id,
            certificate.round.height,
            certificate.round.view,
            fair_v2_ingress_subject_hash(Some(&certificate.subject)),
            None,
            match certificate.phase {
                GlobalPhase::Prepare => FairV2IngressLeaderWirePhase::PrepareQc,
                GlobalPhase::Commit => FairV2IngressLeaderWirePhase::CommitQc,
            },
            None,
        ),
        ConsensusMessageV2Payload::TimeoutVote(vote) => (
            vote.round.context_id,
            vote.round.height,
            vote.round.view,
            fair_v2_ingress_subject_hash(None),
            None,
            FairV2IngressLeaderWirePhase::TimeoutVote,
            None,
        ),
        ConsensusMessageV2Payload::TimeoutCertificate(certificate) => (
            certificate.round.context_id,
            certificate.round.height,
            certificate.round.view,
            fair_v2_ingress_subject_hash(None),
            None,
            FairV2IngressLeaderWirePhase::TimeoutCertificate,
            None,
        ),
        ConsensusMessageV2Payload::PayloadChunk(chunk) => {
            if chunk.index >= state.leader_wire_max_chunk_count {
                return FairV2IngressLeaderWireDerivation::Reject;
            }
            let chunk_manifest_hash: CryptoHash = chunk.manifest_hash.into();
            let retained_slot = FairV2IngressLeaderWireSlot {
                semantic_origin: semantic_origin.clone(),
                phase: FairV2IngressLeaderWirePhase::Chunk,
                chunk_index: Some(chunk.index),
            };
            let retained_coordinates = state
                .leader_wire_lifecycles
                .get(&retained_slot)
                .filter(|record| record.token.identity.manifest_hash == Some(chunk_manifest_hash))
                .map(|record| {
                    let identity = &record.token.identity;
                    (
                        identity.context_id,
                        identity.height,
                        identity.view,
                        identity.subject_hash,
                    )
                });
            let coordinates = retained_coordinates
                .into_iter()
                .chain(state.leader_wire_lifecycles.values().filter_map(|record| {
                    let identity = &record.token.identity;
                    (identity.phase == FairV2IngressLeaderWirePhase::Proposal
                        && identity.manifest_hash == Some(chunk_manifest_hash))
                    .then_some((
                        identity.context_id,
                        identity.height,
                        identity.view,
                        identity.subject_hash,
                    ))
                }))
                .collect::<BTreeSet<_>>();
            let mut coordinates = coordinates.into_iter();
            let Some(coordinate) = coordinates.next() else {
                return FairV2IngressLeaderWireDerivation::UnboundChunk;
            };
            if coordinates.next().is_some() {
                return FairV2IngressLeaderWireDerivation::Reject;
            }
            (
                coordinate.0,
                coordinate.1,
                coordinate.2,
                coordinate.3,
                Some(chunk_manifest_hash),
                FairV2IngressLeaderWirePhase::Chunk,
                Some(chunk.index),
            )
        }
        ConsensusMessageV2Payload::CertifiedBodyResponse(response) => (
            response.manifest.round.context_id,
            response.manifest.round.height,
            response.manifest.round.view,
            fair_v2_ingress_subject_hash(Some(&response.manifest.subject)),
            Some(CryptoHash::new(response.manifest.encode())),
            FairV2IngressLeaderWirePhase::CertifiedResponse,
            None,
        ),
        ConsensusMessageV2Payload::PayloadManifest(_)
        | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | ConsensusMessageV2Payload::VrfCommit(_)
        | ConsensusMessageV2Payload::VrfReveal(_) => {
            return FairV2IngressLeaderWireDerivation::NotApplicable;
        }
    };
    let slot = FairV2IngressLeaderWireSlot {
        semantic_origin: semantic_origin.clone(),
        phase,
        chunk_index,
    };
    let identity = FairV2IngressLeaderWireIdentity {
        context_id,
        height,
        view,
        subject_hash,
        manifest_hash,
        phase,
        semantic_origin: semantic_origin.clone(),
        canonical_wire_hash,
    };
    FairV2IngressLeaderWireDerivation::Exact { identity, slot }
}

fn fair_v2_ingress_admit_leader_wire(
    state: &mut FairV2IngressState,
    derivation: FairV2IngressLeaderWireDerivation,
    publish_ingress: bool,
) -> Result<FairV2IngressLeaderWireAdmission, FairV2IngressLeaderWireAdmissionError> {
    let (identity, slot) = match derivation {
        FairV2IngressLeaderWireDerivation::NotApplicable => {
            return Ok(FairV2IngressLeaderWireAdmission::NotApplicable);
        }
        FairV2IngressLeaderWireDerivation::UnboundChunk => {
            // The bounded worker orphan lifecycle is the sole owner until a
            // Proposal binds this wire to an exact round/subject. Let normal
            // fair ingress carry the packet and its route-neutral ownership to
            // that lifecycle without minting a durable leader-wire ordinal.
            return Ok(FairV2IngressLeaderWireAdmission::NotApplicable);
        }
        FairV2IngressLeaderWireDerivation::Reject => {
            return Err(FairV2IngressLeaderWireAdmissionError::Rejected);
        }
        FairV2IngressLeaderWireDerivation::Exact { identity, slot } => (identity, slot),
    };
    let gate = state
        .leader_wire_lifecycle_gate
        .as_ref()
        .cloned()
        .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    if gate
        .identity_is_obsolete(&identity)
        .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?
    {
        return Err(FairV2IngressLeaderWireAdmissionError::Rejected);
    }
    let durable_exact = gate
        .lookup_exact(&identity, &slot)
        .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    if let Some(incumbent) = state.leader_wire_lifecycles.get(&slot) {
        if incumbent.token.identity == identity {
            let receipt = durable_exact.ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;
            let durable_status_matches = matches!(
                (incumbent.status, receipt.status()),
                (
                    FairV2IngressLeaderWireStatus::Dormant,
                    serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
                ) | (
                    FairV2IngressLeaderWireStatus::Ingress,
                    serviced_candidate_store::LeaderWireLifecycleStatus::Ingress
                ) | (
                    FairV2IngressLeaderWireStatus::Runtime,
                    serviced_candidate_store::LeaderWireLifecycleStatus::Runtime
                ) | (
                    FairV2IngressLeaderWireStatus::VolatileTerminal,
                    serviced_candidate_store::LeaderWireLifecycleStatus::VolatileTerminal
                ) | (
                    FairV2IngressLeaderWireStatus::Terminal,
                    serviced_candidate_store::LeaderWireLifecycleStatus::Terminal
                )
            );
            if receipt.token() != &incumbent.token || receipt.inserted() || !durable_status_matches
            {
                return Err(FairV2IngressLeaderWireAdmissionError::Exhausted);
            }
            let incumbent_status = incumbent.status;
            let incumbent_token = incumbent.token.clone();
            if incumbent_status == FairV2IngressLeaderWireStatus::Dormant && publish_ingress {
                // The logical token survives restart, but its physical queue
                // position does not. Exact replay therefore freezes the
                // complete currently admitted source prefix just before the
                // fresh carrier is published. No work already in ingress can
                // be displaced by the token's older logical ordinal.
                let ingress_predecessors = state
                    .lanes
                    .iter()
                    .map(|(source, lane)| (source.clone(), lane.entries.len()))
                    .collect();
                let receipt = gate
                    .admit_ingress(incumbent_token.clone())
                    .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?;
                if receipt.token() != &incumbent_token
                    || receipt.status()
                        != serviced_candidate_store::LeaderWireLifecycleStatus::Ingress
                    || receipt.inserted()
                {
                    return Err(FairV2IngressLeaderWireAdmissionError::Exhausted);
                }
                let incumbent = state
                    .leader_wire_lifecycles
                    .get_mut(&slot)
                    .expect("validated dormant leader-wire slot remains indexed");
                incumbent.status = FairV2IngressLeaderWireStatus::Ingress;
                incumbent.ingress_predecessors = ingress_predecessors;
            }
            return Ok(match incumbent_status {
                FairV2IngressLeaderWireStatus::Dormant if !publish_ingress => {
                    FairV2IngressLeaderWireAdmission::Ready
                }
                FairV2IngressLeaderWireStatus::Dormant => {
                    FairV2IngressLeaderWireAdmission::Admitted(incumbent_token)
                }
                FairV2IngressLeaderWireStatus::Ingress
                | FairV2IngressLeaderWireStatus::Runtime
                | FairV2IngressLeaderWireStatus::VolatileTerminal
                | FairV2IngressLeaderWireStatus::Terminal => {
                    FairV2IngressLeaderWireAdmission::Coalesced
                }
            });
        }
        if incumbent.status.blocks_replacement() {
            return Err(if identity.view > incumbent.token.identity.view {
                FairV2IngressLeaderWireAdmissionError::Busy
            } else {
                FairV2IngressLeaderWireAdmissionError::Rejected
            });
        }
        if identity.context_id != incumbent.token.identity.context_id
            || identity.height != incumbent.token.identity.height
            || identity.view <= incumbent.token.identity.view
        {
            return Err(FairV2IngressLeaderWireAdmissionError::Rejected);
        }
    } else if durable_exact.is_some() {
        return Err(FairV2IngressLeaderWireAdmissionError::Exhausted);
    }

    let capacity = fair_v2_ingress_leader_wire_lifecycle_capacity(
        state.roster.len(),
        state.leader_wire_max_chunk_count,
    )
    .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    if !state.leader_wire_lifecycles.contains_key(&slot)
        && state.leader_wire_lifecycles.len() >= capacity
    {
        return Err(FairV2IngressLeaderWireAdmissionError::Exhausted);
    }
    if !publish_ingress {
        return Ok(FairV2IngressLeaderWireAdmission::Ready);
    }

    let admission_ordinal = state
        .last_admission_ordinal
        .checked_add(1)
        .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    let scheduler_ordinal = state
        .leader_wire_lifecycle_ordinals
        .as_ref()
        .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?
        .reserve_one()
        .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    let ingress_predecessors = state
        .lanes
        .iter()
        .map(|(source, lane)| {
            (
                source.clone(),
                lane.entries
                    .iter()
                    .filter(|entry| entry.admission_ordinal < admission_ordinal)
                    .count(),
            )
        })
        .collect();
    let token = FairV2IngressLeaderWireToken {
        source_class: identity.phase.source_class(),
        identity,
        slot: slot.clone(),
        admission_ordinal,
        scheduler_ordinal,
    };
    let receipt = gate
        .admit_ingress(token.clone())
        .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?;
    if receipt.token() != &token
        || receipt.status() != serviced_candidate_store::LeaderWireLifecycleStatus::Ingress
        || !receipt.inserted()
    {
        return Err(FairV2IngressLeaderWireAdmissionError::Exhausted);
    }
    state.leader_wire_lifecycles.insert(
        slot.clone(),
        FairV2IngressLeaderWireRecord {
            token: token.clone(),
            status: FairV2IngressLeaderWireStatus::Ingress,
            restored_runtime_owner: None,
            ingress_predecessors,
        },
    );
    state.last_admission_ordinal = admission_ordinal;
    Ok(FairV2IngressLeaderWireAdmission::Admitted(token))
}

impl FairV2IngressMessageKind {
    fn classify(message: &BlockMessage) -> Option<Self> {
        use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

        match message {
            BlockMessage::V2(message) => Some(match &message.payload {
                ConsensusMessageV2Payload::Proposal(_) => Self::V2Proposal,
                ConsensusMessageV2Payload::Vote(_) => Self::V2Vote,
                ConsensusMessageV2Payload::QuorumCertificate(_) => Self::V2QuorumCertificate,
                ConsensusMessageV2Payload::TimeoutVote(_) => Self::V2TimeoutVote,
                ConsensusMessageV2Payload::TimeoutCertificate(_) => Self::V2TimeoutCertificate,
                ConsensusMessageV2Payload::PayloadManifest(_) => Self::V2PayloadManifest,
                ConsensusMessageV2Payload::PayloadChunk(_) => Self::V2PayloadChunk,
                ConsensusMessageV2Payload::CertifiedBodyRequest(_) => Self::V2CertifiedBodyRequest,
                ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
                    Self::V2CertifiedBodyResponse
                }
                ConsensusMessageV2Payload::CommitCertificateRequest(_) => {
                    Self::V2CommitCertificateRequest
                }
                ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
                    Self::V2CommitCertificateResponse
                }
                ConsensusMessageV2Payload::VrfCommit(_) => Self::V2VrfCommit,
                ConsensusMessageV2Payload::VrfReveal(_) => Self::V2VrfReveal,
            }),
            BlockMessage::LaneBlockProposal(_) => Some(Self::LaneBlockProposal),
            BlockMessage::LaneExecutablePayload(_) => Some(Self::LaneExecutablePayload),
            BlockMessage::LaneBlockNewViewVote(_) => Some(Self::LaneBlockNewViewVote),
            BlockMessage::LaneBlockNewViewCertificate(_) => Some(Self::LaneBlockNewViewCertificate),
            BlockMessage::LaneBlockVote(_) => Some(Self::LaneBlockVote),
            BlockMessage::LaneBlockQc(_) => Some(Self::LaneBlockQc),
            BlockMessage::LaneBlockCertificate(_) => Some(Self::LaneBlockCertificate),
            BlockMessage::LaneHistoricalRecoveryRequest(_) => {
                Some(Self::LaneHistoricalRecoveryRequest)
            }
            BlockMessage::LaneHistoricalRecoveryResponse(_) => {
                Some(Self::LaneHistoricalRecoveryResponse)
            }
            BlockMessage::KuraReplicaAdvert(_) => Some(Self::KuraReplicaAdvert),
            _ => None,
        }
    }

    const fn is_v2(self) -> bool {
        matches!(
            self,
            Self::V2Proposal
                | Self::V2Vote
                | Self::V2QuorumCertificate
                | Self::V2TimeoutVote
                | Self::V2TimeoutCertificate
                | Self::V2PayloadManifest
                | Self::V2PayloadChunk
                | Self::V2CertifiedBodyRequest
                | Self::V2CertifiedBodyResponse
                | Self::V2CommitCertificateRequest
                | Self::V2CommitCertificateResponse
                | Self::V2VrfCommit
                | Self::V2VrfReveal
        )
    }
}

fn fair_v2_ingress_consensus_round(
    message: &BlockMessage,
) -> Option<iroha_data_model::block::consensus_v2::ConsensusRound> {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

    let BlockMessage::V2(message) = message else {
        return None;
    };
    match &message.payload {
        ConsensusMessageV2Payload::Proposal(proposal) => Some(proposal.round),
        ConsensusMessageV2Payload::Vote(vote) => Some(vote.round),
        ConsensusMessageV2Payload::QuorumCertificate(certificate) => Some(certificate.round),
        ConsensusMessageV2Payload::TimeoutVote(vote) => Some(vote.round),
        ConsensusMessageV2Payload::TimeoutCertificate(certificate) => Some(certificate.round),
        ConsensusMessageV2Payload::PayloadManifest(manifest) => Some(manifest.round),
        ConsensusMessageV2Payload::CertifiedBodyRequest(request) => Some(request.round),
        ConsensusMessageV2Payload::CertifiedBodyResponse(response) => Some(response.manifest.round),
        ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            Some(response.certificate.round)
        }
        ConsensusMessageV2Payload::PayloadChunk(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::VrfCommit(_)
        | ConsensusMessageV2Payload::VrfReveal(_) => None,
    }
}

fn fair_v2_ingress_is_certified_body_request(inbound: &InboundBlockMessage) -> bool {
    matches!(
        FairV2IngressMessageKind::classify(inbound.message()),
        Some(FairV2IngressMessageKind::V2CertifiedBodyRequest)
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FairV2IngressResourceSnapshot {
    source_len: usize,
    source_progress_len: usize,
    source_certified_fence_escape_len: usize,
    source_timeout_vote_len: usize,
    source_transport_completion_len: usize,
    source_bytes: usize,
    source_certified_fence_escape_bytes: usize,
    source_timeout_vote_bytes: usize,
    source_transport_completion_bytes: usize,
    global_len: usize,
    global_bytes: usize,
    protected_slots: usize,
    message_capacity: usize,
    global_byte_capacity: usize,
    source_byte_capacity: usize,
    certified_fence_escape_byte_reserve: usize,
    timeout_vote_byte_reserve: usize,
    transport_completion_byte_reserve: usize,
}

#[derive(Clone, Debug)]
struct FairV2IngressReplyAttempt {
    route: NetworkReplyRoute,
    message_cursor: u64,
    chunk_cursor: u64,
}

#[derive(Clone, Debug)]
struct FairV2IngressOwnershipOccurrence {
    action: FairV2IngressOwnershipAction,
    /// Receiver-local physical FIFO position for this admitted occurrence.
    ///
    /// This is internal scheduling metadata, not part of the authenticated
    /// wire identity. Exact retransmission coalesces with the existing
    /// occurrence; restart re-admission receives a fresh value.
    physical_admission_ordinal: u64,
    /// Actor-global lifecycle position retained across the runtime handoff.
    ///
    /// Test-only ungated ingress may omit this owner. Every production-bound
    /// occurrence carries either its special gate ordinal or one freshly
    /// minted from the same internal source.
    lifecycle_ordinal: Option<u128>,
    wire_key: FairV2IngressWireKey,
    semantic_origin: Option<PeerId>,
    authenticated_via: Option<PeerId>,
    authenticated_via_is_validator: bool,
    authenticated_source: FairV2IngressSource,
    semantic_owner_source: FairV2IngressSource,
    message_kind: FairV2IngressMessageKind,
    class: FairV2IngressClass,
    encoded_bytes: Arc<[u8]>,
    encoded_len: usize,
    resource_before: FairV2IngressResourceSnapshot,
    resource_after: FairV2IngressResourceSnapshot,
    routes_before: Option<NetworkReplyRoutes>,
    routes_candidate: Option<NetworkReplyRoutes>,
    routes_after: Option<NetworkReplyRoutes>,
    route_capacity: Option<usize>,
    attempts_before: Vec<FairV2IngressReplyAttempt>,
    attempts_before_hash: CryptoHash,
    attempts_after: Vec<FairV2IngressReplyAttempt>,
    attempts_after_hash: CryptoHash,
}

/// Bounded, process-local proof carrier for one queued semantic request.
///
/// The first and latest occurrences are retained exactly, while fixed-width
/// counters preserve the complete action history without letting duplicate
/// traffic allocate an unbounded vector. Opaque capabilities and cursors stay
/// local and are never serialized.
#[derive(Clone, Debug)]
pub(crate) struct FairV2IngressOwnershipEvidence {
    first: FairV2IngressOwnershipOccurrence,
    latest: FairV2IngressOwnershipOccurrence,
    /// First receiver-local physical ordinal which was not yet admitted when
    /// this occurrence crossed from fair ingress into serialized runtime.
    ///
    /// The dequeue transaction freezes this once. A producer continuation
    /// derived from the occurrence can therefore distinguish real pre-cut
    /// ingress from a later replay retaining an older logical lifecycle.
    runtime_physical_cut: Option<u128>,
    leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    leader_wire_runtime_receipt:
        Option<serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt>,
    admission_count: u128,
    occurrence_count: u128,
    action_counts: [u128; FairV2IngressOwnershipAction::COUNT],
    current_routes: Option<NetworkReplyRoutes>,
    attempts: Vec<FairV2IngressReplyAttempt>,
    attempts_hash: CryptoHash,
}

fn fair_v2_ingress_append_peer_identity(projection: &mut Vec<u8>, peer: &PeerId) {
    let encoded = peer.encode();
    projection.extend_from_slice(
        &u64::try_from(encoded.len())
            .expect("canonical peer identity length fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(&encoded);
}

fn fair_v2_ingress_append_optional_peer_identity(projection: &mut Vec<u8>, peer: Option<&PeerId>) {
    match peer {
        None => projection.push(0),
        Some(peer) => {
            projection.push(1);
            fair_v2_ingress_append_peer_identity(projection, peer);
        }
    }
}

fn fair_v2_ingress_append_source_identity(projection: &mut Vec<u8>, source: &FairV2IngressSource) {
    match source {
        FairV2IngressSource::Anonymous => projection.push(0),
        FairV2IngressSource::Validator(peer) => {
            projection.push(1);
            fair_v2_ingress_append_peer_identity(projection, peer);
        }
        FairV2IngressSource::Authenticated(peer) => {
            projection.push(2);
            fair_v2_ingress_append_peer_identity(projection, peer);
        }
    }
}

impl FairV2IngressOwnershipEvidence {
    fn new(
        occurrence: FairV2IngressOwnershipOccurrence,
        leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    ) -> Self {
        let mut action_counts = [0; FairV2IngressOwnershipAction::COUNT];
        action_counts[occurrence.action.index()] = 1;
        Self {
            leader_wire_token,
            leader_wire_runtime_receipt: None,
            runtime_physical_cut: None,
            current_routes: occurrence.routes_after.clone(),
            attempts: occurrence.attempts_after.clone(),
            attempts_hash: occurrence.attempts_after_hash,
            first: occurrence.clone(),
            latest: occurrence,
            admission_count: 1,
            occurrence_count: 1,
            action_counts,
        }
    }

    fn merged(&self, occurrence: FairV2IngressOwnershipOccurrence) -> Option<Self> {
        let occurrence_count = self.occurrence_count.checked_add(1)?;
        let mut action_counts = self.action_counts;
        let count = action_counts.get_mut(occurrence.action.index())?;
        *count = count.checked_add(1)?;
        Some(Self {
            first: self.first.clone(),
            leader_wire_token: self.leader_wire_token.clone(),
            leader_wire_runtime_receipt: self.leader_wire_runtime_receipt.clone(),
            runtime_physical_cut: self.runtime_physical_cut,
            admission_count: self.admission_count,
            current_routes: occurrence.routes_after.clone(),
            attempts: occurrence.attempts_after.clone(),
            attempts_hash: occurrence.attempts_after_hash,
            latest: occurrence,
            occurrence_count,
            action_counts,
        })
    }

    /// Merge a later fair-ingress admission into one already-owned downstream
    /// semantic request without discarding either admission's bounded history.
    ///
    /// Route capabilities remain opaque. The merge delegates authority,
    /// tenure, target, source-capacity, staleness, and ordinal checks to the
    /// route set, while per-source cursors advance to the greatest progress
    /// already observed by either exact carrier.
    pub(crate) fn merge_downstream(&mut self, candidate: Self) -> bool {
        if !self.same_semantic_request(&candidate) {
            return false;
        }
        match (&self.current_routes, &candidate.current_routes) {
            (Some(retained), Some(observed)) => {
                let mut reconciled = retained.clone();
                let Ok(receipt) = reconciled.merge_observed_with_receipt(observed) else {
                    return false;
                };
                self.merge_downstream_with_observed_receipt(candidate, receipt)
                    .is_some()
            }
            (None, Some(observed)) => {
                self.merge_downstream_with_exact_routes(candidate.clone(), Some(observed.clone()))
            }
            (Some(retained), None) => {
                self.merge_downstream_with_exact_routes(candidate, Some(retained.clone()))
            }
            (None, None) => self.merge_downstream_with_exact_routes(candidate, None),
        }
    }

    /// Merge ownership using the consumed receipt of an observed reconciliation.
    ///
    /// Returns the receipt-owned output route set. The caller installs this
    /// value as its independently carried route history.
    pub(crate) fn merge_downstream_with_observed_receipt(
        &mut self,
        candidate: Self,
        receipt: NetworkReplyRoutesObservedMergeReceipt,
    ) -> Option<NetworkReplyRoutes> {
        if !self.can_merge_downstream_exact(&candidate) {
            return None;
        }
        let current_routes = receipt.into_output(
            self.current_routes.as_ref()?,
            candidate.current_routes.as_ref()?,
        )?;
        self.merge_downstream_with_exact_routes(candidate, Some(current_routes.clone()))
            .then_some(current_routes)
    }

    /// Merge ownership using the consumed receipt of a strict reconciliation.
    ///
    /// Strict and observed receipt types remain distinct so a stale-tolerant
    /// observation cannot cross an exact-output admission seam.
    pub(crate) fn merge_downstream_with_strict_receipt(
        &mut self,
        candidate: Self,
        receipt: NetworkReplyRoutesStrictMergeReceipt,
    ) -> Option<NetworkReplyRoutes> {
        if !self.can_merge_downstream_exact(&candidate) {
            return None;
        }
        let current_routes = receipt.into_output(
            self.current_routes.as_ref()?,
            candidate.current_routes.as_ref()?,
        )?;
        self.merge_downstream_with_exact_routes(candidate, Some(current_routes.clone()))
            .then_some(current_routes)
    }

    fn can_merge_downstream_exact(&self, candidate: &Self) -> bool {
        self.same_semantic_request(candidate)
    }

    fn merge_downstream_with_exact_routes(
        &mut self,
        mut candidate: Self,
        current_routes: Option<NetworkReplyRoutes>,
    ) -> bool {
        if !self.can_merge_downstream_exact(&candidate) {
            return false;
        }
        let retained_lifecycle = self.first.lifecycle_ordinal;
        let candidate_lifecycle = candidate.first.lifecycle_ordinal;
        if retained_lifecycle.is_some() != candidate_lifecycle.is_some()
            || matches!(
                (retained_lifecycle, candidate_lifecycle),
                (Some(retained), Some(candidate)) if candidate < retained
            )
        {
            return false;
        }
        if candidate_lifecycle != retained_lifecycle {
            // A retry admitted after the first carrier crossed into runtime is
            // a later physical occurrence of the same logical request. Merge
            // its bounded action/route history without letting that retry
            // replace the immutable first lifecycle position. Productive
            // leader-wire carriers cannot take this path because their token
            // binds the scheduler ordinal and is part of semantic equality.
            if candidate.leader_wire_token.is_some()
                || candidate.leader_wire_runtime_receipt.is_some()
            {
                return false;
            }
            candidate.first.lifecycle_ordinal = retained_lifecycle;
            candidate.latest.lifecycle_ordinal = retained_lifecycle;
            if !candidate.validate_exact() {
                return false;
            }
        }
        // Downstream coalescence retains the first physical occurrence and
        // its immutable predecessor cut. A later route/source observation is
        // history for that logical request, not a replacement FIFO owner.
        candidate.first.physical_admission_ordinal = self.first.physical_admission_ordinal;
        candidate.latest.physical_admission_ordinal = self.first.physical_admission_ordinal;
        candidate.runtime_physical_cut = self.runtime_physical_cut;
        if !candidate.validate_exact() {
            return false;
        }
        let admission_count = match self.admission_count.checked_add(candidate.admission_count) {
            Some(count) => count,
            None => return false,
        };
        let occurrence_count = match self
            .occurrence_count
            .checked_add(candidate.occurrence_count)
        {
            Some(count) => count,
            None => return false,
        };
        let mut action_counts = self.action_counts;
        for (retained, observed) in action_counts.iter_mut().zip(candidate.action_counts) {
            let Some(merged) = retained.checked_add(observed) else {
                return false;
            };
            *retained = merged;
        }
        let Some(attempts) = fair_v2_ingress_merge_attempt_cursors(
            &self.attempts,
            &candidate.attempts,
            current_routes.as_ref(),
        ) else {
            return false;
        };
        let attempts_hash = fair_v2_ingress_attempt_cursor_hash(&attempts);
        let merged = Self {
            first: self.first.clone(),
            latest: candidate.latest,
            leader_wire_token: self.leader_wire_token.clone(),
            leader_wire_runtime_receipt: self.leader_wire_runtime_receipt.clone(),
            runtime_physical_cut: self.runtime_physical_cut,
            admission_count,
            occurrence_count,
            action_counts,
            current_routes,
            attempts,
            attempts_hash,
        };
        if !merged.validate_exact() {
            return false;
        }
        *self = merged;
        true
    }

    /// Whether two validated carriers name the same canonical semantic
    /// request rather than merely sharing identical wire bytes.
    ///
    /// Distinct semantic origins are independent requests. Alternate
    /// authenticated delivery sources for one origin retain the same wire key
    /// and may merge their per-source routes.
    pub(crate) fn same_semantic_request(&self, other: &Self) -> bool {
        self.validate_exact()
            && other.validate_exact()
            && self.first.wire_key == other.first.wire_key
            && self.first.message_kind == other.first.message_kind
            && self.first.class == other.first.class
            && self.first.encoded_bytes == other.first.encoded_bytes
            && self.leader_wire_token == other.leader_wire_token
            && self.leader_wire_runtime_receipt == other.leader_wire_runtime_receipt
    }

    /// Exact productive leader-wire admission carried across downstream cuts.
    pub(crate) const fn leader_wire_token(&self) -> Option<&FairV2IngressLeaderWireToken> {
        self.leader_wire_token.as_ref()
    }

    /// Durable ingress-to-runtime handoff paired with the productive token.
    pub(crate) const fn leader_wire_runtime_receipt(
        &self,
    ) -> Option<&serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt> {
        self.leader_wire_runtime_receipt.as_ref()
    }

    /// Earliest actor-global lifecycle position carried into serialized runtime.
    pub(crate) fn runtime_lifecycle_ordinal(&self) -> Option<u128> {
        self.validate_exact()
            .then_some(self.first.lifecycle_ordinal)
            .flatten()
    }

    /// Physical FIFO position of the one coalesced ingress occurrence.
    pub(crate) fn physical_admission_ordinal(&self) -> Option<u64> {
        self.validate_exact()
            .then_some(self.first.physical_admission_ordinal)
    }

    /// Immutable receiver-local cut frozen by checked dequeue.
    pub(crate) fn runtime_physical_cut(&self) -> Option<u128> {
        self.validate_exact()
            .then_some(self.runtime_physical_cut)
            .flatten()
    }

    /// Freeze the physical predecessor cut exactly once while the selected
    /// fair-ingress carrier is still owned by the dequeue transaction.
    fn freeze_runtime_physical_cut(&mut self, physical_cut: u128) -> bool {
        if !self.validate_exact()
            || self.runtime_physical_cut.is_some()
            || physical_cut <= u128::from(self.first.physical_admission_ordinal)
        {
            return false;
        }
        let mut frozen = self.clone();
        frozen.runtime_physical_cut = Some(physical_cut);
        if !frozen.validate_exact() {
            return false;
        }
        *self = frozen;
        true
    }

    fn install_leader_wire_runtime_receipt(
        &mut self,
        receipt: serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
    ) -> bool {
        if self.leader_wire_runtime_receipt.is_some()
            || self.leader_wire_token.as_ref() != Some(receipt.token())
            || receipt.owner().admission_ordinal() != receipt.token().scheduler_ordinal()
        {
            return false;
        }
        self.leader_wire_runtime_receipt = Some(receipt);
        self.validate_exact()
    }

    /// Whether this carrier was derived from the exact normalized message now
    /// crossing a downstream ownership seam.
    pub(crate) fn matches_message(&self, message: &BlockMessage) -> bool {
        let encoded = match message {
            BlockMessage::V2(message) => message.encode(),
            message if message.is_lane_local() || message.is_live_auxiliary() => message.encode(),
            _ => return false,
        };
        self.first.encoded_bytes.as_ref() == encoded.as_slice()
            && Some(self.first.message_kind) == FairV2IngressMessageKind::classify(message)
    }

    /// Whether the carrier's semantic request origin is the independently
    /// retained inbound sender.
    pub(crate) fn matches_semantic_origin(&self, origin: Option<&PeerId>) -> bool {
        self.validate_exact() && self.first.semantic_origin.as_ref() == origin
    }

    /// Decode the exact canonical v2 envelope retained by this ownership
    /// carrier. The full carrier is validated before any projection is
    /// returned, so downstream code cannot use decoded bytes to bypass route,
    /// resource, or source-history checks.
    pub(crate) fn canonical_v2_message(
        &self,
    ) -> Option<iroha_data_model::block::consensus_v2::ConsensusMessageV2> {
        if !self.validate_exact() || !self.first.message_kind.is_v2() {
            return None;
        }
        let mut cursor = self.first.encoded_bytes.as_ref();
        let message =
            iroha_data_model::block::consensus_v2::ConsensusMessageV2::decode(&mut cursor).ok()?;
        cursor.is_empty().then_some(message)
    }

    /// Process-local integrity projection for carrying this exact ownership
    /// history through the serialized runtime. Pointer-derived source keys are
    /// intentionally included: this hash is never serialized or used by
    /// consensus, and two independent network actors must not alias merely
    /// because their wire bytes and counters match.
    pub(crate) fn process_local_projection_hash(&self) -> CryptoHash {
        let mut projection = Vec::new();
        projection.extend_from_slice(b"iroha:sumeragi:v2:fair-ingress-owner:v8");
        for occurrence in [&self.first, &self.latest] {
            projection.extend_from_slice(&occurrence.physical_admission_ordinal.to_le_bytes());
            match occurrence.lifecycle_ordinal {
                None => projection.push(0),
                Some(ordinal) => {
                    projection.push(1);
                    projection.extend_from_slice(&ordinal.to_le_bytes());
                }
            }
            fair_v2_ingress_append_optional_peer_identity(
                &mut projection,
                occurrence.semantic_origin.as_ref(),
            );
            fair_v2_ingress_append_optional_peer_identity(
                &mut projection,
                occurrence.authenticated_via.as_ref(),
            );
            projection.push(u8::from(occurrence.authenticated_via_is_validator));
            fair_v2_ingress_append_source_identity(
                &mut projection,
                &occurrence.authenticated_source,
            );
            fair_v2_ingress_append_source_identity(
                &mut projection,
                &occurrence.semantic_owner_source,
            );
        }
        match self.runtime_physical_cut {
            None => projection.push(0),
            Some(cut) => {
                projection.push(1);
                projection.extend_from_slice(&cut.to_le_bytes());
            }
        }
        projection.extend_from_slice(&self.admission_count.to_le_bytes());
        projection.extend_from_slice(&self.occurrence_count.to_le_bytes());
        for count in self.action_counts {
            projection.extend_from_slice(&count.to_le_bytes());
        }
        projection.extend_from_slice(self.attempts_hash.as_ref());
        match &self.leader_wire_token {
            None => projection.push(0),
            Some(token) => {
                projection.push(1);
                projection.extend_from_slice(token.identity_hash().as_ref());
                projection.extend_from_slice(&token.admission_ordinal.to_le_bytes());
                projection.extend_from_slice(&token.scheduler_ordinal.to_le_bytes());
                projection.push(match token.source_class {
                    FairV2IngressLeaderWireSourceClass::Control => 0,
                    FairV2IngressLeaderWireSourceClass::Chunk => 1,
                    FairV2IngressLeaderWireSourceClass::CertifiedResponse => 2,
                });
            }
        }
        match &self.leader_wire_runtime_receipt {
            None => projection.push(0),
            Some(receipt) => {
                projection.push(1);
                projection.extend_from_slice(receipt.owner().causal_lifecycle_key().as_ref());
                projection.extend_from_slice(&receipt.owner().admission_ordinal().to_le_bytes());
            }
        }
        projection.extend_from_slice(&self.first.encoded_bytes);
        projection.push(u8::try_from(self.latest.action.index()).unwrap_or(u8::MAX));
        match &self.current_routes {
            None => projection.push(0),
            Some(routes) => {
                projection.push(1);
                projection.extend_from_slice(
                    &u64::try_from(routes.source_capacity())
                        .expect("bounded reply-route source capacity fits u64")
                        .to_le_bytes(),
                );
                projection.extend_from_slice(
                    &u64::try_from(routes.len())
                        .expect("bounded reply-route count fits u64")
                        .to_le_bytes(),
                );
                projection.extend_from_slice(routes.process_local_exact_history_hash().as_ref());
            }
        }
        for attempt in &self.attempts {
            projection.extend_from_slice(attempt.route.process_local_identity_hash().as_ref());
            projection.extend_from_slice(&attempt.message_cursor.to_le_bytes());
            projection.extend_from_slice(&attempt.chunk_cursor.to_le_bytes());
        }
        CryptoHash::new(projection)
    }

    /// Whether the independently carried response routes are exactly the
    /// carrier's current opaque per-source route set.
    pub(crate) fn matches_reply_routes(&self, routes: Option<&NetworkReplyRoutes>) -> bool {
        fair_v2_ingress_route_sets_same_exact_history(self.current_routes.as_ref(), routes)
    }

    /// Current bounded route set after every admitted downstream merge.
    #[cfg(test)]
    pub(crate) const fn current_reply_routes(&self) -> Option<&NetworkReplyRoutes> {
        self.current_routes.as_ref()
    }

    /// Consume one authoritative prune receipt into this carrier.
    ///
    /// The receipt owns the only permitted output history, so the caller
    /// cannot substitute a route set after the liveness snapshot. The returned
    /// routes must replace the caller's independently carried copy.
    pub(crate) fn project_retained_reply_routes(
        &mut self,
        receipt: NetworkReplyRoutesPruneReceipt,
    ) -> Option<NetworkReplyRoutes> {
        if !self.validate_exact() {
            return None;
        }
        let retained = receipt.into_output(self.current_routes.as_ref()?)?;
        let mut projected = self.clone();
        projected.current_routes = Some(retained.clone());
        projected.attempts = fair_v2_ingress_attempts_after_prune(
            &projected.attempts,
            projected.current_routes.as_ref(),
        );
        projected.attempts_hash = fair_v2_ingress_attempt_cursor_hash(&projected.attempts);
        if !projected.validate_exact() || !projected.matches_reply_routes(Some(&retained)) {
            return None;
        }
        *self = projected;
        Some(retained)
    }

    /// Most recent ownership action retained by this queued semantic owner.
    #[cfg(test)]
    pub(crate) const fn latest_action(&self) -> FairV2IngressOwnershipAction {
        self.latest.action
    }

    /// Advance one retained source's downstream cursors without allowing
    /// either message or chunk progress to regress.
    pub(crate) fn advance_reply_cursors(
        &mut self,
        route: &NetworkReplyRoute,
        message_cursor: u64,
        chunk_cursor: u64,
    ) -> bool {
        let Some(attempt) = self
            .attempts
            .iter_mut()
            .find(|attempt| attempt.route.same_delivery(route))
        else {
            return false;
        };
        if message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor {
            return false;
        }
        attempt.message_cursor = message_cursor;
        attempt.chunk_cursor = chunk_cursor;
        self.attempts_hash = fair_v2_ingress_attempt_cursor_hash(&self.attempts);
        true
    }

    /// Validate the exact semantic bytes, bounded accounting, route ownership,
    /// and non-regressing per-source cursor relation.
    pub(crate) fn validate_exact(&self) -> bool {
        self.first.action == FairV2IngressOwnershipAction::New
            && self.admission_count != 0
            && self.action_counts[FairV2IngressOwnershipAction::New.index()] == self.admission_count
            && self
                .action_counts
                .iter()
                .copied()
                .try_fold(0u128, |total, count| total.checked_add(count))
                == Some(self.occurrence_count)
            && self.action_counts[self.latest.action.index()] != 0
            && self.first.physical_admission_ordinal != 0
            && self.first.physical_admission_ordinal == self.latest.physical_admission_ordinal
            && self
                .runtime_physical_cut
                .is_none_or(|cut| u128::from(self.first.physical_admission_ordinal) < cut)
            && self.first.lifecycle_ordinal == self.latest.lifecycle_ordinal
            && self
                .first
                .lifecycle_ordinal
                .is_none_or(|ordinal| ordinal != 0)
            && self.first.wire_key == self.latest.wire_key
            && self.first.semantic_origin == self.latest.semantic_origin
            && self.first.authenticated_source == self.first.semantic_owner_source
            && (self.admission_count != 1
                || self.first.semantic_owner_source == self.latest.semantic_owner_source)
            && self.first.message_kind == self.latest.message_kind
            && self.first.class == self.latest.class
            && self.first.encoded_bytes.as_ref() == self.latest.encoded_bytes.as_ref()
            && self.first.encoded_len == self.latest.encoded_len
            && self.leader_wire_token.as_ref().is_none_or(|token| {
                self.first
                    .semantic_origin
                    .as_ref()
                    .is_some_and(|origin| &token.identity.semantic_origin == origin)
                    && token.identity.canonical_wire_hash == self.first.wire_key.hash
                    && token.slot.semantic_origin == token.identity.semantic_origin
                    && token.slot.phase == token.identity.phase
                    && token.source_class == token.identity.phase.source_class()
                    && (token.source_class == FairV2IngressLeaderWireSourceClass::Chunk)
                        == token.slot.chunk_index.is_some()
                    && token.admission_ordinal != 0
                    && token.scheduler_ordinal != 0
                    && self.first.lifecycle_ordinal == Some(token.scheduler_ordinal)
            })
            && match (
                self.leader_wire_token.as_ref(),
                self.leader_wire_runtime_receipt.as_ref(),
            ) {
                (None, None) | (Some(_), None) => true,
                (Some(token), Some(receipt)) => {
                    receipt.token() == token
                        && receipt.owner().causal_lifecycle_key() == token.identity_hash()
                        && receipt.owner().admission_ordinal() == token.scheduler_ordinal()
                }
                (None, Some(_)) => false,
            }
            && self.first.validate_exact()
            && self.latest.validate_exact()
            && self.attempts_hash == fair_v2_ingress_attempt_cursor_hash(&self.attempts)
            && fair_v2_ingress_attempts_cover_latest(&self.latest.attempts_after, &self.attempts)
            && fair_v2_ingress_carrier_attempts_match_routes(
                &self.attempts,
                self.current_routes.as_ref(),
            )
    }
}

impl FairV2IngressOwnershipOccurrence {
    fn validate_exact(&self) -> bool {
        let mut cursor = self.encoded_bytes.as_ref();
        let decoded = if self.message_kind.is_v2() {
            let Ok(message) =
                iroha_data_model::block::consensus_v2::ConsensusMessageV2::decode(&mut cursor)
            else {
                return false;
            };
            BlockMessage::V2(message)
        } else {
            let Ok(message) = BlockMessage::decode(&mut cursor) else {
                return false;
            };
            message
        };
        let decoded_class = FairV2IngressClass::classify_message(&decoded);
        let decoded_kind = FairV2IngressMessageKind::classify(&decoded);
        let is_timeout_vote = fair_v2_ingress_message_is_timeout_vote(&decoded);
        let is_certified_fence_escape = fair_v2_ingress_message_is_certified_fence_escape(&decoded);
        let is_transport_completion = self.class == FairV2IngressClass::TransportCompletion;
        let uses_certified_fence_escape_reserve = is_certified_fence_escape
            && !matches!(&self.authenticated_source, FairV2IngressSource::Anonymous);
        let semantic_exact = self.wire_key.origin == self.semantic_origin
            && self.wire_key.hash == CryptoHash::new(self.encoded_bytes.as_ref())
            && self.encoded_len == self.encoded_bytes.len()
            && cursor.is_empty()
            && decoded_class == self.class
            && decoded_kind == Some(self.message_kind);
        let source_exact = match (&self.authenticated_source, &self.authenticated_via) {
            (FairV2IngressSource::Validator(source), Some(via)) => {
                self.authenticated_via_is_validator && source == via
            }
            (FairV2IngressSource::Authenticated(source), Some(via)) => {
                !self.authenticated_via_is_validator && source == via
            }
            (FairV2IngressSource::Anonymous, None) => !self.authenticated_via_is_validator,
            (FairV2IngressSource::Validator(_) | FairV2IngressSource::Authenticated(_), None)
            | (FairV2IngressSource::Anonymous, Some(_)) => false,
        };
        let capacities_exact = self.resource_before.message_capacity
            == self.resource_after.message_capacity
            && self.resource_before.global_byte_capacity
                == self.resource_after.global_byte_capacity
            && self.resource_before.source_byte_capacity
                == self.resource_after.source_byte_capacity
            && self.resource_before.certified_fence_escape_byte_reserve
                == self.resource_after.certified_fence_escape_byte_reserve
            && self.resource_before.timeout_vote_byte_reserve
                == self.resource_after.timeout_vote_byte_reserve
            && self.resource_before.transport_completion_byte_reserve
                == self.resource_after.transport_completion_byte_reserve
            && self.resource_before.global_len <= self.resource_before.message_capacity
            && self.resource_before.global_bytes <= self.resource_before.global_byte_capacity
            && self.resource_before.source_bytes <= self.resource_before.source_byte_capacity
            && self.resource_before.source_progress_len <= self.resource_before.source_len
            && self.resource_before.source_certified_fence_escape_len
                <= self.resource_before.source_progress_len
            && self.resource_before.source_timeout_vote_len
                <= self.resource_before.source_progress_len
            && self.resource_before.source_transport_completion_len
                <= self.resource_before.source_len
            && self.resource_after.global_len <= self.resource_after.message_capacity
            && self.resource_after.global_bytes <= self.resource_after.global_byte_capacity
            && self.resource_after.source_bytes <= self.resource_after.source_byte_capacity
            && self.resource_after.source_progress_len <= self.resource_after.source_len
            && self.resource_after.source_certified_fence_escape_len
                <= self.resource_after.source_progress_len
            && self.resource_after.source_timeout_vote_len
                <= self.resource_after.source_progress_len
            && self.resource_after.source_transport_completion_len
                <= self.resource_after.source_len
            && self.resource_before.source_timeout_vote_bytes
                <= self.resource_before.timeout_vote_byte_reserve
            && self.resource_after.source_timeout_vote_bytes
                <= self.resource_after.timeout_vote_byte_reserve
            && self.resource_before.source_certified_fence_escape_bytes
                <= self.resource_before.certified_fence_escape_byte_reserve
            && self.resource_after.source_certified_fence_escape_bytes
                <= self.resource_after.certified_fence_escape_byte_reserve
            && self.resource_before.source_transport_completion_bytes
                <= self.resource_before.transport_completion_byte_reserve
            && self.resource_after.source_transport_completion_bytes
                <= self.resource_after.transport_completion_byte_reserve
            && self.resource_before.source_timeout_vote_bytes <= self.resource_before.source_bytes
            && self.resource_after.source_timeout_vote_bytes <= self.resource_after.source_bytes
            && self.resource_before.source_certified_fence_escape_bytes
                <= self.resource_before.source_bytes
            && self.resource_after.source_certified_fence_escape_bytes
                <= self.resource_after.source_bytes
            && self.resource_before.source_transport_completion_bytes
                <= self.resource_before.source_bytes
            && self.resource_after.source_transport_completion_bytes
                <= self.resource_after.source_bytes
            && self
                .resource_before
                .global_len
                .checked_add(self.resource_before.protected_slots)
                .is_some_and(|owned| owned <= self.resource_before.message_capacity)
            && self
                .resource_after
                .global_len
                .checked_add(self.resource_after.protected_slots)
                .is_some_and(|owned| owned <= self.resource_after.message_capacity);
        let exact_add = |before: usize, increment: usize, after: usize| {
            before.checked_add(increment) == Some(after)
        };
        let resource_exact = capacities_exact
            && match self.action {
                FairV2IngressOwnershipAction::New => {
                    exact_add(
                        self.resource_before.global_len,
                        1,
                        self.resource_after.global_len,
                    ) && exact_add(
                        self.resource_before.global_bytes,
                        self.encoded_len,
                        self.resource_after.global_bytes,
                    ) && exact_add(
                        self.resource_before.source_len,
                        1,
                        self.resource_after.source_len,
                    ) && exact_add(
                        self.resource_before.source_bytes,
                        self.encoded_len,
                        self.resource_after.source_bytes,
                    ) && exact_add(
                        self.resource_before.source_progress_len,
                        usize::from(self.class == FairV2IngressClass::Progress),
                        self.resource_after.source_progress_len,
                    ) && exact_add(
                        self.resource_before.source_certified_fence_escape_len,
                        usize::from(uses_certified_fence_escape_reserve),
                        self.resource_after.source_certified_fence_escape_len,
                    ) && exact_add(
                        self.resource_before.source_timeout_vote_len,
                        usize::from(is_timeout_vote),
                        self.resource_after.source_timeout_vote_len,
                    ) && exact_add(
                        self.resource_before.source_transport_completion_len,
                        usize::from(is_transport_completion),
                        self.resource_after.source_transport_completion_len,
                    ) && exact_add(
                        self.resource_before.source_certified_fence_escape_bytes,
                        if uses_certified_fence_escape_reserve {
                            self.encoded_len
                        } else {
                            0
                        },
                        self.resource_after.source_certified_fence_escape_bytes,
                    ) && exact_add(
                        self.resource_before.source_timeout_vote_bytes,
                        if is_timeout_vote
                            && matches!(
                                &self.authenticated_source,
                                FairV2IngressSource::Validator(_)
                            )
                        {
                            self.encoded_len
                        } else {
                            0
                        },
                        self.resource_after.source_timeout_vote_bytes,
                    ) && exact_add(
                        self.resource_before.source_transport_completion_bytes,
                        if is_transport_completion {
                            self.encoded_len
                        } else {
                            0
                        },
                        self.resource_after.source_transport_completion_bytes,
                    ) && self.authenticated_source == self.semantic_owner_source
                }
                _ => self.resource_before == self.resource_after,
            };
        let routes_bind_semantic_origin = self.semantic_origin.as_ref().is_none_or(|origin| {
            self.routes_before
                .iter()
                .chain(self.routes_candidate.iter())
                .chain(self.routes_after.iter())
                .all(|routes| routes.semantic_target() == origin)
        });
        let candidate_binds_authenticated_via = self.authenticated_via.as_ref().map_or_else(
            || self.routes_candidate.is_none(),
            |via| {
                self.routes_candidate
                    .iter()
                    .flat_map(|routes| routes.iter())
                    .all(|route| route.is_authenticated_via(via))
            },
        );
        let candidate_is_retained = self.routes_candidate.iter().all(|candidate| {
            candidate.iter().all(|route| {
                self.routes_after
                    .iter()
                    .flat_map(|routes| routes.iter())
                    .any(|retained| route.same_delivery(retained))
            })
        });
        let route_exact = fair_v2_ingress_route_capacity(
            self.routes_before.as_ref(),
            self.routes_candidate.as_ref(),
            self.routes_after.as_ref(),
        ) == Some(self.route_capacity)
            && self.route_capacity.is_none_or(|capacity| {
                self.routes_after
                    .as_ref()
                    .is_none_or(|routes| routes.len() <= capacity)
            })
            && routes_bind_semantic_origin
            && candidate_binds_authenticated_via
            && candidate_is_retained
            && fair_v2_ingress_action_is_structurally_exact(
                self.action,
                self.routes_before.as_ref(),
                self.routes_candidate.as_ref(),
                self.routes_after.as_ref(),
                &self.attempts_before,
            )
            && fair_v2_ingress_attempts_preserve_cursors(
                &self.attempts_before,
                &self.attempts_after,
            )
            && self.attempts_before_hash
                == fair_v2_ingress_attempt_cursor_hash(&self.attempts_before)
            && self.attempts_after_hash
                == fair_v2_ingress_attempt_cursor_hash(&self.attempts_after)
            && fair_v2_ingress_carrier_attempts_match_routes(
                &self.attempts_before,
                self.routes_before.as_ref(),
            )
            && fair_v2_ingress_carrier_attempts_match_routes(
                &self.attempts_after,
                self.routes_after.as_ref(),
            );
        semantic_exact && source_exact && resource_exact && route_exact
    }
}

impl FairV2IngressClass {
    fn classify(inbound: &InboundBlockMessage) -> Self {
        Self::classify_message(inbound.message())
    }

    fn classify_message(message: &BlockMessage) -> Self {
        let BlockMessage::V2(message) = message else {
            return match message {
                BlockMessage::LaneExecutablePayload(_)
                | BlockMessage::LaneHistoricalRecoveryResponse(_) => Self::TransportCompletion,
                message if message.is_lane_local() => Self::Progress,
                _ => Self::Auxiliary,
            };
        };
        use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

        match &message.payload {
            ConsensusMessageV2Payload::Vote(vote)
                if vote.phase == iroha_data_model::block::consensus_v2::GlobalPhase::Commit =>
            {
                Self::Progress
            }
            ConsensusMessageV2Payload::QuorumCertificate(_)
            | ConsensusMessageV2Payload::TimeoutCertificate(_)
            | ConsensusMessageV2Payload::TimeoutVote(_)
            | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | ConsensusMessageV2Payload::VrfCommit(_)
            | ConsensusMessageV2Payload::VrfReveal(_) => Self::Progress,
            ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_) => Self::TransportCompletion,
            ConsensusMessageV2Payload::Proposal(_)
            | ConsensusMessageV2Payload::Vote(_)
            | ConsensusMessageV2Payload::PayloadManifest(_) => Self::Auxiliary,
        }
    }
}

fn fair_v2_ingress_route_action(
    retained_attempts: &[FairV2IngressReplyAttempt],
    candidate: Option<&NetworkReplyRoutes>,
) -> Result<FairV2IngressOwnershipAction, NetworkReplyRouteError> {
    let Some(candidate) = candidate else {
        return Ok(FairV2IngressOwnershipAction::ExactDuplicate);
    };
    let mut action = FairV2IngressOwnershipAction::ExactDuplicate;
    for route in candidate.iter() {
        let prior = retained_attempts
            .iter()
            .map(|attempt| &attempt.route)
            .find(|prior| route.same_source(prior));
        let Some(prior) = prior else {
            action = FairV2IngressOwnershipAction::NewAlternateSource;
            continue;
        };
        let update = route.source_update_from(prior)?;
        action = match (action, update) {
            (FairV2IngressOwnershipAction::NewAlternateSource, _) => action,
            (_, NetworkReplyRouteSourceUpdate::Reconnected) => {
                FairV2IngressOwnershipAction::Reconnect
            }
            (
                FairV2IngressOwnershipAction::ExactDuplicate
                | FairV2IngressOwnershipAction::SameSourceLaterDelivery,
                NetworkReplyRouteSourceUpdate::LaterDelivery,
            ) => FairV2IngressOwnershipAction::SameSourceLaterDelivery,
            (_, NetworkReplyRouteSourceUpdate::Exact) => action,
            (FairV2IngressOwnershipAction::Reconnect, _) => action,
            (FairV2IngressOwnershipAction::New, _) => action,
        };
    }
    Ok(action)
}

fn fair_v2_ingress_route_capacity(
    before: Option<&NetworkReplyRoutes>,
    candidate: Option<&NetworkReplyRoutes>,
    after: Option<&NetworkReplyRoutes>,
) -> Option<Option<usize>> {
    let mut capacities = before
        .into_iter()
        .chain(candidate)
        .chain(after)
        .map(NetworkReplyRoutes::source_capacity);
    let Some(first) = capacities.next() else {
        return Some(None);
    };
    capacities
        .all(|capacity| capacity == first)
        .then_some(Some(first))
}

fn fair_v2_ingress_route_sets_same_exact_history(
    left: Option<&NetworkReplyRoutes>,
    right: Option<&NetworkReplyRoutes>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left.has_same_exact_history(right),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn fair_v2_ingress_action_is_structurally_exact(
    action: FairV2IngressOwnershipAction,
    before: Option<&NetworkReplyRoutes>,
    candidate: Option<&NetworkReplyRoutes>,
    after: Option<&NetworkReplyRoutes>,
    attempts_before: &[FairV2IngressReplyAttempt],
) -> bool {
    match action {
        FairV2IngressOwnershipAction::New => {
            before.is_none() && fair_v2_ingress_route_sets_same_exact_history(candidate, after)
        }
        FairV2IngressOwnershipAction::ExactDuplicate => candidate.is_none_or(|candidate| {
            candidate.iter().all(|route| {
                attempts_before
                    .iter()
                    .any(|prior| route.same_delivery(&prior.route))
            })
        }),
        FairV2IngressOwnershipAction::SameSourceLaterDelivery => {
            candidate.is_some_and(|candidate| {
                candidate.iter().any(|route| {
                    attempts_before
                        .iter()
                        .map(|attempt| &attempt.route)
                        .any(|prior| {
                            route.same_source(prior)
                                && route.same_tenure(prior)
                                && !route.same_delivery(prior)
                        })
                })
            })
        }
        FairV2IngressOwnershipAction::Reconnect => candidate.is_some_and(|candidate| {
            candidate.iter().any(|route| {
                attempts_before
                    .iter()
                    .map(|attempt| &attempt.route)
                    .any(|prior| route.same_source(prior) && !route.same_tenure(prior))
            })
        }),
        FairV2IngressOwnershipAction::NewAlternateSource => candidate.is_some_and(|candidate| {
            candidate.iter().any(|route| {
                !attempts_before
                    .iter()
                    .map(|attempt| &attempt.route)
                    .any(|prior| route.same_source(prior))
            })
        }),
    }
}

fn fair_v2_ingress_attempts_for_routes(
    before: &[FairV2IngressReplyAttempt],
    routes: Option<&NetworkReplyRoutes>,
) -> Vec<FairV2IngressReplyAttempt> {
    routes
        .into_iter()
        .flat_map(|routes| routes.iter())
        .map(|route| {
            before
                .iter()
                .find(|attempt| attempt.route.same_source(route))
                .map_or_else(
                    || FairV2IngressReplyAttempt {
                        route: route.clone(),
                        message_cursor: 0,
                        chunk_cursor: 0,
                    },
                    |attempt| FairV2IngressReplyAttempt {
                        route: route.clone(),
                        message_cursor: attempt.message_cursor,
                        chunk_cursor: attempt.chunk_cursor,
                    },
                )
        })
        .collect()
}

fn fair_v2_ingress_attempts_after_prune(
    before: &[FairV2IngressReplyAttempt],
    routes: Option<&NetworkReplyRoutes>,
) -> Vec<FairV2IngressReplyAttempt> {
    before
        .iter()
        .map(|attempt| {
            routes
                .into_iter()
                .flat_map(NetworkReplyRoutes::iter)
                .find(|route| route.same_source(&attempt.route))
                .map_or_else(
                    || attempt.clone(),
                    |route| FairV2IngressReplyAttempt {
                        route: route.clone(),
                        message_cursor: attempt.message_cursor,
                        chunk_cursor: attempt.chunk_cursor,
                    },
                )
        })
        .collect()
}

fn fair_v2_ingress_attempts_preserve_cursors(
    before: &[FairV2IngressReplyAttempt],
    after: &[FairV2IngressReplyAttempt],
) -> bool {
    before.iter().all(|prior| {
        after
            .iter()
            .find(|attempt| attempt.route.same_source(&prior.route))
            .is_some_and(|attempt| {
                attempt.message_cursor == prior.message_cursor
                    && attempt.chunk_cursor == prior.chunk_cursor
            })
    }) && after.iter().all(|attempt| {
        before
            .iter()
            .any(|prior| prior.route.same_source(&attempt.route))
            || (attempt.message_cursor == 0 && attempt.chunk_cursor == 0)
    })
}

fn fair_v2_ingress_attempts_cover_latest(
    before: &[FairV2IngressReplyAttempt],
    after: &[FairV2IngressReplyAttempt],
) -> bool {
    before.iter().all(|prior| {
        after
            .iter()
            .find(|attempt| attempt.route.same_source(&prior.route))
            .is_some_and(|attempt| {
                attempt.message_cursor >= prior.message_cursor
                    && attempt.chunk_cursor >= prior.chunk_cursor
            })
    })
}

fn fair_v2_ingress_merge_attempt_cursors(
    retained: &[FairV2IngressReplyAttempt],
    candidate: &[FairV2IngressReplyAttempt],
    routes: Option<&NetworkReplyRoutes>,
) -> Option<Vec<FairV2IngressReplyAttempt>> {
    let mut merged =
        BTreeMap::<iroha_p2p::network::NetworkReplySourceKey, FairV2IngressReplyAttempt>::new();
    for attempt in retained.iter().chain(candidate) {
        let source = attempt.route.source_key();
        if let Some(current) = merged.get_mut(&source) {
            current.message_cursor = current.message_cursor.max(attempt.message_cursor);
            current.chunk_cursor = current.chunk_cursor.max(attempt.chunk_cursor);
        } else {
            merged.insert(source, attempt.clone());
        }
    }
    if let Some(routes) = routes {
        for route in routes.iter() {
            let source = route.source_key();
            if let Some(attempt) = merged.get_mut(&source) {
                attempt.route = route.clone();
            } else {
                merged.insert(
                    source,
                    FairV2IngressReplyAttempt {
                        route: route.clone(),
                        message_cursor: 0,
                        chunk_cursor: 0,
                    },
                );
            }
        }
        if merged.len() > routes.source_capacity() {
            return None;
        }
    } else if !merged.is_empty() {
        return None;
    }
    Some(merged.into_values().collect())
}

fn fair_v2_ingress_attempt_cursor_hash(attempts: &[FairV2IngressReplyAttempt]) -> CryptoHash {
    let mut projection = Vec::with_capacity(24usize.saturating_mul(attempts.len()));
    projection.extend_from_slice(b"iroha:sumeragi:v2:fair-ingress-cursors:v2");
    let count = u64::try_from(attempts.len())
        .expect("bounded fair-ingress route count is representable as u64");
    projection.extend_from_slice(&count.to_le_bytes());
    for attempt in attempts {
        projection.extend_from_slice(attempt.route.process_local_identity_hash().as_ref());
        projection.extend_from_slice(&attempt.message_cursor.to_le_bytes());
        projection.extend_from_slice(&attempt.chunk_cursor.to_le_bytes());
    }
    CryptoHash::new(projection)
}

fn fair_v2_ingress_carrier_attempts_match_routes(
    attempts: &[FairV2IngressReplyAttempt],
    routes: Option<&NetworkReplyRoutes>,
) -> bool {
    let Some(routes) = routes else {
        return attempts.is_empty();
    };
    if attempts.len() > routes.source_capacity()
        || attempts.iter().enumerate().any(|(index, attempt)| {
            attempts[index + 1..]
                .iter()
                .any(|other| attempt.route.same_source(&other.route))
        })
    {
        return false;
    }
    routes.iter().all(|route| {
        attempts
            .iter()
            .any(|attempt| attempt.route.same_delivery(route))
    }) && attempts.iter().all(|attempt| {
        routes
            .iter()
            .find(|route| route.same_source(&attempt.route))
            .map_or_else(
                || !attempt.route.is_active(),
                |route| attempt.route.same_delivery(route),
            )
    })
}

fn fair_v2_ingress_current_protected_slots(
    state: &FairV2IngressState,
    authenticated_non_validator_source_capacity: Option<usize>,
) -> usize {
    let materialized = state
        .lanes
        .iter()
        .map(|(source, lane)| {
            fair_v2_ingress_lane_protected_slots(
                source.class(),
                !state.roster.is_empty(),
                lane.entries.len(),
                lane.progress_len
                    .saturating_sub(lane.timeout_vote_len)
                    .saturating_sub(lane.certified_fence_escape_len)
                    != 0,
                lane.certified_fence_escape_len != 0,
                lane.timeout_vote_len != 0,
                lane.transport_completion_len != 0,
            )
        })
        .sum::<usize>();
    let latent_authenticated = authenticated_non_validator_source_capacity.map_or(0, |capacity| {
        let materialized_authenticated = state
            .lanes
            .keys()
            .filter(|source| matches!(source, FairV2IngressSource::Authenticated(_)))
            .count();
        capacity
            .checked_sub(materialized_authenticated)
            .and_then(|latent| latent.checked_mul(3))
            .expect("configured authenticated-source geometry contains every materialized lane")
    });
    materialized
        .checked_add(latent_authenticated)
        .expect("configured fair-ingress protected-slot geometry is representable")
}

fn fair_v2_ingress_is_timeout_vote(inbound: &InboundBlockMessage) -> bool {
    fair_v2_ingress_message_is_timeout_vote(inbound.message())
}

/// Whether one queued occurrence is the exact pre-runtime TimeoutVote owner
/// delivered directly by its authenticated validator source.
///
/// This is only a selector prerequisite. The serialized runtime still checks
/// the current episode, signer index, signature, frozen roster slot, and
/// ordinary Progress capacity before the occurrence can leave fair ingress.
fn fair_v2_ingress_is_direct_validator_timeout_vote_owner(
    source: &FairV2IngressSource,
    entry: &FairV2IngressEntry,
) -> bool {
    let FairV2IngressSource::Validator(authenticated_source) = source else {
        return false;
    };
    let Some(token) = entry.leader_wire_token.as_ref() else {
        return false;
    };
    let Some(ownership) = entry.inbound.ingress_ownership() else {
        return false;
    };
    fair_v2_ingress_is_timeout_vote(&entry.inbound)
        && entry.inbound.sender() == Some(authenticated_source)
        && entry.inbound.via() == Some(authenticated_source)
        && token.identity.phase == FairV2IngressLeaderWirePhase::TimeoutVote
        && token.source_class == FairV2IngressLeaderWireSourceClass::Control
        && token.identity.semantic_origin == *authenticated_source
        && token.slot.semantic_origin == *authenticated_source
        && ownership.validate_exact()
        && ownership.leader_wire_token() == Some(token)
        && ownership.leader_wire_runtime_receipt().is_none()
        && ownership.runtime_physical_cut().is_none()
        && ownership.physical_admission_ordinal() == Some(entry.admission_ordinal)
}

fn fair_v2_ingress_message_is_timeout_vote(message: &BlockMessage) -> bool {
    matches!(
        message,
        BlockMessage::V2(message)
            if matches!(
                &message.payload,
                iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload::TimeoutVote(_)
            )
    )
}

#[cfg(test)]
fn fair_v2_ingress_is_transport_completion(inbound: &InboundBlockMessage) -> bool {
    FairV2IngressClass::classify(inbound) == FairV2IngressClass::TransportCompletion
}

/// Point-in-time occupancy of the bounded transport-to-runner v2 ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FairV2IngressSnapshot {
    /// Number of messages currently owned by all fair source lanes.
    pub(crate) depth: usize,
    /// Fixed total capacity shared by the fair source lanes.
    pub(crate) capacity: usize,
    /// Local monotonic age of the oldest queued message.
    pub(crate) oldest_age: Option<Duration>,
    /// Local monotonic age since the runner last scanned this ownership interval.
    ///
    /// Before its first scan, this is the age of the non-empty interval rather
    /// than the age of an earlier empty-queue scheduler turn.
    pub(crate) service_idle_age: Option<Duration>,
}

/// Invalid message or byte capacity for the active roster's fair v2 ingress lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FairV2IngressCapacityError {
    configured: usize,
    required: usize,
    kind: FairV2IngressCapacityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressCapacityKind {
    Messages,
    CertifiedServeGate,
    LeaderWireLifecycleGate,
    Bytes,
    CertifiedFenceEscapeBytes,
    TimeoutVoteBytes,
    OrdinaryBytes,
    TransportCompletionBytes,
    ConsensusFrameBytes,
    ControlFrameBytes,
    BlockSyncFrameBytes,
    OutboundHighFrameBytes,
}

impl FairV2IngressCapacityError {
    /// Configured capacity in the rejected unit.
    pub(crate) const fn configured(self) -> usize {
        self.configured
    }

    /// Minimum capacity needed for the active roster in the rejected unit.
    pub(crate) const fn required(self) -> usize {
        self.required
    }

    /// Whether the rejected reservation is measured in canonical wire bytes.
    pub(crate) const fn is_bytes(self) -> bool {
        matches!(
            self.kind,
            FairV2IngressCapacityKind::Bytes
                | FairV2IngressCapacityKind::CertifiedFenceEscapeBytes
                | FairV2IngressCapacityKind::TimeoutVoteBytes
                | FairV2IngressCapacityKind::OrdinaryBytes
                | FairV2IngressCapacityKind::TransportCompletionBytes
                | FairV2IngressCapacityKind::ConsensusFrameBytes
                | FairV2IngressCapacityKind::ControlFrameBytes
                | FairV2IngressCapacityKind::BlockSyncFrameBytes
                | FairV2IngressCapacityKind::OutboundHighFrameBytes
        )
    }
}

fn fair_v2_ingress_required_capacity(
    roster_len: usize,
    authenticated_non_validator_source_capacity: Option<usize>,
) -> Option<usize> {
    let Some(authenticated_non_validator_source_capacity) =
        authenticated_non_validator_source_capacity
    else {
        if roster_len == 0 {
            return Some(1);
        }
        return roster_len
            .checked_mul(5)
            .and_then(|required| required.checked_add(2));
    };
    let anonymous_slots = if roster_len == 0 { 1 } else { 2 };
    roster_len
        .checked_mul(5)
        .and_then(|required| {
            authenticated_non_validator_source_capacity
                .checked_mul(3)
                .and_then(|authenticated_sources| required.checked_add(authenticated_sources))
        })
        .and_then(|required| required.checked_add(anonymous_slots))
}

fn fair_v2_ingress_reserve_ordinary_lifecycle_ordinal(
    state: &FairV2IngressState,
) -> Result<Option<u128>, String> {
    let leader_source = state.leader_wire_lifecycle_ordinals.as_ref();
    let serve_gate = state.certified_serve_gate.as_ref();
    if let (Some(source), Some(gate)) = (leader_source, serve_gate)
        && !gate.shares_lifecycle_ordinals(source)
    {
        return Err("fair ingress gates changed their shared lifecycle ordinal source".to_owned());
    }
    if let Some(source) = leader_source {
        return source.reserve_one().map(Some);
    }
    if let Some(gate) = serve_gate {
        return gate.reserve_ordinary_lifecycle_ordinal().map(Some);
    }
    if state.requires_certified_serve_gate || state.requires_leader_wire_lifecycle_gate {
        return Err("production fair ingress lost its lifecycle ordinal source".to_owned());
    }
    Ok(None)
}

const fn fair_v2_ingress_lane_protected_slots(
    source_class: FairV2IngressSourceClass,
    reserve_anonymous_completion: bool,
    depth: usize,
    has_ordinary_progress: bool,
    has_certified_fence_escape: bool,
    has_timeout_vote: bool,
    has_transport_completion: bool,
) -> usize {
    if matches!(source_class, FairV2IngressSourceClass::Anonymous) {
        if !reserve_anonymous_completion {
            return if depth == 0 { 1 } else { 0 };
        }
        let missing_transport_completion = if has_transport_completion { 0 } else { 1 };
        let missing_generic_slots = 2_usize.saturating_sub(depth);
        return if missing_generic_slots > missing_transport_completion {
            missing_generic_slots
        } else {
            missing_transport_completion
        };
    }
    if matches!(source_class, FairV2IngressSourceClass::Authenticated) {
        let missing_certified_fence_escape = if has_certified_fence_escape { 0 } else { 1 };
        let missing_transport_completion = if has_transport_completion { 0 } else { 1 };
        let missing_classes = missing_certified_fence_escape + missing_transport_completion;
        let missing_generic_slots = 3_usize.saturating_sub(depth);
        return if missing_generic_slots > missing_classes {
            missing_generic_slots
        } else {
            missing_classes
        };
    }
    let missing_ordinary_progress = if has_ordinary_progress { 0 } else { 1 };
    let missing_certified_fence_escape = if has_certified_fence_escape { 0 } else { 1 };
    let missing_timeout_vote = if has_timeout_vote { 0 } else { 1 };
    let missing_transport_completion = if has_transport_completion { 0 } else { 1 };
    let missing_classes = missing_ordinary_progress
        + missing_certified_fence_escape
        + missing_timeout_vote
        + missing_transport_completion;
    let missing_generic_slots = 5_usize.saturating_sub(depth);
    if missing_generic_slots > missing_classes {
        missing_generic_slots
    } else {
        missing_classes
    }
}

fn fair_v2_ingress_required_byte_capacity(
    roster_len: usize,
    authenticated_non_validator_source_capacity: Option<usize>,
    source_byte_capacity: usize,
) -> Option<usize> {
    roster_len
        .checked_add(authenticated_non_validator_source_capacity.unwrap_or(0))
        .and_then(|source_count| source_count.checked_add(1))
        .and_then(|source_count| source_count.checked_mul(source_byte_capacity))
}

fn fair_v2_ingress_compact_len_prefix_bytes(value: usize) -> Option<usize> {
    let value = u64::try_from(value).ok()?;
    let significant_bits = (u64::BITS - value.leading_zeros()).max(1);
    usize::try_from(significant_bits.div_ceil(7)).ok()
}

fn fair_v2_ingress_framed_bytes(payload_bytes: usize) -> Option<usize> {
    fair_v2_ingress_compact_len_prefix_bytes(payload_bytes)?.checked_add(payload_bytes)
}

fn fair_v2_ingress_required_manifest_bytes(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
) -> Option<usize> {
    let chunk_count = usize::try_from(layout.max_chunk_count).ok()?;
    let hash_element_bytes = CryptoHash::LENGTH.checked_add(
        fair_v2_ingress_compact_len_prefix_bytes(CryptoHash::LENGTH)?,
    )?;
    let hash_sequence_bytes = chunk_count
        .checked_mul(hash_element_bytes)?
        .checked_add(8)?;
    fair_v2_ingress_framed_bytes(hash_sequence_bytes)?.checked_add(228)
}

fn fair_v2_ingress_required_quorum_certificate_bytes(roster_len: usize) -> Option<usize> {
    let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
    let signer_vector_bytes = roster_len.checked_mul(5)?.checked_add(8)?;
    let signature_vector_bytes = signature_bytes.checked_add(8)?;
    53_usize
        .checked_add(53)?
        .checked_add(5)?
        .checked_add(102)?
        .checked_add(fair_v2_ingress_framed_bytes(213)?)?
        .checked_add(fair_v2_ingress_framed_bytes(signer_vector_bytes)?)?
        .checked_add(fair_v2_ingress_framed_bytes(signature_vector_bytes)?)
}

/// Exact bare-envelope ceiling for every certified signer-fence escape.
///
/// This covers a direct CommitQC, a TC with one distinct highest-QC group per
/// validator, and a durable CommitQC recovery response. It deliberately does
/// not include Proposal bytes: Proposal remains ordinary progress and cannot
/// spend this isolated partition.
fn fair_v2_ingress_required_certified_fence_escape_bytes(roster_len: usize) -> usize {
    let required = || -> Option<usize> {
        let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
        let quorum_certificate_bytes =
            fair_v2_ingress_required_quorum_certificate_bytes(roster_len)?;
        let direct_quorum_certificate =
            fair_v2_ingress_v2_envelope_bytes(quorum_certificate_bytes)?;

        let optional_quorum_certificate_bytes =
            fair_v2_ingress_framed_bytes(quorum_certificate_bytes)?.checked_add(1)?;
        let signature_vector_bytes = fair_v2_ingress_framed_bytes(signature_bytes.checked_add(8)?)?;
        let timeout_group_bytes = fair_v2_ingress_framed_bytes(optional_quorum_certificate_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(13)?)?
            .checked_add(signature_vector_bytes)?;
        let timeout_group_vector_bytes = roster_len
            .checked_mul(fair_v2_ingress_framed_bytes(timeout_group_bytes)?)?
            .checked_add(8)?;
        let timeout_certificate_bytes =
            fair_v2_ingress_framed_bytes(timeout_group_vector_bytes)?.checked_add(53)?;
        let timeout_certificate = fair_v2_ingress_v2_envelope_bytes(timeout_certificate_bytes)?;
        let recovery_response =
            fair_v2_ingress_required_commit_certificate_response_bytes(roster_len);
        Some(
            direct_quorum_certificate
                .max(timeout_certificate)
                .max(recovery_response),
        )
    };
    required().unwrap_or(usize::MAX)
}

/// Exact canonical-wire ceiling for a proposal under one frozen context.
///
/// The checked calculation mirrors bare Norito's fixed v1 layout. A maximal
/// proposal carries a timeout certificate with one non-empty timeout group per
/// validator; each group carries a full PrepareQC, while the proposal's highest
/// QC also carries every validator. `F(x)` is one compact length prefix plus
/// `x` payload bytes. Overflow maps to `usize::MAX`, so activation fails closed.
fn fair_v2_ingress_required_proposal_bytes(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
    roster_len: usize,
) -> usize {
    let required = || -> Option<usize> {
        let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
        let manifest_bytes = fair_v2_ingress_required_manifest_bytes(layout)?;

        // QuorumCertificate = certified Round + its repeated strict-same-round
        // proposal field + phase + Subject + ExecutionCommitment
        // + Vec<ValidatorIndex> + aggregate signature.
        let signature_vector_bytes = signature_bytes.checked_add(8)?;
        let quorum_certificate_bytes =
            fair_v2_ingress_required_quorum_certificate_bytes(roster_len)?;
        let optional_quorum_certificate_bytes =
            fair_v2_ingress_framed_bytes(quorum_certificate_bytes)?.checked_add(1)?;

        // Every timeout group may contribute its own highest QC and one signer
        // with a maximum-sized signature. Groups are distinct by QC round.
        let timeout_group_bytes = fair_v2_ingress_framed_bytes(optional_quorum_certificate_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(13)?)?
            .checked_add(fair_v2_ingress_framed_bytes(signature_vector_bytes)?)?;
        let framed_timeout_group_bytes = fair_v2_ingress_framed_bytes(timeout_group_bytes)?;
        let timeout_group_vector_bytes = roster_len
            .checked_mul(framed_timeout_group_bytes)?
            .checked_add(8)?;
        let timeout_certificate_bytes =
            fair_v2_ingress_framed_bytes(timeout_group_vector_bytes)?.checked_add(53)?;
        let timeout_justification_bytes = fair_v2_ingress_framed_bytes(timeout_certificate_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(
                optional_quorum_certificate_bytes,
            )?)?;
        let proposal_justification_bytes =
            fair_v2_ingress_framed_bytes(timeout_justification_bytes)?.checked_add(4)?;

        let proposal_bytes = 53_usize
            .checked_add(5)?
            .checked_add(102)?
            .checked_add(fair_v2_ingress_framed_bytes(manifest_bytes)?)?
            .checked_add(fair_v2_ingress_framed_bytes(proposal_justification_bytes)?)?
            .checked_add(fair_v2_ingress_framed_bytes(signature_vector_bytes)?)?;
        let proposal_payload = fair_v2_ingress_framed_bytes(proposal_bytes)?.checked_add(4)?;
        fair_v2_ingress_framed_bytes(proposal_payload)?.checked_add(3)
    };
    required().unwrap_or(usize::MAX)
}

/// Convert a bare v2 envelope length into the bare core network-message length.
///
/// Live Sumeragi transport nests the envelope in the `BlockMessage::V2` enum,
/// serializes that enum as a complete self-describing Norito frame through
/// `BlockMessageWire`, then nests that frame in `NetworkMessage::SumeragiBlock`.
/// This checked calculation mirrors those exact layers without allocating a
/// maximal proposal or body.
fn fair_v2_ingress_network_message_bytes_from_block_message(
    block_message_bytes: usize,
) -> Option<usize> {
    let align = norito::core::archived_payload_align::<BlockMessage>();
    let padding = if align <= 1 {
        0
    } else {
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 {
            0
        } else {
            align.checked_sub(remainder)?
        }
    };
    let block_wire_bytes = norito::core::Header::SIZE
        .checked_add(padding)?
        .checked_add(block_message_bytes)?;
    fair_v2_ingress_framed_bytes(fair_v2_ingress_framed_bytes(block_wire_bytes)?)?
        .checked_add(core::mem::size_of::<u32>())
}

fn fair_v2_ingress_network_message_bytes(consensus_envelope_bytes: usize) -> Option<usize> {
    let block_message_bytes = fair_v2_ingress_framed_bytes(consensus_envelope_bytes)?
        .checked_add(core::mem::size_of::<u32>())?;
    fair_v2_ingress_network_message_bytes_from_block_message(block_message_bytes)
}

/// Exact plaintext P2P data-frame ceiling for one bare v2 envelope.
///
/// The protocol-wide maximum public-key payload is used as both relay origin
/// and direct target, covering validators, observers, and rotated responders
/// independently of the active roster or compiled crypto features. The direct
/// frame dominates broadcast. Arithmetic failures fail closed as `usize::MAX`.
fn fair_v2_ingress_required_p2p_frame_bytes(consensus_envelope_bytes: usize) -> usize {
    let required = || -> Option<usize> {
        let network_message_bytes =
            fair_v2_ingress_network_message_bytes(consensus_envelope_bytes)?;
        Some(
            iroha_p2p::network::data_frame_wire_len_from_payload_len_with_peer_key_bytes::<
                crate::NetworkMessage,
            >(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
                Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
                iroha_p2p::network::MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                network_message_bytes,
            ),
        )
    };
    required().unwrap_or(usize::MAX)
}

/// Exact plaintext P2P frame ceiling for one bounded lane-local block message.
fn fair_v2_ingress_required_lane_p2p_frame_bytes(block_message_bytes: usize) -> usize {
    let required = || -> Option<usize> {
        let network_message_bytes =
            fair_v2_ingress_network_message_bytes_from_block_message(block_message_bytes)?;
        Some(
            iroha_p2p::network::data_frame_wire_len_from_payload_len_with_peer_key_bytes::<
                crate::NetworkMessage,
            >(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
                Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
                iroha_p2p::network::MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                network_message_bytes,
            ),
        )
    };
    required().unwrap_or(usize::MAX)
}

fn fair_v2_ingress_v2_envelope_bytes(payload_bytes: usize) -> Option<usize> {
    let tagged_payload_bytes = fair_v2_ingress_framed_bytes(payload_bytes)?.checked_add(4)?;
    fair_v2_ingress_framed_bytes(tagged_payload_bytes)?.checked_add(3)
}

/// Canonical bytes for a `PeerId` field whose public key has `raw_key_bytes`
/// algorithm-specific bytes, excluding the compact algorithm tag.
fn fair_v2_ingress_embedded_peer_id_bytes(raw_key_bytes: usize) -> Option<usize> {
    let compact_key_bytes = raw_key_bytes.checked_add(1)?;
    let encoded_byte_bytes = fair_v2_ingress_framed_bytes(1)?;
    let public_key_bytes = compact_key_bytes
        .checked_mul(encoded_byte_bytes)?
        .checked_add(8)?;
    let peer_id_bytes = fair_v2_ingress_framed_bytes(public_key_bytes)?;
    fair_v2_ingress_framed_bytes(peer_id_bytes)
}

/// Exact canonical `NetworkMessage` bytes for one maximum sidecar chunk.
///
/// The checked calculation includes both embedded peer identities, the
/// fixed-boundary 64-KiB byte sequence, `CertifiedMergeSidecarMessage::Chunk`,
/// the shared `Arc` carrier, and `NetworkMessage::CertifiedMergeSidecar`.
/// `raw_key_bytes` excludes the compact public-key algorithm tag.
fn fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key(
    raw_key_bytes: usize,
) -> Option<usize> {
    let fixed_u8_field = fair_v2_ingress_framed_bytes(1)?;
    let fixed_u32_field = fair_v2_ingress_framed_bytes(4)?;
    let fixed_u64_field = fair_v2_ingress_framed_bytes(8)?;
    let hash_field = fair_v2_ingress_framed_bytes(CryptoHash::LENGTH)?;

    // Generation, stream epoch, and semantic sequence are transparent
    // newtypes whose sole NonZeroU64 field is itself length-delimited by the
    // derived tuple-struct codec. The enclosing chunk frames each newtype once
    // more as a named field.
    let non_zero_u64_newtype_bytes = fair_v2_ingress_framed_bytes(8)?;
    let non_zero_u64_newtype_field = fair_v2_ingress_framed_bytes(non_zero_u64_newtype_bytes)?;
    let peer_id_field = fair_v2_ingress_embedded_peer_id_bytes(raw_key_bytes)?;
    let byte_sequence_bytes = MAX_CERTIFIED_MERGE_CHUNK_BYTES.checked_add(8)?;
    let byte_sequence_field = fair_v2_ingress_framed_bytes(byte_sequence_bytes)?;

    let chunk_bytes = fixed_u8_field
        .checked_add(non_zero_u64_newtype_field)?
        .checked_add(non_zero_u64_newtype_field)?
        .checked_add(non_zero_u64_newtype_field)?
        .checked_add(hash_field)?
        .checked_add(hash_field)?
        .checked_add(fixed_u64_field)?
        .checked_add(fixed_u64_field)?
        .checked_add(hash_field)?
        .checked_add(peer_id_field)?
        .checked_add(peer_id_field)?
        .checked_add(fixed_u32_field)?
        .checked_add(fixed_u32_field)?
        .checked_add(byte_sequence_field)?;
    let sidecar_message_bytes = fair_v2_ingress_framed_bytes(chunk_bytes)?.checked_add(4)?;

    // Arc<T> writes one ownership prefix around T; the outer enum then frames
    // that Arc field once more after its four-byte discriminant.
    fair_v2_ingress_framed_bytes(fair_v2_ingress_framed_bytes(sidecar_message_bytes)?)?
        .checked_add(4)
}

/// Exact plaintext direct P2P frame required by a maximum sidecar chunk.
///
/// Protocol-maximum public-key payloads cover the embedded requester and
/// responder as well as the relay origin and target. Arithmetic failures map
/// to `usize::MAX`, so context activation fails closed.
fn fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes() -> usize {
    let required = || -> Option<usize> {
        let network_message_bytes =
            fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
            )?;
        Some(
            iroha_p2p::network::data_frame_wire_len_from_payload_len_with_peer_key_bytes::<
                crate::NetworkMessage,
            >(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
                Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
                iroha_p2p::network::MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                network_message_bytes,
            ),
        )
    };
    required().unwrap_or(usize::MAX)
}

/// Exact bare-envelope ceiling for progress-critical recovery requests.
///
/// Certified-body recovery carries a maximal PrepareQC. Durable-certificate
/// recovery carries the frozen chain id. Both authenticate a requester with a
/// protocol-maximum public key and signature, covering non-roster observers as
/// well as validators. The checked calculation mirrors bare Norito v1 exactly.
fn fair_v2_ingress_required_recovery_request_bytes_for_key(
    chain_id: &ChainId,
    roster_len: usize,
    raw_key_bytes: usize,
) -> usize {
    let required = || -> Option<usize> {
        let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
        let quorum_certificate_bytes =
            fair_v2_ingress_required_quorum_certificate_bytes(roster_len)?;
        let requester_bytes = fair_v2_ingress_embedded_peer_id_bytes(raw_key_bytes)?;
        let signature_vector_bytes = fair_v2_ingress_framed_bytes(signature_bytes.checked_add(8)?)?;

        let certified_body_request_bytes = 53_usize
            .checked_add(102)?
            .checked_add(fair_v2_ingress_framed_bytes(quorum_certificate_bytes)?)?
            .checked_add(requester_bytes)?
            .checked_add(signature_vector_bytes)?;
        let certified_body_request =
            fair_v2_ingress_v2_envelope_bytes(certified_body_request_bytes)?;

        let chain_string_bytes = fair_v2_ingress_framed_bytes(chain_id.as_str().len())?;
        let boxed_chain_string_bytes = fair_v2_ingress_framed_bytes(chain_string_bytes)?;
        let embedded_chain_id_bytes = fair_v2_ingress_framed_bytes(boxed_chain_string_bytes)?;
        let commit_certificate_request_bytes = 3_usize
            .checked_add(embedded_chain_id_bytes)?
            .checked_add(34)?
            .checked_add(9)?
            .checked_add(requester_bytes)?
            .checked_add(signature_vector_bytes)?;
        let commit_certificate_request =
            fair_v2_ingress_v2_envelope_bytes(commit_certificate_request_bytes)?;
        Some(certified_body_request.max(commit_certificate_request))
    };
    required().unwrap_or(usize::MAX)
}

fn fair_v2_ingress_required_recovery_request_bytes(chain_id: &ChainId, roster_len: usize) -> usize {
    fair_v2_ingress_required_recovery_request_bytes_for_key(
        chain_id,
        roster_len,
        iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
    )
}

/// Exact bare-envelope ceiling for durable CommitQC recovery responses.
fn fair_v2_ingress_required_commit_certificate_response_bytes_for_key(
    roster_len: usize,
    raw_key_bytes: usize,
) -> usize {
    let required = || -> Option<usize> {
        let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
        let quorum_certificate_bytes =
            fair_v2_ingress_required_quorum_certificate_bytes(roster_len)?;
        let responder_bytes = fair_v2_ingress_embedded_peer_id_bytes(raw_key_bytes)?;
        let response_bytes = 33_usize
            .checked_add(fair_v2_ingress_framed_bytes(quorum_certificate_bytes)?)?
            .checked_add(responder_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(
                signature_bytes.checked_add(8)?,
            )?)?;
        fair_v2_ingress_v2_envelope_bytes(response_bytes)
    };
    required().unwrap_or(usize::MAX)
}

fn fair_v2_ingress_required_commit_certificate_response_bytes(roster_len: usize) -> usize {
    fair_v2_ingress_required_commit_certificate_response_bytes_for_key(
        roster_len,
        iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
    )
}

/// Exact canonical-wire ceiling for either payload transport completion.
///
/// The checked calculation mirrors bare Norito's fixed v1 layout with default
/// compact lengths. `F(x)` is one compact length prefix plus `x` payload
/// bytes. A `Vec<Hash>` is its eight-byte sequence count plus 33 bytes per
/// element: one compact element-length byte and the 32-byte hash. The numeric
/// constants are the exact maxima for the remaining bounded structural fields
/// (including a 256-byte consensus signature) at each nesting layer. Overflow
/// maps to `usize::MAX`, making height activation fail closed before ingress
/// opens.
fn fair_v2_ingress_required_transport_completion_bytes(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
) -> usize {
    let required = || -> Option<usize> {
        let payload_bytes = usize::try_from(layout.max_payload_size_bytes).ok()?;
        let chunk_bytes = usize::try_from(layout.chunk_size_bytes).ok()?;
        let manifest_bytes = fair_v2_ingress_required_manifest_bytes(layout)?;
        let encoded_body_bytes = payload_bytes.checked_add(8)?;
        let response_bytes = fair_v2_ingress_framed_bytes(manifest_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(encoded_body_bytes)?)?
            .checked_add(304)?;
        let encoded_chunk_bytes = chunk_bytes.checked_add(8)?;
        let chunk_bytes = fair_v2_ingress_framed_bytes(encoded_chunk_bytes)?.checked_add(309)?;

        let response_payload = fair_v2_ingress_framed_bytes(response_bytes)?.checked_add(4)?;
        let response_envelope = fair_v2_ingress_framed_bytes(response_payload)?.checked_add(3)?;
        let chunk_payload = fair_v2_ingress_framed_bytes(chunk_bytes)?.checked_add(4)?;
        let chunk_envelope = fair_v2_ingress_framed_bytes(chunk_payload)?.checked_add(3)?;
        Some(response_envelope.max(chunk_envelope))
    };
    required().unwrap_or(usize::MAX)
}

/// Exact BlockSync-topic P2P frame requirement for one frozen context.
///
/// Context-owned DA completion, lane-local completion, and the layout-neutral
/// certified merge-sidecar chunk all share this transport partition.
fn fair_v2_ingress_required_block_sync_p2p_frame_bytes(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
) -> usize {
    fair_v2_ingress_required_p2p_frame_bytes(fair_v2_ingress_required_transport_completion_bytes(
        layout,
    ))
    .max(fair_v2_ingress_required_lane_p2p_frame_bytes(
        MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES,
    ))
    .max(fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressRejectReason {
    UnsupportedMessageKind,
    WrongProtocolVersion,
    MessageTooLarge,
    UnsupportedEnvelope,
    PendingWireOwnershipMismatch,
    RouteOwnershipInvalid,
    AttemptCursorInvalid,
    OwnershipEvidenceInvalid,
    SourceLaneInvalid,
    UnauthorizedTransportCompletion,
    ProductiveOriginMissing,
    ProductiveOriginOutsideRoster,
    WrongHeightContext,
    LeaderWireObsoleteOrConflicting,
}

#[derive(Debug)]
struct FairV2IngressRejection {
    inbound: InboundBlockMessage,
    reason: FairV2IngressRejectReason,
}

#[derive(Debug)]
enum FairV2IngressPushError {
    Closed(InboundBlockMessage),
    FailStop(InboundBlockMessage),
    Full(InboundBlockMessage),
    Rejected(FairV2IngressRejection),
}

impl FairV2IngressPushError {
    fn rejected(inbound: InboundBlockMessage, reason: FairV2IngressRejectReason) -> Self {
        Self::Rejected(FairV2IngressRejection { inbound, reason })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressPushDisposition {
    Enqueued,
    Coalesced,
}

/// Why a checked fair-ingress dequeue crossed its downstream admission gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FairV2IngressDequeueDisposition {
    /// The downstream consumer admitted the exact occurrence normally.
    Admit,
    /// The monotone safety-WAL recovery cut permanently obsoleted the exact
    /// productive wire, so capacity cannot make it relevant again.
    RetireObsolete,
}

/// Closed internal policy for crossing a durable physical ingress barrier.
///
/// The ordinary selector preserves every barrier. The timeout-vote episode
/// variant exposes only a directly authenticated validator's exact productive
/// TimeoutVote to the downstream episode predicate while a selected Serve
/// occurrence or one bounded certified-response carrier owns the shared
/// physical turn. Response authority is acquired only after dequeue, so the
/// phase check deliberately does not assume a claim which cannot exist yet.
/// It neither borrows certified capacity nor admits the vote by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FairV2IngressBarrierBypass {
    /// Preserve every durable ingress barrier.
    None,
    /// Let the finite current-view TimeoutVote episode reach its predicate.
    TimeoutVoteEpisode,
}

/// Fixed-capacity, roster-aware v2 ingress with per-hop admission and service fairness.
///
/// Every authenticated validator hop owns one protected source slot, one ordinary
/// progress slot, one certified-fence-escape slot, one distinct TimeoutVote slot,
/// and one transport-completion slot. Every authenticated non-validator hop
/// independently owns three slots: general work, certified fence escape, and
/// transport completion. Only messages without an authenticated transport hop share the two-position
/// anonymous lane. A current-roster completion forwarded by a non-validator
/// source, or a proof-carrying historical-recovery response from a predecessor
/// signer, stays in that exact authenticated source's lane and cannot spend
/// another source's reservation. The latter is authorized downstream against
/// its outstanding request and frozen historical certificate.
/// Exact wire retransmissions coalesce
/// only while the same semantic origin still owns an identical queued envelope;
/// after service, a later retransmission is admitted normally. Distinct
/// semantic origins relayed by one hop share that hop's finite owners. A
/// distinct response through the same validator retries after fair service
/// releases that validator hop's sole completion owner.
/// Every newly queued occurrence receives one immutable local admission
/// ordinal; coalesced retransmissions retain their existing owner's ordinal.
/// While a certified-body request is queued, its earliest ordinal is a global
/// cutoff: preexisting occurrences and that request remain fairly selectable,
/// but later traffic cannot acquire service ahead of it.
///
/// Non-empty lanes are serviced in round-robin order, so a source may use
/// otherwise idle capacity but cannot starve an honest validator's progress.
/// Canonical envelope hashes are computed before taking the shared queue lock,
/// so duplicate detection never compares whole bodies while holding that lock.
/// Canonical wire bytes are charged to fixed aggregate and per-source budgets.
/// Within each validator partition, ordinary traffic, certified fence escape,
/// TimeoutVote, and payload transport completion own disjoint byte regions.
/// Authenticated non-validator partitions isolate certified escape as well;
/// anonymous partitions separate only ordinary and completion bytes.
/// Lane-local control uses ordinary progress while atomic certificate recovery
/// owns its isolated certified reservation; exact
/// executable-payload and proof-carrying historical-recovery response bytes
/// share the completion reservation. Roster
/// installation succeeds only when the configured authenticated-source
/// geometry plus the anonymous lane own isolated byte partitions.
/// `CommitCertificateResponse` remains reducer-producing Progress and uses the
/// certified reservation only when it embeds a CommitQC.
pub(crate) struct FairV2Ingress {
    capacity: usize,
    byte_capacity: usize,
    source_byte_capacity: usize,
    certified_fence_escape_byte_reserve: usize,
    timeout_vote_byte_reserve: usize,
    transport_completion_byte_reserve: usize,
    consensus_frame_byte_capacity: usize,
    control_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    outbound_high_frame_byte_capacity: usize,
    /// Maximum simultaneously materialized authenticated non-validator lanes.
    ///
    /// Test-only constructors leave this absent and exercise occupancy-bound
    /// arithmetic directly with deliberately small queue geometries.
    authenticated_non_validator_source_capacity: Option<usize>,
    /// Serializes consumers while allowing expensive admission checks to run
    /// without blocking producers on the ingress-state mutex.
    service_lock: Mutex<()>,
    state: Mutex<FairV2IngressState>,
}

impl FairV2Ingress {
    #[cfg(test)]
    fn new(
        capacity: usize,
        byte_capacity: usize,
        source_byte_capacity: usize,
        timeout_vote_byte_reserve: usize,
        transport_completion_byte_reserve: usize,
    ) -> Self {
        Self::new_with_transport_frame_caps(
            capacity,
            byte_capacity,
            source_byte_capacity,
            timeout_vote_byte_reserve,
            transport_completion_byte_reserve,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )
    }

    #[cfg(test)]
    fn new_with_transport_frame_caps(
        capacity: usize,
        byte_capacity: usize,
        source_byte_capacity: usize,
        timeout_vote_byte_reserve: usize,
        transport_completion_byte_reserve: usize,
        consensus_frame_byte_capacity: usize,
        control_frame_byte_capacity: usize,
        block_sync_frame_byte_capacity: usize,
        outbound_high_frame_byte_capacity: usize,
    ) -> Self {
        Self::new_with_source_geometry_and_transport_frame_caps(
            capacity,
            byte_capacity,
            source_byte_capacity,
            0,
            timeout_vote_byte_reserve,
            transport_completion_byte_reserve,
            consensus_frame_byte_capacity,
            control_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            outbound_high_frame_byte_capacity,
            None,
        )
    }

    fn new_with_source_geometry_and_transport_frame_caps(
        capacity: usize,
        byte_capacity: usize,
        source_byte_capacity: usize,
        certified_fence_escape_byte_reserve: usize,
        timeout_vote_byte_reserve: usize,
        transport_completion_byte_reserve: usize,
        consensus_frame_byte_capacity: usize,
        control_frame_byte_capacity: usize,
        block_sync_frame_byte_capacity: usize,
        outbound_high_frame_byte_capacity: usize,
        authenticated_non_validator_source_capacity: Option<usize>,
    ) -> Self {
        let mut lanes = BTreeMap::new();
        lanes.insert(FairV2IngressSource::Anonymous, FairV2IngressLane::default());
        Self {
            capacity,
            byte_capacity,
            source_byte_capacity,
            certified_fence_escape_byte_reserve,
            timeout_vote_byte_reserve,
            transport_completion_byte_reserve,
            consensus_frame_byte_capacity,
            control_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            outbound_high_frame_byte_capacity,
            authenticated_non_validator_source_capacity,
            service_lock: Mutex::new(()),
            state: Mutex::new(FairV2IngressState {
                roster: BTreeSet::new(),
                lanes,
                pending_wire_owners: BTreeMap::new(),
                leader_wire_lifecycles: BTreeMap::new(),
                leader_wire_max_chunk_count: u32::try_from(capacity).unwrap_or(u32::MAX),
                last_admission_ordinal: 0,
                ready: VecDeque::new(),
                len: 0,
                bytes: 0,
                nonempty_since: None,
                last_service_attempt_at: None,
                required_ordinary_bytes: 0,
                required_certified_fence_escape_bytes: 0,
                required_transport_completion_bytes: 0,
                required_consensus_frame_bytes: 0,
                required_control_frame_bytes: 0,
                required_block_sync_frame_bytes: 0,
                required_outbound_high_frame_bytes: 0,
                requires_certified_serve_gate: false,
                certified_serve_gate: None,
                requires_leader_wire_lifecycle_gate: false,
                leader_wire_lifecycle_gate: None,
                leader_wire_lifecycle_ordinals: None,
                leader_wire_context: None,
                open: false,
            }),
        }
    }

    fn debug_assert_consistent(&self, state: &FairV2IngressState) {
        #[cfg(not(debug_assertions))]
        let _ = state;
        #[cfg(debug_assertions)]
        {
            let actual_len = state
                .lanes
                .values()
                .map(|lane| lane.entries.len())
                .sum::<usize>();
            let actual_bytes = state.lanes.values().map(|lane| lane.bytes).sum::<usize>();
            debug_assert_eq!(state.len, actual_len);
            debug_assert_eq!(state.bytes, actual_bytes);
            debug_assert!(state.bytes <= self.byte_capacity);
            debug_assert_eq!(state.nonempty_since.is_some(), state.len != 0);
            if state.len == 0 {
                debug_assert!(state.last_service_attempt_at.is_none());
            }

            let ready = state.ready.iter().cloned().collect::<BTreeSet<_>>();
            debug_assert_eq!(ready.len(), state.ready.len());
            let nonempty = state
                .lanes
                .iter()
                .filter(|(_, lane)| !lane.entries.is_empty())
                .map(|(source, _)| source.clone())
                .collect::<BTreeSet<_>>();
            debug_assert_eq!(ready, nonempty);

            let indexed_owners = state
                .lanes
                .iter()
                .flat_map(|(source, lane)| {
                    lane.pending_wire
                        .iter()
                        .cloned()
                        .map(|key| (key, source.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            debug_assert_eq!(state.pending_wire_owners, indexed_owners);
            debug_assert!(
                fair_v2_ingress_leader_wire_lifecycle_capacity(
                    state.roster.len(),
                    state.leader_wire_max_chunk_count,
                )
                .is_some_and(|capacity| state.leader_wire_lifecycles.len() <= capacity)
            );
            debug_assert!(state.leader_wire_lifecycles.iter().all(|(slot, record)| {
                &record.token.slot == slot
                    && record.token.identity.semantic_origin == slot.semantic_origin
                    && record.token.identity.phase == slot.phase
                    && record.token.source_class == slot.phase.source_class()
                    && state.roster.contains(&slot.semantic_origin)
                    && slot.chunk_index.is_none_or(|index| {
                        record.token.source_class == FairV2IngressLeaderWireSourceClass::Chunk
                            && index < state.leader_wire_max_chunk_count
                    })
                    && record.token.admission_ordinal <= state.last_admission_ordinal
                    && record.token.scheduler_ordinal != 0
                    && record.ingress_predecessors.iter().all(|(source, count)| {
                        *count <= state.lanes.get(source).map_or(0, |lane| lane.entries.len())
                    })
            }));
            let lifecycle_scheduler_ordinals = state
                .leader_wire_lifecycles
                .values()
                .map(|record| record.token.scheduler_ordinal)
                .collect::<BTreeSet<_>>();
            debug_assert_eq!(
                lifecycle_scheduler_ordinals.len(),
                state.leader_wire_lifecycles.len()
            );
            debug_assert!(state.leader_wire_lifecycles.values().all(|record| {
                let carrier_count = state
                    .lanes
                    .values()
                    .flat_map(|lane| lane.entries.iter())
                    .filter(|entry| entry.leader_wire_token.as_ref() == Some(&record.token))
                    .count();
                carrier_count
                    == usize::from(record.status == FairV2IngressLeaderWireStatus::Ingress)
            }));
            let admission_ordinals = state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter().map(|entry| entry.admission_ordinal))
                .collect::<BTreeSet<_>>();
            debug_assert_eq!(admission_ordinals.len(), state.len);
            debug_assert!(
                admission_ordinals
                    .last()
                    .is_none_or(|ordinal| *ordinal <= state.last_admission_ordinal)
            );
            let certified_entries = state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .filter(|entry| fair_v2_ingress_is_certified_body_request(&entry.inbound))
                .collect::<Vec<_>>();
            if let Some(gate) = state.certified_serve_gate.as_ref() {
                debug_assert!(certified_entries.iter().all(|entry| {
                    let BlockMessage::V2(ConsensusMessageV2 {
                        payload: ConsensusMessageV2Payload::CertifiedBodyRequest(request),
                        ..
                    }) = entry.inbound.message()
                    else {
                        unreachable!("certified ingress filter returns only requests");
                    };
                    gate.requires_reservation(request)
                        == entry.certified_serve_reservation.is_some()
                        && entry
                            .certified_serve_reservation
                            .as_ref()
                            .is_none_or(|reservation| {
                                entry.inbound.ingress_ownership.as_ref().and_then(
                                    FairV2IngressOwnershipEvidence::runtime_lifecycle_ordinal,
                                ) == Some(reservation.scheduler_ordinal())
                            })
                }));
            }

            for (source, lane) in &state.lanes {
                debug_assert!(lane.bytes <= self.source_byte_capacity);
                debug_assert!(lane.entries.iter().all(|entry| {
                    entry.encoded_len == entry.encoded_bytes.len()
                        && entry
                            .inbound
                            .ingress_ownership
                            .as_ref()
                            .is_some_and(FairV2IngressOwnershipEvidence::validate_exact)
                }));
                debug_assert!(lane.entries.iter().all(|entry| {
                    entry.leader_wire_token.as_ref().is_none_or(|token| {
                        state
                            .leader_wire_lifecycles
                            .get(&token.slot)
                            .is_some_and(|record| {
                                record.token == *token
                                    && record.status == FairV2IngressLeaderWireStatus::Ingress
                            })
                    })
                }));
                debug_assert_eq!(
                    lane.progress_len,
                    lane.entries
                        .iter()
                        .filter(|entry| entry.class == FairV2IngressClass::Progress)
                        .count()
                );
                debug_assert_eq!(
                    lane.timeout_vote_len,
                    lane.entries
                        .iter()
                        .filter(|entry| fair_v2_ingress_is_timeout_vote(&entry.inbound))
                        .count()
                );
                let source_has_certified_reserve =
                    !matches!(source, FairV2IngressSource::Anonymous);
                debug_assert_eq!(
                    lane.certified_fence_escape_len,
                    if source_has_certified_reserve {
                        lane.entries
                            .iter()
                            .filter(|entry| {
                                fair_v2_ingress_is_certified_fence_escape(&entry.inbound)
                            })
                            .count()
                    } else {
                        0
                    }
                );
                debug_assert_eq!(
                    lane.transport_completion_len,
                    lane.entries
                        .iter()
                        .filter(|entry| entry.class == FairV2IngressClass::TransportCompletion)
                        .count()
                );
                let indexed = lane
                    .entries
                    .iter()
                    .filter_map(|entry| entry.wire_key.clone())
                    .collect::<BTreeSet<_>>();
                debug_assert_eq!(lane.pending_wire, indexed);
                debug_assert_eq!(
                    lane.bytes,
                    lane.entries
                        .iter()
                        .map(|entry| entry.encoded_len)
                        .sum::<usize>()
                );
                let actual_transport_completion_bytes = lane
                    .entries
                    .iter()
                    .filter(|entry| entry.class == FairV2IngressClass::TransportCompletion)
                    .map(|entry| {
                        debug_assert!(entry.encoded_len <= self.transport_completion_byte_reserve);
                        entry.encoded_len
                    })
                    .sum::<usize>();
                debug_assert!(lane.transport_completion_len <= 1);
                debug_assert_eq!(
                    lane.transport_completion_bytes,
                    actual_transport_completion_bytes
                );
                debug_assert!(
                    lane.transport_completion_bytes <= self.transport_completion_byte_reserve
                );
                let actual_certified_fence_escape_bytes = if source_has_certified_reserve {
                    lane.entries
                        .iter()
                        .filter(|entry| fair_v2_ingress_is_certified_fence_escape(&entry.inbound))
                        .map(|entry| {
                            debug_assert!(
                                entry.encoded_len <= self.certified_fence_escape_byte_reserve
                            );
                            entry.encoded_len
                        })
                        .sum::<usize>()
                } else {
                    0
                };
                debug_assert!(lane.certified_fence_escape_len <= 1);
                debug_assert_eq!(
                    lane.certified_fence_escape_bytes,
                    actual_certified_fence_escape_bytes
                );
                debug_assert!(
                    lane.certified_fence_escape_bytes <= self.certified_fence_escape_byte_reserve
                );
                if matches!(source, FairV2IngressSource::Validator(_)) {
                    let actual_timeout_vote_bytes = lane
                        .entries
                        .iter()
                        .filter(|entry| fair_v2_ingress_is_timeout_vote(&entry.inbound))
                        .map(|entry| {
                            debug_assert!(entry.encoded_len <= self.timeout_vote_byte_reserve);
                            entry.encoded_len
                        })
                        .sum::<usize>();
                    debug_assert!(lane.timeout_vote_len <= 1);
                    debug_assert_eq!(lane.timeout_vote_bytes, actual_timeout_vote_bytes);
                    debug_assert!(lane.timeout_vote_bytes <= self.timeout_vote_byte_reserve);
                    let reserved_bytes = lane
                        .certified_fence_escape_bytes
                        .checked_add(lane.timeout_vote_bytes)
                        .and_then(|reserved| reserved.checked_add(lane.transport_completion_bytes))
                        .expect("per-source byte ownership remains bounded");
                    debug_assert!(lane.bytes.checked_sub(reserved_bytes).is_some_and(
                        |ordinary_bytes| {
                            ordinary_bytes
                                <= self
                                    .source_byte_capacity
                                    .saturating_sub(self.certified_fence_escape_byte_reserve)
                                    .saturating_sub(self.timeout_vote_byte_reserve)
                                    .saturating_sub(self.transport_completion_byte_reserve)
                        }
                    ));
                } else if matches!(source, FairV2IngressSource::Authenticated(_)) {
                    debug_assert_eq!(lane.timeout_vote_bytes, 0);
                    let reserved_bytes = lane
                        .certified_fence_escape_bytes
                        .checked_add(lane.transport_completion_bytes)
                        .expect("per-source byte ownership remains bounded");
                    debug_assert!(lane.bytes.checked_sub(reserved_bytes).is_some_and(
                        |ordinary_bytes| {
                            ordinary_bytes
                                <= self
                                    .source_byte_capacity
                                    .saturating_sub(self.certified_fence_escape_byte_reserve)
                                    .saturating_sub(self.transport_completion_byte_reserve)
                        }
                    ));
                } else {
                    debug_assert_eq!(lane.certified_fence_escape_bytes, 0);
                    debug_assert_eq!(lane.timeout_vote_bytes, 0);
                    debug_assert!(
                        lane.bytes
                            .checked_sub(lane.transport_completion_bytes)
                            .is_some_and(|ordinary_bytes| {
                                ordinary_bytes
                                    <= self
                                        .source_byte_capacity
                                        .saturating_sub(self.transport_completion_byte_reserve)
                            })
                    );
                }
            }

            if state.open {
                debug_assert!(
                    state
                        .required_ordinary_bytes
                        .checked_add(self.certified_fence_escape_byte_reserve)
                        .and_then(|reserved| reserved.checked_add(self.timeout_vote_byte_reserve))
                        .and_then(|reserved| {
                            reserved.checked_add(self.transport_completion_byte_reserve)
                        })
                        .is_some_and(|reserved| reserved <= self.source_byte_capacity)
                );
                debug_assert!(
                    state.required_certified_fence_escape_bytes
                        <= self.certified_fence_escape_byte_reserve
                );
                debug_assert!(
                    state.required_transport_completion_bytes
                        <= self.transport_completion_byte_reserve
                );
                debug_assert!(
                    state.required_consensus_frame_bytes <= self.consensus_frame_byte_capacity
                );
                debug_assert!(
                    state.required_control_frame_bytes <= self.control_frame_byte_capacity
                );
                debug_assert!(
                    state.required_block_sync_frame_bytes <= self.block_sync_frame_byte_capacity
                );
                debug_assert!(
                    state.required_outbound_high_frame_bytes
                        <= self.outbound_high_frame_byte_capacity
                );
                let protected = fair_v2_ingress_current_protected_slots(
                    state,
                    self.authenticated_non_validator_source_capacity,
                );
                debug_assert!(
                    state
                        .len
                        .checked_add(protected)
                        .is_some_and(|owned| owned <= self.capacity)
                );
            }
        }
    }

    fn ownership_resource_snapshot(
        &self,
        state: &FairV2IngressState,
        source: &FairV2IngressSource,
    ) -> FairV2IngressResourceSnapshot {
        let lane = state.lanes.get(source);
        FairV2IngressResourceSnapshot {
            source_len: lane.map_or(0, |lane| lane.entries.len()),
            source_progress_len: lane.map_or(0, |lane| lane.progress_len),
            source_certified_fence_escape_len: lane
                .map_or(0, |lane| lane.certified_fence_escape_len),
            source_timeout_vote_len: lane.map_or(0, |lane| lane.timeout_vote_len),
            source_transport_completion_len: lane.map_or(0, |lane| lane.transport_completion_len),
            source_bytes: lane.map_or(0, |lane| lane.bytes),
            source_certified_fence_escape_bytes: lane
                .map_or(0, |lane| lane.certified_fence_escape_bytes),
            source_timeout_vote_bytes: lane.map_or(0, |lane| lane.timeout_vote_bytes),
            source_transport_completion_bytes: lane
                .map_or(0, |lane| lane.transport_completion_bytes),
            global_len: state.len,
            global_bytes: state.bytes,
            protected_slots: fair_v2_ingress_current_protected_slots(
                state,
                self.authenticated_non_validator_source_capacity,
            ),
            message_capacity: self.capacity,
            global_byte_capacity: self.byte_capacity,
            source_byte_capacity: self.source_byte_capacity,
            certified_fence_escape_byte_reserve: self.certified_fence_escape_byte_reserve,
            timeout_vote_byte_reserve: self.timeout_vote_byte_reserve,
            transport_completion_byte_reserve: self.transport_completion_byte_reserve,
        }
    }

    /// Close admission and atomically install the next height's frozen roster.
    ///
    /// Queued messages belong to the preceding immutable height and are
    /// discarded while the public ingress gate is closed. The caller may open
    /// the queue only after context and safety-WAL recovery complete.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn configure_roster(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
    ) -> Result<(), FairV2IngressCapacityError> {
        self.configure_roster_with_byte_requirements(roster, 0, 0, 0, 0, 0, 0, 0)
    }

    /// Install a frozen roster and validate every progress envelope against
    /// its ingress, topic-frame, and outbound encrypted-frame byte owner.
    pub(crate) fn configure_roster_for_context(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
        chain_id: &ChainId,
        layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
    ) -> Result<(), FairV2IngressCapacityError> {
        let roster = roster.into_iter().collect::<BTreeSet<_>>();
        let required_proposal_bytes = fair_v2_ingress_required_proposal_bytes(layout, roster.len());
        let required_commit_certificate_response_bytes =
            fair_v2_ingress_required_commit_certificate_response_bytes(roster.len());
        let required_certified_fence_escape_bytes =
            fair_v2_ingress_required_certified_fence_escape_bytes(roster.len());
        let required_control_message_bytes =
            required_proposal_bytes.max(required_commit_certificate_response_bytes);
        let required_transport_completion_bytes =
            fair_v2_ingress_required_transport_completion_bytes(layout)
                .max(MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES);
        let required_recovery_request_bytes =
            fair_v2_ingress_required_recovery_request_bytes(chain_id, roster.len());
        let required_lane_progress_frame_bytes =
            fair_v2_ingress_required_lane_p2p_frame_bytes(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);
        let required_consensus_frame_bytes =
            fair_v2_ingress_required_p2p_frame_bytes(required_recovery_request_bytes)
                .max(required_lane_progress_frame_bytes)
                .max(crate::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES);
        let required_control_frame_bytes =
            fair_v2_ingress_required_p2p_frame_bytes(required_control_message_bytes);
        let required_block_sync_frame_bytes =
            fair_v2_ingress_required_block_sync_p2p_frame_bytes(layout);
        let required_outbound_plaintext_frame_bytes = required_consensus_frame_bytes
            .max(required_control_frame_bytes)
            .max(required_block_sync_frame_bytes);
        let Some(required_outbound_high_frame_bytes) =
            iroha_p2p::frame_queue_charge(required_outbound_plaintext_frame_bytes)
        else {
            return Err(FairV2IngressCapacityError {
                configured: self.outbound_high_frame_byte_capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::OutboundHighFrameBytes,
            });
        };
        let configured = self.configure_roster_with_byte_requirements(
            roster,
            BODY_ENVELOPE_HEADROOM_BYTES
                .max(required_control_message_bytes)
                .max(required_recovery_request_bytes)
                .max(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES)
                .max(crate::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES),
            required_certified_fence_escape_bytes,
            required_transport_completion_bytes,
            required_consensus_frame_bytes,
            required_control_frame_bytes,
            required_block_sync_frame_bytes,
            required_outbound_high_frame_bytes,
        );
        if configured.is_ok() {
            self.state.lock().leader_wire_max_chunk_count = layout.max_chunk_count;
        }
        configured
    }

    fn configure_roster_with_byte_requirements(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
        required_ordinary_bytes: usize,
        required_certified_fence_escape_bytes: usize,
        required_transport_completion_bytes: usize,
        required_consensus_frame_bytes: usize,
        required_control_frame_bytes: usize,
        required_block_sync_frame_bytes: usize,
        required_outbound_high_frame_bytes: usize,
    ) -> Result<(), FairV2IngressCapacityError> {
        let roster = roster.into_iter().collect::<BTreeSet<_>>();
        let required = fair_v2_ingress_required_capacity(
            roster.len(),
            self.authenticated_non_validator_source_capacity,
        );
        let mut lanes = BTreeMap::new();
        for peer in &roster {
            lanes.insert(
                FairV2IngressSource::Validator(peer.clone()),
                FairV2IngressLane::default(),
            );
        }
        lanes.insert(FairV2IngressSource::Anonymous, FairV2IngressLane::default());
        let _service_guard = self.service_lock.lock();
        let mut state = self.state.lock();
        state.open = false;
        state.roster = roster;
        state.lanes = lanes;
        state.pending_wire_owners.clear();
        state.leader_wire_lifecycles.clear();
        // Keep `last_admission_ordinal`: queued ownership is reset at rollover,
        // but occurrence order remains monotone for the lifetime of this ingress.
        state.ready.clear();
        state.len = 0;
        state.bytes = 0;
        state.nonempty_since = None;
        state.last_service_attempt_at = None;
        state.required_ordinary_bytes = required_ordinary_bytes;
        state.required_certified_fence_escape_bytes = required_certified_fence_escape_bytes;
        state.required_transport_completion_bytes = required_transport_completion_bytes;
        state.required_consensus_frame_bytes = required_consensus_frame_bytes;
        state.required_control_frame_bytes = required_control_frame_bytes;
        state.required_block_sync_frame_bytes = required_block_sync_frame_bytes;
        state.required_outbound_high_frame_bytes = required_outbound_high_frame_bytes;
        let Some(required) = required else {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::Messages,
            });
        };
        if required > self.capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required,
                kind: FairV2IngressCapacityKind::Messages,
            });
        }
        if self.certified_fence_escape_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.certified_fence_escape_byte_reserve,
                kind: FairV2IngressCapacityKind::CertifiedFenceEscapeBytes,
            });
        }
        if self.timeout_vote_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.timeout_vote_byte_reserve,
                kind: FairV2IngressCapacityKind::TimeoutVoteBytes,
            });
        }
        let Some(required_reserved_bytes) = self
            .certified_fence_escape_byte_reserve
            .checked_add(self.timeout_vote_byte_reserve)
            .and_then(|reserved| reserved.checked_add(self.transport_completion_byte_reserve))
        else {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        };
        if required_reserved_bytes > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: required_reserved_bytes,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        }
        let ordinary_bytes = self.source_byte_capacity - required_reserved_bytes;
        if ordinary_bytes < state.required_ordinary_bytes {
            return Err(FairV2IngressCapacityError {
                configured: ordinary_bytes,
                required: state.required_ordinary_bytes,
                kind: FairV2IngressCapacityKind::OrdinaryBytes,
            });
        }
        if state.required_certified_fence_escape_bytes > self.certified_fence_escape_byte_reserve {
            return Err(FairV2IngressCapacityError {
                configured: self.certified_fence_escape_byte_reserve,
                required: state.required_certified_fence_escape_bytes,
                kind: FairV2IngressCapacityKind::CertifiedFenceEscapeBytes,
            });
        }
        if state.required_transport_completion_bytes > self.transport_completion_byte_reserve {
            return Err(FairV2IngressCapacityError {
                configured: self.transport_completion_byte_reserve,
                required: state.required_transport_completion_bytes,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        }
        if state.required_consensus_frame_bytes > self.consensus_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.consensus_frame_byte_capacity,
                required: state.required_consensus_frame_bytes,
                kind: FairV2IngressCapacityKind::ConsensusFrameBytes,
            });
        }
        if state.required_control_frame_bytes > self.control_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.control_frame_byte_capacity,
                required: state.required_control_frame_bytes,
                kind: FairV2IngressCapacityKind::ControlFrameBytes,
            });
        }
        if state.required_block_sync_frame_bytes > self.block_sync_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.block_sync_frame_byte_capacity,
                required: state.required_block_sync_frame_bytes,
                kind: FairV2IngressCapacityKind::BlockSyncFrameBytes,
            });
        }
        if state.required_outbound_high_frame_bytes > self.outbound_high_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.outbound_high_frame_byte_capacity,
                required: state.required_outbound_high_frame_bytes,
                kind: FairV2IngressCapacityKind::OutboundHighFrameBytes,
            });
        }
        let Some(required_bytes) = fair_v2_ingress_required_byte_capacity(
            state.roster.len(),
            self.authenticated_non_validator_source_capacity,
            self.source_byte_capacity,
        ) else {
            return Err(FairV2IngressCapacityError {
                configured: self.byte_capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::Bytes,
            });
        };
        if required_bytes > self.byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.byte_capacity,
                required: required_bytes,
                kind: FairV2IngressCapacityKind::Bytes,
            });
        }
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Require one per-height Serve gate before production admission opens.
    fn require_certified_serve_gate(&self) {
        let mut state = self.state.lock();
        assert!(
            !state.open,
            "Serve-gate policy changes only while ingress is closed"
        );
        assert_eq!(
            state.len, 0,
            "Serve-gate policy precedes all ingress ownership"
        );
        state.requires_certified_serve_gate = true;
    }

    /// Require a context-bound durable leader-wire lifecycle before opening.
    fn require_leader_wire_lifecycle_gate(&self) {
        let mut state = self.state.lock();
        assert!(
            !state.open,
            "leader-wire lifecycle policy changes only while ingress is closed"
        );
        assert_eq!(
            state.len, 0,
            "leader-wire lifecycle policy precedes all ingress ownership"
        );
        state.requires_leader_wire_lifecycle_gate = true;
    }

    /// Bind the current height's internal Serve owner before opening ingress.
    pub(crate) fn bind_certified_serve_gate(
        &self,
        gate: v2_worker::CertifiedServeIngressGate,
    ) -> Result<(), String> {
        let mut state = self.state.lock();
        if state.open || state.len != 0 {
            return Err("certified Serve gate can bind only to an empty closed ingress".to_owned());
        }
        if state.certified_serve_gate.is_some() {
            return Err("certified Serve gate is already bound".to_owned());
        }
        if state
            .leader_wire_lifecycle_ordinals
            .as_ref()
            .is_some_and(|source| !gate.shares_lifecycle_ordinals(source))
        {
            return Err(
                "certified Serve gate changed the actor-global lifecycle ordinal source".to_owned(),
            );
        }
        state.certified_serve_gate = Some(gate);
        Ok(())
    }

    /// Bind and restore the current height's durable productive-wire owner.
    pub(crate) fn bind_leader_wire_lifecycle_gate(
        &self,
        gate: Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>,
        restore: serviced_candidate_store::LeaderWireLifecycleRestore,
        lifecycle_ordinals: v2_runtime::RuntimeLifecycleOrdinalSource,
        context_id: iroha_data_model::block::consensus_v2::HeightContextId,
        height: iroha_data_model::block::consensus_v2::Height,
    ) -> Result<(), String> {
        let mut state = self.state.lock();
        if state.open || state.len != 0 {
            return Err(
                "leader-wire lifecycle gate can bind only to an empty closed ingress".to_owned(),
            );
        }
        if state.leader_wire_lifecycle_gate.is_some()
            || state.leader_wire_lifecycle_ordinals.is_some()
            || state.leader_wire_context.is_some()
            || !state.leader_wire_lifecycles.is_empty()
        {
            return Err("leader-wire lifecycle gate is already bound".to_owned());
        }
        if state
            .certified_serve_gate
            .as_ref()
            .is_some_and(|gate| !gate.shares_lifecycle_ordinals(&lifecycle_ordinals))
        {
            return Err(
                "leader-wire gate changed the actor-global lifecycle ordinal source".to_owned(),
            );
        }
        if gate.restore()? != restore {
            return Err("leader-wire lifecycle restore changed before binding".to_owned());
        }
        let capacity = fair_v2_ingress_leader_wire_lifecycle_capacity(
            state.roster.len(),
            state.leader_wire_max_chunk_count,
        )
        .ok_or_else(|| "leader-wire lifecycle binding capacity overflowed".to_owned())?;
        if !gate.matches_geometry(
            context_id,
            height,
            &state.roster,
            capacity,
            state.leader_wire_max_chunk_count,
        ) {
            return Err("leader-wire lifecycle gate changed frozen geometry".to_owned());
        }

        let mut records = BTreeMap::new();
        let mut ingress_ordinals = BTreeSet::new();
        let mut scheduler_ordinals = BTreeSet::new();
        let mut scheduler_high_watermark = 0;
        for restored in restore.records() {
            let token = restored.token().clone();
            if !token.validate_exact(
                context_id,
                height,
                &state.roster,
                state.leader_wire_max_chunk_count,
            ) || token.admission_ordinal > restore.last_admission_ordinal()
                || token.scheduler_ordinal > restore.scheduler_ordinal_high_watermark()
                || !ingress_ordinals.insert(token.admission_ordinal)
                || !scheduler_ordinals.insert(token.scheduler_ordinal)
            {
                return Err("leader-wire restore crossed configured ingress geometry".to_owned());
            }
            let status = match restored.status() {
                serviced_candidate_store::LeaderWireLifecycleStatus::Dormant => {
                    if restored.terminal_evidence().is_some() {
                        return Err(
                            "active leader-wire restore carried terminal evidence".to_owned()
                        );
                    }
                    FairV2IngressLeaderWireStatus::Dormant
                }
                serviced_candidate_store::LeaderWireLifecycleStatus::Terminal => {
                    if restored.runtime_owner().is_none() || restored.terminal_evidence().is_none()
                    {
                        return Err(
                            "terminal leader-wire restore lost its typed stable evidence"
                                .to_owned(),
                        );
                    }
                    FairV2IngressLeaderWireStatus::Terminal
                }
                serviced_candidate_store::LeaderWireLifecycleStatus::Ingress
                | serviced_candidate_store::LeaderWireLifecycleStatus::Runtime
                | serviced_candidate_store::LeaderWireLifecycleStatus::VolatileTerminal => {
                    return Err(
                        "leader-wire gate did not normalize an active restart owner".to_owned()
                    );
                }
            };
            scheduler_high_watermark = scheduler_high_watermark.max(token.scheduler_ordinal);
            if records
                .insert(
                    token.slot.clone(),
                    FairV2IngressLeaderWireRecord {
                        token,
                        status,
                        restored_runtime_owner: restored.runtime_owner(),
                        // Volatile fair queues do not survive process restart.
                        ingress_predecessors: BTreeMap::new(),
                    },
                )
                .is_some()
            {
                return Err("leader-wire restore repeated a bounded owner slot".to_owned());
            }
        }
        if scheduler_high_watermark > restore.scheduler_ordinal_high_watermark() {
            return Err("leader-wire restore lost its scheduler high-watermark".to_owned());
        }
        lifecycle_ordinals.advance_past(restore.scheduler_ordinal_high_watermark())?;
        state.last_admission_ordinal = state
            .last_admission_ordinal
            .max(restore.last_admission_ordinal());
        state.leader_wire_lifecycles = records;
        state.leader_wire_lifecycle_gate = Some(gate);
        state.leader_wire_lifecycle_ordinals = Some(lifecycle_ordinals);
        state.leader_wire_context = Some((context_id, height));
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Apply a live safety-WAL recovery cut to carrierless leader-wire owners.
    ///
    /// Production ingress holds this mirror lock while the durable gate
    /// publishes first. Only restart-restored Dormant records can disappear;
    /// Ingress and Runtime records retain their physical/consumer ownership
    /// until their ordinary terminal path completes.
    pub(crate) fn advance_leader_wire_recovery_cut(
        &self,
        next: serviced_candidate_store::LeaderWireRecoveryAuthority,
    ) -> Result<usize, String> {
        let mut state = self.state.lock();
        if !state.requires_leader_wire_lifecycle_gate {
            return Ok(0);
        }
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| {
                "leader-wire recovery cut crossed an unbound lifecycle gate".to_owned()
            })?;
        let retiring = state
            .leader_wire_lifecycles
            .iter()
            .filter_map(|(slot, record)| {
                (record.status == FairV2IngressLeaderWireStatus::Dormant
                    && next.obsoletes(&record.token))
                .then(|| slot.clone())
            })
            .collect::<BTreeSet<_>>();

        gate.advance_recovery_cut(next, &retiring)?;
        for slot in &retiring {
            let removed = state
                .leader_wire_lifecycles
                .remove(slot)
                .expect("durably retired dormant leader-wire slot remains mirrored");
            debug_assert_eq!(removed.status, FairV2IngressLeaderWireStatus::Dormant);
        }
        self.debug_assert_consistent(&state);
        Ok(retiring.len())
    }

    /// Detach a closed height's durable productive-wire owner.
    pub(crate) fn unbind_leader_wire_lifecycle_gate(
        &self,
        gate: &Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    ) -> Result<(), String> {
        let mut state = self.state.lock();
        if state.open || state.len != 0 {
            return Err(
                "leader-wire lifecycle gate cannot unbind from nonempty open ingress".to_owned(),
            );
        }
        let Some(bound) = state.leader_wire_lifecycle_gate.as_ref() else {
            return Ok(());
        };
        if !serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(bound, gate) {
            return Err("leader-wire lifecycle gate changed per-height ownership".to_owned());
        }
        state.leader_wire_lifecycle_gate = None;
        state.leader_wire_lifecycle_ordinals = None;
        state.leader_wire_context = None;
        state.leader_wire_lifecycles.clear();
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Retire every closed-height carrier and atomically detach both durable
    /// ingress gates.
    ///
    /// Productive leader-wire records describe physical entries in the same
    /// lanes that carry certified Serve reservations. Clearing those lanes
    /// before detaching the leader-wire gate would transiently leave a durable
    /// `Ingress` record without its unique carrier. Keep the two per-height
    /// ownership cuts under one ingress transaction instead.
    ///
    /// This detach deliberately does not forge a backward `Ingress` to
    /// `Dormant` refinement in the persistent leader-wire gate. On shutdown or
    /// abnormal runner exit, same-height restart reconciliation normalizes
    /// active records to selector-dormant `Dormant`. After durable height
    /// finality, replay's decision authority instead retires the obsolete
    /// records. Both paths retain the ordinal high-watermarks.
    pub(crate) fn unbind_height_ingress_gates(
        &self,
        certified_serve_gate: &v2_worker::CertifiedServeIngressGate,
        leader_wire_gate: &Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    ) -> Result<(), String> {
        let _service_guard = self.service_lock.lock();
        let mut state = self.state.lock();
        if state.open {
            return Err("height ingress gates cannot unbind from open ingress".to_owned());
        }
        let bound_certified_serve = state
            .certified_serve_gate
            .as_ref()
            .ok_or_else(|| "height ingress lost its certified Serve gate".to_owned())?;
        if !bound_certified_serve.ptr_eq(certified_serve_gate) {
            return Err("certified Serve gate changed per-height I/O ownership".to_owned());
        }
        let bound_leader_wire = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .ok_or_else(|| "height ingress lost its leader-wire lifecycle gate".to_owned())?;
        if !serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(
            bound_leader_wire,
            leader_wire_gate,
        ) {
            return Err("leader-wire lifecycle gate changed per-height ownership".to_owned());
        }

        // Every queued carrier belongs to the closed height. Replacing the
        // lanes drops each Serve RAII ticket while the ingress lock is held;
        // ticket rollback takes only the I/O lock, and no I/O path calls back
        // into fair ingress.
        let mut lanes = BTreeMap::new();
        for peer in &state.roster {
            lanes.insert(
                FairV2IngressSource::Validator(peer.clone()),
                FairV2IngressLane::default(),
            );
        }
        lanes.insert(FairV2IngressSource::Anonymous, FairV2IngressLane::default());
        state.lanes = lanes;
        state.pending_wire_owners.clear();
        state.ready.clear();
        state.len = 0;
        state.bytes = 0;
        state.nonempty_since = None;
        state.last_service_attempt_at = None;

        let detached_certified_serve = state
            .certified_serve_gate
            .take()
            .expect("validated certified Serve gate remains bound");
        debug_assert!(detached_certified_serve.ptr_eq(certified_serve_gate));
        let detached_leader_wire = state
            .leader_wire_lifecycle_gate
            .take()
            .expect("validated leader-wire lifecycle gate remains bound");
        debug_assert!(
            serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(
                &detached_leader_wire,
                leader_wire_gate,
            )
        );
        state.leader_wire_lifecycle_ordinals = None;
        state.leader_wire_context = None;
        state.leader_wire_lifecycles.clear();
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Retire all closed-height occurrences, then detach their exact Serve gate.
    pub(crate) fn unbind_certified_serve_gate(
        &self,
        gate: &v2_worker::CertifiedServeIngressGate,
    ) -> Result<(), String> {
        let _service_guard = self.service_lock.lock();
        let mut state = self.state.lock();
        if state.open {
            return Err("certified Serve gate cannot unbind from open ingress".to_owned());
        }
        let Some(bound) = state.certified_serve_gate.as_ref() else {
            return Ok(());
        };
        if !bound.ptr_eq(gate) {
            return Err("certified Serve gate changed per-height I/O ownership".to_owned());
        }

        // Every queued carrier belongs to the closed height. Replacing the
        // lanes drops each RAII ticket while the ingress lock is held; ticket
        // rollback takes only the I/O lock, and no I/O path calls back here.
        let mut lanes = BTreeMap::new();
        for peer in &state.roster {
            lanes.insert(
                FairV2IngressSource::Validator(peer.clone()),
                FairV2IngressLane::default(),
            );
        }
        lanes.insert(FairV2IngressSource::Anonymous, FairV2IngressLane::default());
        state.lanes = lanes;
        state.pending_wire_owners.clear();
        for record in state.leader_wire_lifecycles.values_mut() {
            record.ingress_predecessors.clear();
        }
        state.ready.clear();
        state.len = 0;
        state.bytes = 0;
        state.nonempty_since = None;
        state.last_service_attempt_at = None;
        let detached = state
            .certified_serve_gate
            .take()
            .expect("validated certified Serve gate remains bound");
        debug_assert!(detached.ptr_eq(gate));
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Open admission for the already-configured immutable height.
    pub(crate) fn open(&self) -> Result<(), FairV2IngressCapacityError> {
        let mut state = self.state.lock();
        if state.requires_certified_serve_gate && state.certified_serve_gate.is_none() {
            return Err(FairV2IngressCapacityError {
                configured: 0,
                required: 1,
                kind: FairV2IngressCapacityKind::CertifiedServeGate,
            });
        }
        if state.requires_leader_wire_lifecycle_gate
            && (state.leader_wire_lifecycle_gate.is_none()
                || state.leader_wire_lifecycle_ordinals.is_none()
                || state.leader_wire_context.is_none())
        {
            return Err(FairV2IngressCapacityError {
                configured: 0,
                required: 1,
                kind: FairV2IngressCapacityKind::LeaderWireLifecycleGate,
            });
        }
        if let (Some(gate), Some(source)) = (
            state.certified_serve_gate.as_ref(),
            state.leader_wire_lifecycle_ordinals.as_ref(),
        ) && !gate.shares_lifecycle_ordinals(source)
        {
            return Err(FairV2IngressCapacityError {
                configured: 0,
                required: 1,
                kind: FairV2IngressCapacityKind::LeaderWireLifecycleGate,
            });
        }
        let Some(required) = fair_v2_ingress_required_capacity(
            state.roster.len(),
            self.authenticated_non_validator_source_capacity,
        ) else {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::Messages,
            });
        };
        if required > self.capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required,
                kind: FairV2IngressCapacityKind::Messages,
            });
        }
        if self.certified_fence_escape_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.certified_fence_escape_byte_reserve,
                kind: FairV2IngressCapacityKind::CertifiedFenceEscapeBytes,
            });
        }
        if self.timeout_vote_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.timeout_vote_byte_reserve,
                kind: FairV2IngressCapacityKind::TimeoutVoteBytes,
            });
        }
        let Some(required_reserved_bytes) = self
            .certified_fence_escape_byte_reserve
            .checked_add(self.timeout_vote_byte_reserve)
            .and_then(|reserved| reserved.checked_add(self.transport_completion_byte_reserve))
        else {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        };
        if required_reserved_bytes > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: required_reserved_bytes,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        }
        let ordinary_bytes = self.source_byte_capacity - required_reserved_bytes;
        if ordinary_bytes < state.required_ordinary_bytes {
            return Err(FairV2IngressCapacityError {
                configured: ordinary_bytes,
                required: state.required_ordinary_bytes,
                kind: FairV2IngressCapacityKind::OrdinaryBytes,
            });
        }
        if state.required_certified_fence_escape_bytes > self.certified_fence_escape_byte_reserve {
            return Err(FairV2IngressCapacityError {
                configured: self.certified_fence_escape_byte_reserve,
                required: state.required_certified_fence_escape_bytes,
                kind: FairV2IngressCapacityKind::CertifiedFenceEscapeBytes,
            });
        }
        if state.required_transport_completion_bytes > self.transport_completion_byte_reserve {
            return Err(FairV2IngressCapacityError {
                configured: self.transport_completion_byte_reserve,
                required: state.required_transport_completion_bytes,
                kind: FairV2IngressCapacityKind::TransportCompletionBytes,
            });
        }
        if state.required_consensus_frame_bytes > self.consensus_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.consensus_frame_byte_capacity,
                required: state.required_consensus_frame_bytes,
                kind: FairV2IngressCapacityKind::ConsensusFrameBytes,
            });
        }
        if state.required_control_frame_bytes > self.control_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.control_frame_byte_capacity,
                required: state.required_control_frame_bytes,
                kind: FairV2IngressCapacityKind::ControlFrameBytes,
            });
        }
        if state.required_block_sync_frame_bytes > self.block_sync_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.block_sync_frame_byte_capacity,
                required: state.required_block_sync_frame_bytes,
                kind: FairV2IngressCapacityKind::BlockSyncFrameBytes,
            });
        }
        if state.required_outbound_high_frame_bytes > self.outbound_high_frame_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.outbound_high_frame_byte_capacity,
                required: state.required_outbound_high_frame_bytes,
                kind: FairV2IngressCapacityKind::OutboundHighFrameBytes,
            });
        }
        let Some(required_bytes) = fair_v2_ingress_required_byte_capacity(
            state.roster.len(),
            self.authenticated_non_validator_source_capacity,
            self.source_byte_capacity,
        ) else {
            return Err(FairV2IngressCapacityError {
                configured: self.byte_capacity,
                required: usize::MAX,
                kind: FairV2IngressCapacityKind::Bytes,
            });
        };
        if required_bytes > self.byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.byte_capacity,
                required: required_bytes,
                kind: FairV2IngressCapacityKind::Bytes,
            });
        }
        state.open = true;
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Close admission before rollover or abnormal runner exit.
    pub(crate) fn close(&self) {
        self.state.lock().open = false;
    }

    fn mark_leader_wire_runtime_locked(
        state: &mut FairV2IngressState,
        token: &FairV2IngressLeaderWireToken,
        owner: serviced_candidate_store::LeaderWireRuntimeOwner,
    ) -> Result<serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt, String> {
        if owner.admission_ordinal() != token.scheduler_ordinal() {
            return Err("leader-wire runtime changed its shared scheduler ordinal".to_owned());
        }
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| "leader-wire runtime crossed an unbound lifecycle gate".to_owned())?;
        let record = state
            .leader_wire_lifecycles
            .get(&token.slot)
            .ok_or_else(|| "leader-wire runtime has no ingress record".to_owned())?;
        if record.token != *token
            || record.status != FairV2IngressLeaderWireStatus::Ingress
            || record
                .restored_runtime_owner
                .is_some_and(|restored| restored != owner)
        {
            return Err("leader-wire runtime changed exact ingress ownership".to_owned());
        }
        let receipt = gate.mark_runtime(token, owner)?;
        let record = state
            .leader_wire_lifecycles
            .get_mut(&token.slot)
            .expect("validated leader-wire ingress record remains bound");
        record.status = FairV2IngressLeaderWireStatus::Runtime;
        record.restored_runtime_owner = Some(owner);
        record.ingress_predecessors.clear();
        Ok(receipt)
    }

    fn bind_leader_wire_runtime_ownership_locked(
        state: &mut FairV2IngressState,
        ownership: &mut FairV2IngressOwnershipEvidence,
    ) -> Result<(), String> {
        if !ownership.validate_exact() {
            return Err("leader-wire runtime received invalid ingress ownership".to_owned());
        }
        let Some(token) = ownership.leader_wire_token().cloned() else {
            return if ownership.leader_wire_runtime_receipt().is_none() {
                Ok(())
            } else {
                Err("nonproductive ingress carried a leader-wire runtime receipt".to_owned())
            };
        };
        if let Some(receipt) = ownership.leader_wire_runtime_receipt() {
            return (receipt.token() == &token
                && receipt.owner().causal_lifecycle_key() == token.identity_hash()
                && receipt.owner().admission_ordinal() == token.scheduler_ordinal())
            .then_some(())
            .ok_or_else(|| "leader-wire runtime receipt changed immutable ownership".to_owned());
        }
        let record = state
            .leader_wire_lifecycles
            .get(&token.slot)
            .ok_or_else(|| "leader-wire token has no bound lifecycle record".to_owned())?;
        if record.token != token {
            return Err("leader-wire runtime rebind changed immutable token".to_owned());
        }
        let owner = record.restored_runtime_owner.map_or_else(
            || {
                serviced_candidate_store::LeaderWireRuntimeOwner::new(
                    token.identity_hash(),
                    token.scheduler_ordinal(),
                )
            },
            Ok,
        )?;
        if owner.causal_lifecycle_key() != token.identity_hash()
            || owner.admission_ordinal() != token.scheduler_ordinal()
        {
            return Err("restored leader-wire runtime changed its token identity".to_owned());
        }
        let receipt = Self::mark_leader_wire_runtime_locked(state, &token, owner)?;
        if !ownership.install_leader_wire_runtime_receipt(receipt) {
            return Err("leader-wire runtime receipt could not bind ingress ownership".to_owned());
        }
        Ok(())
    }

    /// Bind a drained productive carrier to its immutable generic runtime.
    pub(crate) fn bind_leader_wire_runtime_ownership(
        &self,
        ownership: &mut FairV2IngressOwnershipEvidence,
    ) -> Result<(), String> {
        let mut state = self.state.lock();
        Self::bind_leader_wire_runtime_ownership_locked(&mut state, ownership)?;
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Publish a process-local terminal which must reopen after a crash.
    pub(crate) fn mark_leader_wire_volatile_terminal(
        &self,
        runtime: &serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
    ) -> Result<(), String> {
        self.mark_leader_wire_volatile_terminal_checked(runtime, false)
    }

    /// Publish a volatile terminal only when the live safety-WAL cut has
    /// permanently obsoleted this exact runtime owner.
    pub(crate) fn mark_obsolete_leader_wire_volatile_terminal(
        &self,
        runtime: &serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
    ) -> Result<(), String> {
        self.mark_leader_wire_volatile_terminal_checked(runtime, true)
    }

    fn mark_leader_wire_volatile_terminal_checked(
        &self,
        runtime: &serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
        require_obsolete: bool,
    ) -> Result<(), String> {
        let token = runtime.token();
        let mut state = self.state.lock();
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| {
                "leader-wire volatile terminal crossed an unbound lifecycle gate".to_owned()
            })?;
        let record = state
            .leader_wire_lifecycles
            .get(&token.slot)
            .ok_or_else(|| "leader-wire volatile terminal has no runtime record".to_owned())?;
        if record.token != *token
            || !matches!(
                record.status,
                FairV2IngressLeaderWireStatus::Runtime
                    | FairV2IngressLeaderWireStatus::VolatileTerminal
            )
            || record.restored_runtime_owner != Some(runtime.owner())
        {
            return Err("leader-wire volatile terminal changed runtime ownership".to_owned());
        }
        if require_obsolete && !gate.identity_is_obsolete(&token.identity)? {
            return Err(
                "leader-wire obsolete terminal lacks durable recovery authority".to_owned(),
            );
        }
        gate.mark_volatile_terminal(runtime)?;
        let record = state
            .leader_wire_lifecycles
            .get_mut(&token.slot)
            .expect("validated leader-wire runtime record remains bound");
        record.status = FairV2IngressLeaderWireStatus::VolatileTerminal;
        record.ingress_predecessors.clear();
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Publish a restart-stable terminal after exact producer evidence exists.
    pub(crate) fn mark_leader_wire_producer_terminal(
        &self,
        runtime: &serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
        producer_terminal: serviced_candidate_store::ProducerContinuationTerminalToken,
    ) -> Result<(), String> {
        let token = runtime.token();
        let mut state = self.state.lock();
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| "leader-wire terminal crossed an unbound lifecycle gate".to_owned())?;
        let record = state
            .leader_wire_lifecycles
            .get(&token.slot)
            .ok_or_else(|| "leader-wire terminal has no runtime record".to_owned())?;
        if record.token != *token
            || !matches!(
                record.status,
                FairV2IngressLeaderWireStatus::Runtime
                    | FairV2IngressLeaderWireStatus::VolatileTerminal
                    | FairV2IngressLeaderWireStatus::Terminal
            )
            || record.restored_runtime_owner != Some(runtime.owner())
        {
            return Err("leader-wire terminal changed exact runtime ownership".to_owned());
        }
        gate.mark_producer_terminal(runtime, producer_terminal)?;
        let record = state
            .leader_wire_lifecycles
            .get_mut(&token.slot)
            .expect("validated leader-wire runtime record remains bound");
        record.status = FairV2IngressLeaderWireStatus::Terminal;
        record.ingress_predecessors.clear();
        self.debug_assert_consistent(&state);
        Ok(())
    }

    /// Publish a restart-stable terminal from independently durable body bytes.
    pub(crate) fn mark_leader_wire_durable_body_terminal(
        &self,
        runtime: &serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
        durable_body: &v2_body_store::DurableBodyReceipt,
    ) -> Result<(), String> {
        let token = runtime.token();
        let mut state = self.state.lock();
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| {
                "leader-wire body terminal crossed an unbound lifecycle gate".to_owned()
            })?;
        let record = state
            .leader_wire_lifecycles
            .get(&token.slot)
            .ok_or_else(|| "leader-wire body terminal has no runtime record".to_owned())?;
        if record.token != *token
            || !matches!(
                record.status,
                FairV2IngressLeaderWireStatus::Runtime
                    | FairV2IngressLeaderWireStatus::VolatileTerminal
                    | FairV2IngressLeaderWireStatus::Terminal
            )
            || record.restored_runtime_owner != Some(runtime.owner())
        {
            return Err("leader-wire body terminal changed exact runtime ownership".to_owned());
        }
        gate.mark_durable_body_terminal(runtime, durable_body)?;
        let record = state
            .leader_wire_lifecycles
            .get_mut(&token.slot)
            .expect("validated leader-wire runtime record remains bound");
        record.status = FairV2IngressLeaderWireStatus::Terminal;
        record.ingress_predecessors.clear();
        self.debug_assert_consistent(&state);
        Ok(())
    }

    fn try_push(
        &self,
        inbound: InboundBlockMessage,
    ) -> Result<FairV2IngressPushDisposition, FairV2IngressPushError> {
        self.try_push_at(inbound, Instant::now())
    }

    fn try_push_at(
        &self,
        mut inbound: InboundBlockMessage,
        enqueued_at: Instant,
    ) -> Result<FairV2IngressPushDisposition, FairV2IngressPushError> {
        let class = FairV2IngressClass::classify(&inbound);
        let Some(message_kind) = FairV2IngressMessageKind::classify(inbound.message()) else {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::UnsupportedMessageKind,
            ));
        };
        let is_timeout_vote = fair_v2_ingress_is_timeout_vote(&inbound);
        let is_transport_completion = class == FairV2IngressClass::TransportCompletion;
        let encoded = match inbound.message() {
            BlockMessage::V2(message) => {
                if message.validate_version().is_err() {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::WrongProtocolVersion,
                    ));
                }
                Arc::<[u8]>::from(message.encode())
            }
            message if message.is_lane_local() => {
                let encoded = Arc::<[u8]>::from(message.encode());
                let encoded_len = encoded.len();
                let lane_limit = match message {
                    // Canonical carrier recovery is bounded by the same
                    // configured completion partition and ConsensusPayload
                    // frame as certified global bodies. It is not an
                    // autonomous lane source bundle.
                    BlockMessage::LaneHistoricalRecoveryResponse(response)
                        if matches!(
                            &response.payload,
                            self::message::LaneHistoricalRecoveryPayloadV1::CanonicalBlock { .. }
                        ) =>
                    {
                        self.transport_completion_byte_reserve
                    }
                    _ if class == FairV2IngressClass::TransportCompletion => {
                        MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES
                    }
                    _ => MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES,
                };
                if encoded_len > lane_limit {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::MessageTooLarge,
                    ));
                }
                encoded
            }
            BlockMessage::KuraReplicaAdvert(advert) => {
                if inbound.sender() != Some(&advert.keeper)
                    || inbound.via() != Some(&advert.keeper)
                    || advert.verify_keeper_signature().is_err()
                {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::OwnershipEvidenceInvalid,
                    ));
                }
                let encoded = Arc::<[u8]>::from(inbound.message().encode());
                if encoded.len() > crate::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::MessageTooLarge,
                    ));
                }
                encoded
            }
            _ => {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::UnsupportedEnvelope,
                ));
            }
        };
        let encoded_len = encoded.len();
        let wire_hash = CryptoHash::new(encoded.as_ref());
        // Delivery deduplication remains scoped to the semantic origin: two
        // requesters behind one trusted relay can require distinct responses.
        // Count, byte, and fair-service ownership below is instead charged to
        // the authenticated hop so origin churn cannot multiply resources.
        let wire_key = Some(FairV2IngressWireKey {
            origin: inbound.sender.clone(),
            hash: wire_hash,
        });
        let mut state = self.state.lock();
        if !state.open {
            return Err(FairV2IngressPushError::Closed(inbound));
        }
        let source = match inbound.via() {
            Some(peer) if state.roster.contains(peer) => {
                FairV2IngressSource::Validator(peer.clone())
            }
            Some(peer) => FairV2IngressSource::Authenticated(peer.clone()),
            None => FairV2IngressSource::Anonymous,
        };
        let authenticated_via_is_validator = matches!(&source, FairV2IngressSource::Validator(_));
        if let Some((key, owner_source)) = wire_key.as_ref().and_then(|key| {
            state
                .pending_wire_owners
                .get(key)
                .cloned()
                .map(|owner| (key, owner))
        }) {
            let resource = self.ownership_resource_snapshot(&state, &source);
            let lane = state
                .lanes
                .get(&owner_source)
                .expect("globally indexed fair-ingress owner lane remains present");
            let queued = lane
                .entries
                .iter()
                .find(|entry| entry.wire_key.as_ref() == Some(key))
                .expect("global pending wire key has one queued owner");
            if queued.encoded_bytes.as_ref() != encoded.as_ref()
                || queued.class != class
                || queued
                    .inbound
                    .ingress_ownership
                    .as_ref()
                    .is_none_or(|evidence| !evidence.validate_exact())
            {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::PendingWireOwnershipMismatch,
                ));
            }
            let routes_before = queued.inbound.reply_routes.clone();
            let routes_candidate = inbound.reply_routes.clone();
            let prior_evidence = queued
                .inbound
                .ingress_ownership
                .as_ref()
                .expect("every queued semantic owner retains ingress evidence");
            let action = match fair_v2_ingress_route_action(
                &prior_evidence.attempts,
                routes_candidate.as_ref(),
            ) {
                Ok(action) => action,
                Err(_) => {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::RouteOwnershipInvalid,
                    ));
                }
            };
            let routes_after = match (&routes_before, &routes_candidate) {
                (Some(retained), Some(candidate)) => {
                    let mut merged = retained.clone();
                    let Ok(receipt) = merged.merge_with_receipt(candidate) else {
                        return Err(FairV2IngressPushError::rejected(
                            inbound,
                            FairV2IngressRejectReason::RouteOwnershipInvalid,
                        ));
                    };
                    let Some(receipt_output) = receipt.into_output(retained, candidate) else {
                        return Err(FairV2IngressPushError::rejected(
                            inbound,
                            FairV2IngressRejectReason::RouteOwnershipInvalid,
                        ));
                    };
                    Some(receipt_output)
                }
                (None, Some(candidate)) => Some(candidate.clone()),
                (Some(retained), None) => Some(retained.clone()),
                (None, None) => None,
            };
            let Some(route_capacity) = fair_v2_ingress_route_capacity(
                routes_before.as_ref(),
                routes_candidate.as_ref(),
                routes_after.as_ref(),
            ) else {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::RouteOwnershipInvalid,
                ));
            };
            let attempts_before = prior_evidence.attempts.clone();
            let candidate_attempts =
                fair_v2_ingress_attempts_for_routes(&attempts_before, routes_candidate.as_ref());
            let Some(attempts_after) = fair_v2_ingress_merge_attempt_cursors(
                &attempts_before,
                &candidate_attempts,
                routes_after.as_ref(),
            ) else {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::AttemptCursorInvalid,
                ));
            };
            let attempts_before_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_before);
            let attempts_after_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_after);
            let occurrence = FairV2IngressOwnershipOccurrence {
                action,
                physical_admission_ordinal: queued.admission_ordinal,
                lifecycle_ordinal: prior_evidence.first.lifecycle_ordinal,
                wire_key: key.clone(),
                semantic_origin: inbound.sender.clone(),
                authenticated_via: inbound.via.clone(),
                authenticated_via_is_validator,
                authenticated_source: source,
                semantic_owner_source: owner_source.clone(),
                message_kind,
                class,
                encoded_bytes: Arc::clone(&queued.encoded_bytes),
                encoded_len,
                resource_before: resource.clone(),
                resource_after: resource,
                routes_before,
                routes_candidate,
                routes_after: routes_after.clone(),
                route_capacity,
                attempts_before,
                attempts_before_hash,
                attempts_after,
                attempts_after_hash,
            };
            if !occurrence.validate_exact() {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::OwnershipEvidenceInvalid,
                ));
            }
            let Some(evidence) = prior_evidence.merged(occurrence) else {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::OwnershipEvidenceInvalid,
                ));
            };
            let lane = state
                .lanes
                .get_mut(&owner_source)
                .expect("globally indexed fair-ingress owner lane remains present");
            let queued = lane
                .entries
                .iter_mut()
                .find(|entry| entry.wire_key.as_ref() == Some(key))
                .expect("global pending wire key has one queued owner");
            let queued_inbound = Arc::make_mut(&mut queued.inbound);
            queued_inbound.reply_routes = routes_after;
            queued_inbound.ingress_ownership = Some(evidence);
            return Ok(FairV2IngressPushDisposition::Coalesced);
        }
        if !fair_v2_ingress_is_certified_body_request(&inbound) {
            let dormant_serve_debt = match state.certified_serve_gate.as_ref() {
                Some(gate) => match gate.dormant_ingress_scheduler_ordinal() {
                    Ok(dormant) => dormant.is_some(),
                    Err(_) => {
                        state.open = false;
                        return Err(FairV2IngressPushError::FailStop(inbound));
                    }
                },
                None if state.requires_certified_serve_gate => {
                    state.open = false;
                    return Err(FairV2IngressPushError::FailStop(inbound));
                }
                None => false,
            };
            if dormant_serve_debt {
                // Production startup never exposes carrierless Serve debt.
                // Its appearance in a live height is invariant evidence, not
                // backpressure that a requester must repair. Close fair
                // ingress so the runner restarts into local startup discharge.
                state.open = false;
                return Err(FairV2IngressPushError::FailStop(inbound));
            }
        }
        let source_lane_is_new = !state.lanes.contains_key(&source);
        if source_lane_is_new && !matches!(source, FairV2IngressSource::Authenticated(_)) {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::SourceLaneInvalid,
            ));
        }
        let empty_lane = FairV2IngressLane::default();
        let lane = state.lanes.get(&source).unwrap_or(&empty_lane);
        let lane_certified_fence_escape_len = lane.certified_fence_escape_len;
        let lane_timeout_vote_len = lane.timeout_vote_len;
        let lane_transport_completion_len = lane.transport_completion_len;
        let is_validator_source = matches!(source, FairV2IngressSource::Validator(_));
        let uses_certified_fence_escape_reserve = !matches!(source, FairV2IngressSource::Anonymous)
            && fair_v2_ingress_is_certified_fence_escape(&inbound);
        let is_current_validator_origin = inbound
            .sender()
            .is_some_and(|peer| state.roster.contains(peer));
        let is_historical_recovery_response =
            message_kind == FairV2IngressMessageKind::LaneHistoricalRecoveryResponse;
        // Ordinary transport completions are protocol-valid only for a
        // current frozen-roster semantic origin. A historical lane-recovery
        // response is the one narrow exception: its responder authority comes
        // from the outstanding request's frozen CommitQC or READY certificate,
        // which the lane adapter verifies before persistence. Keep that proof-
        // carrying response in the bounded completion partition and require an
        // authenticated semantic origin, so a validator removed from the
        // successor roster can finish an old lane without granting arbitrary
        // old peers current-height completion authority.
        let authenticated_historical_recovery_response = is_historical_recovery_response
            && inbound.sender().is_some()
            && inbound.via().is_some();
        if is_transport_completion
            && !is_current_validator_origin
            && !authenticated_historical_recovery_response
        {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::UnauthorizedTransportCompletion,
            ));
        }
        let (owned_class_bytes, source_class_byte_limit) = if uses_certified_fence_escape_reserve {
            (
                lane.certified_fence_escape_bytes,
                self.certified_fence_escape_byte_reserve,
            )
        } else if is_validator_source && is_timeout_vote {
            (lane.timeout_vote_bytes, self.timeout_vote_byte_reserve)
        } else if is_transport_completion {
            (
                lane.transport_completion_bytes,
                self.transport_completion_byte_reserve,
            )
        } else if is_validator_source {
            let reserved_bytes = lane
                .certified_fence_escape_bytes
                .checked_add(lane.timeout_vote_bytes)
                .and_then(|reserved| reserved.checked_add(lane.transport_completion_bytes))
                .expect("configured per-source byte limit prevents overflow");
            (
                lane.bytes
                    .checked_sub(reserved_bytes)
                    .expect("reserved byte owners are included in the source total"),
                self.source_byte_capacity
                    .saturating_sub(self.certified_fence_escape_byte_reserve)
                    .saturating_sub(self.timeout_vote_byte_reserve)
                    .saturating_sub(self.transport_completion_byte_reserve),
            )
        } else if matches!(source, FairV2IngressSource::Authenticated(_)) {
            let reserved_bytes = lane
                .certified_fence_escape_bytes
                .checked_add(lane.transport_completion_bytes)
                .expect("configured per-source byte limit prevents overflow");
            (
                lane.bytes
                    .checked_sub(reserved_bytes)
                    .expect("reserved byte owners are included in the source total"),
                self.source_byte_capacity
                    .saturating_sub(self.certified_fence_escape_byte_reserve)
                    .saturating_sub(self.transport_completion_byte_reserve),
            )
        } else {
            (
                lane.bytes
                    .checked_sub(lane.transport_completion_bytes)
                    .expect("non-validator completion bytes are included in the source total"),
                self.source_byte_capacity
                    .saturating_sub(self.transport_completion_byte_reserve),
            )
        };
        if encoded_len > source_class_byte_limit || encoded_len > self.byte_capacity {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::MessageTooLarge,
            ));
        }

        let routes_candidate = inbound.reply_routes.clone();
        let routes_after = routes_candidate.clone();
        let Some(route_capacity) =
            fair_v2_ingress_route_capacity(None, routes_candidate.as_ref(), routes_after.as_ref())
        else {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::RouteOwnershipInvalid,
            ));
        };
        let attempts_before = Vec::new();
        let attempts_after = fair_v2_ingress_attempts_for_routes(&[], routes_after.as_ref());
        let attempts_before_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_before);
        let attempts_after_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_after);

        let leader_wire_derivation = if state.requires_leader_wire_lifecycle_gate
            && fair_v2_ingress_is_productive_leader_wire(inbound.message())
        {
            let Some(semantic_origin) = inbound.sender().cloned() else {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::ProductiveOriginMissing,
                ));
            };
            if !state.roster.contains(&semantic_origin) {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::ProductiveOriginOutsideRoster,
                ));
            }
            let derivation = fair_v2_ingress_leader_wire_identity(
                &state,
                inbound.message(),
                &semantic_origin,
                wire_hash,
            );
            if let FairV2IngressLeaderWireDerivation::Exact { identity, .. } = &derivation {
                let Some((context_id, height)) = state.leader_wire_context else {
                    state.open = false;
                    return Err(FairV2IngressPushError::FailStop(inbound));
                };
                if identity.context_id != context_id || identity.height != height {
                    return Err(FairV2IngressPushError::rejected(
                        inbound,
                        FairV2IngressRejectReason::WrongHeightContext,
                    ));
                }
            }
            derivation
        } else {
            FairV2IngressLeaderWireDerivation::NotApplicable
        };
        let mut ready_leader_wire_derivation = None;
        let mut leader_wire_token = match fair_v2_ingress_admit_leader_wire(
            &mut state,
            leader_wire_derivation.clone(),
            false,
        ) {
            Ok(FairV2IngressLeaderWireAdmission::NotApplicable) => None,
            Ok(FairV2IngressLeaderWireAdmission::Coalesced) => {
                return Ok(FairV2IngressPushDisposition::Coalesced);
            }
            Ok(FairV2IngressLeaderWireAdmission::Admitted(_)) => {
                state.open = false;
                return Err(FairV2IngressPushError::FailStop(inbound));
            }
            Ok(FairV2IngressLeaderWireAdmission::Ready) => {
                ready_leader_wire_derivation = Some(leader_wire_derivation);
                None
            }
            Err(FairV2IngressLeaderWireAdmissionError::Busy) => {
                return Err(FairV2IngressPushError::Full(inbound));
            }
            Err(FairV2IngressLeaderWireAdmissionError::Rejected) => {
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::LeaderWireObsoleteOrConflicting,
                ));
            }
            Err(FairV2IngressLeaderWireAdmissionError::Exhausted) => {
                state.open = false;
                return Err(FairV2IngressPushError::FailStop(inbound));
            }
        };

        // A brand-new or Dormant exact wire remains read-only Ready until every
        // physical-capacity cut below succeeds. No rejected packet can mint a
        // scheduler barrier and then disappear. Once the durable Ingress
        // publication occurs, any impossible local validation failure fails
        // stop rather than stranding the exact token.
        macro_rules! reject_after_leader_wire_admission {
            () => {{
                if leader_wire_token.is_some() {
                    state.open = false;
                    return Err(FairV2IngressPushError::FailStop(inbound));
                }
                return Err(FairV2IngressPushError::rejected(
                    inbound,
                    FairV2IngressRejectReason::OwnershipEvidenceInvalid,
                ));
            }};
        }
        if source_lane_is_new {
            let retained_authenticated_non_validator_sources = state
                .lanes
                .keys()
                .filter(|source| matches!(source, FairV2IngressSource::Authenticated(_)))
                .count();
            if self
                .authenticated_non_validator_source_capacity
                .is_some_and(|capacity| retained_authenticated_non_validator_sources >= capacity)
            {
                return Err(FairV2IngressPushError::Full(inbound));
            }
        }
        // A validator lane has one critical TimeoutVote byte owner. Exact
        // retransmissions were coalesced above; a distinct later-view vote is
        // retried by retained control after fair service releases the owner.
        if is_validator_source && is_timeout_vote && lane_timeout_vote_len != 0 {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        // Every authenticated transport source has one isolated certified
        // signer-fence escape. Ordinary progress cannot consume this owner.
        if uses_certified_fence_escape_reserve && lane_certified_fence_escape_len != 0 {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        // A validator also has one source-isolated payload-completion owner.
        if is_transport_completion && lane_transport_completion_len != 0 {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        if encoded_len > source_class_byte_limit.saturating_sub(owned_class_bytes)
            || encoded_len > self.byte_capacity.saturating_sub(state.bytes)
        {
            return Err(FairV2IngressPushError::Full(inbound));
        }

        // Project the reservation potential after this admission. Every empty
        // source needs a first-message slot. Each validator additionally keeps
        // independent non-timeout Progress and TimeoutVote slots. A short lane
        // retains enough continuation potential that servicing one entry can
        // restore every reservation of the resulting state. The incoming item
        // is part of the projection.
        let protected_slots_after_admission = state
            .lanes
            .iter()
            .map(|(lane_source, lane)| {
                let is_target = *lane_source == source;
                let projected_len = lane.entries.len() + usize::from(is_target);
                let projected_timeout_vote_len =
                    lane.timeout_vote_len + usize::from(is_target && is_timeout_vote);
                let projected_certified_fence_escape_len = lane.certified_fence_escape_len
                    + usize::from(is_target && uses_certified_fence_escape_reserve);
                let projected_transport_completion_len = lane.transport_completion_len
                    + usize::from(is_target && is_transport_completion);
                let projected_ordinary_progress_len = lane
                    .progress_len
                    .saturating_sub(lane.timeout_vote_len)
                    .saturating_sub(lane.certified_fence_escape_len)
                    + usize::from(
                        is_target
                            && class == FairV2IngressClass::Progress
                            && !is_timeout_vote
                            && !uses_certified_fence_escape_reserve,
                    );
                fair_v2_ingress_lane_protected_slots(
                    lane_source.class(),
                    !state.roster.is_empty(),
                    projected_len,
                    projected_ordinary_progress_len != 0,
                    projected_certified_fence_escape_len != 0,
                    projected_timeout_vote_len != 0,
                    projected_transport_completion_len != 0,
                )
            })
            .sum::<usize>();
        let Some(protected_slots_after_admission) =
            protected_slots_after_admission.checked_add(if source_lane_is_new {
                fair_v2_ingress_lane_protected_slots(
                    source.class(),
                    !state.roster.is_empty(),
                    1,
                    class == FairV2IngressClass::Progress
                        && !is_timeout_vote
                        && !uses_certified_fence_escape_reserve,
                    uses_certified_fence_escape_reserve,
                    is_timeout_vote,
                    is_transport_completion,
                )
            } else {
                0
            })
        else {
            reject_after_leader_wire_admission!();
        };
        let Some(materialized_authenticated_after) = state
            .lanes
            .keys()
            .filter(|source| matches!(source, FairV2IngressSource::Authenticated(_)))
            .count()
            .checked_add(usize::from(source_lane_is_new))
        else {
            reject_after_leader_wire_admission!();
        };
        let Some(latent_authenticated_slots_after) = self
            .authenticated_non_validator_source_capacity
            .map_or(Some(0), |capacity| {
                capacity
                    .checked_sub(materialized_authenticated_after)
                    .and_then(|latent| latent.checked_mul(3))
            })
        else {
            reject_after_leader_wire_admission!();
        };
        let Some(protected_slots_after_admission) =
            protected_slots_after_admission.checked_add(latent_authenticated_slots_after)
        else {
            reject_after_leader_wire_admission!();
        };
        let usable_capacity = self
            .capacity
            .saturating_sub(protected_slots_after_admission);
        if state.len >= usable_capacity {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        let resource_before = self.ownership_resource_snapshot(&state, &source);
        let mut resource_after = resource_before.clone();
        let Some(source_len_after) = resource_after.source_len.checked_add(1) else {
            reject_after_leader_wire_admission!();
        };
        let Some(source_bytes_after) = resource_after.source_bytes.checked_add(encoded_len) else {
            reject_after_leader_wire_admission!();
        };
        let Some(global_len_after) = resource_after.global_len.checked_add(1) else {
            reject_after_leader_wire_admission!();
        };
        let Some(global_bytes_after) = resource_after.global_bytes.checked_add(encoded_len) else {
            reject_after_leader_wire_admission!();
        };
        resource_after.source_len = source_len_after;
        resource_after.source_bytes = source_bytes_after;
        resource_after.global_len = global_len_after;
        resource_after.global_bytes = global_bytes_after;
        resource_after.protected_slots = protected_slots_after_admission;
        if class == FairV2IngressClass::Progress {
            resource_after.source_progress_len = resource_after
                .source_progress_len
                .checked_add(1)
                .expect("bounded Progress owner count cannot overflow");
        }
        if uses_certified_fence_escape_reserve {
            resource_after.source_certified_fence_escape_len = resource_after
                .source_certified_fence_escape_len
                .checked_add(1)
                .expect("bounded certified fence-escape owner count cannot overflow");
            resource_after.source_certified_fence_escape_bytes = resource_after
                .source_certified_fence_escape_bytes
                .checked_add(encoded_len)
                .expect("validated certified fence-escape byte reserve prevents overflow");
        }
        if is_timeout_vote {
            resource_after.source_timeout_vote_len = resource_after
                .source_timeout_vote_len
                .checked_add(1)
                .expect("bounded TimeoutVote owner count cannot overflow");
            if is_validator_source {
                resource_after.source_timeout_vote_bytes = resource_after
                    .source_timeout_vote_bytes
                    .checked_add(encoded_len)
                    .expect("validated TimeoutVote byte reserve prevents overflow");
            }
        }
        if is_transport_completion {
            resource_after.source_transport_completion_len = resource_after
                .source_transport_completion_len
                .checked_add(1)
                .expect("bounded transport-completion owner count cannot overflow");
            resource_after.source_transport_completion_bytes = resource_after
                .source_transport_completion_bytes
                .checked_add(encoded_len)
                .expect("validated transport-completion byte reserve prevents overflow");
        }
        // Reserve the physical carrier position before publishing any durable
        // lifecycle transition. A restored logical owner retains its immutable
        // token ordinals, but its new queue occurrence must be strictly newer
        // than every carrier admitted since the restart.
        let Some(carrier_admission_ordinal) = state.last_admission_ordinal.checked_add(1) else {
            state.open = false;
            return Err(FairV2IngressPushError::FailStop(inbound));
        };
        // This is the atomic admission cut: the ingress lock still excludes
        // competing producers, all message/byte/protected-class capacity and
        // physical-ordinal availability have been proved, and the durable
        // lifecycle is published directly as Ingress. A Dormant retry retains
        // its immutable logical identity while freezing a new physical
        // predecessor cut, so all work already admitted after restart remains
        // ahead of its fresh carrier.
        if let Some(derivation) = ready_leader_wire_derivation.take() {
            leader_wire_token =
                match fair_v2_ingress_admit_leader_wire(&mut state, derivation, true) {
                    Ok(FairV2IngressLeaderWireAdmission::Admitted(token)) => Some(token),
                    Ok(
                        FairV2IngressLeaderWireAdmission::NotApplicable
                        | FairV2IngressLeaderWireAdmission::Coalesced
                        | FairV2IngressLeaderWireAdmission::Ready,
                    )
                    | Err(_) => {
                        state.open = false;
                        return Err(FairV2IngressPushError::FailStop(inbound));
                    }
                };
        }
        let mut occurrence = FairV2IngressOwnershipOccurrence {
            action: FairV2IngressOwnershipAction::New,
            physical_admission_ordinal: carrier_admission_ordinal,
            lifecycle_ordinal: leader_wire_token
                .as_ref()
                .map(FairV2IngressLeaderWireToken::scheduler_ordinal),
            wire_key: wire_key
                .as_ref()
                .expect("every admitted fair-ingress message has a canonical wire key")
                .clone(),
            semantic_origin: inbound.sender.clone(),
            authenticated_via: inbound.via.clone(),
            authenticated_via_is_validator,
            authenticated_source: source.clone(),
            semantic_owner_source: source.clone(),
            message_kind,
            class,
            encoded_bytes: Arc::clone(&encoded),
            encoded_len,
            resource_before,
            resource_after,
            routes_before: None,
            routes_candidate,
            routes_after,
            route_capacity,
            attempts_before,
            attempts_before_hash,
            attempts_after,
            attempts_after_hash,
        };
        if !occurrence.validate_exact() {
            if leader_wire_token.is_some() {
                state.open = false;
                return Err(FairV2IngressPushError::FailStop(inbound));
            }
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::OwnershipEvidenceInvalid,
            ));
        }
        let admission_ordinal = carrier_admission_ordinal;
        let certified_request = match inbound.message() {
            BlockMessage::V2(ConsensusMessageV2 {
                payload: ConsensusMessageV2Payload::CertifiedBodyRequest(request),
                ..
            }) => Some(request.clone()),
            _ => None,
        };
        let certified_serve_reservation = if let Some(request) = certified_request.as_ref() {
            if matches!(
                inbound.message(),
                BlockMessage::V2(message) if message.validate_version().is_err()
            ) {
                reject_after_leader_wire_admission!();
            }
            if inbound.sender() != Some(&request.requester) {
                reject_after_leader_wire_admission!();
            }
            let Some(reply_routes) = inbound.reply_routes() else {
                // A reply capability is a physical transport prerequisite,
                // not part of the signed logical request. Reject its absence
                // before the durable gate can mint a lifecycle or touch the
                // actor-global scheduler source.
                reject_after_leader_wire_admission!();
            };
            if reply_routes.semantic_target() != &request.requester {
                reject_after_leader_wire_admission!();
            }
            let gate = if let Some(gate) = state.certified_serve_gate.clone() {
                Some(gate)
            } else {
                if state.requires_certified_serve_gate {
                    state.open = false;
                    return Err(FairV2IngressPushError::Closed(inbound));
                }
                None
            };
            if let Some(gate) = gate {
                let Some(authenticated_via) = inbound.via().cloned() else {
                    reject_after_leader_wire_admission!();
                };
                let requester_is_roster = state.roster.contains(&request.requester);
                match gate.reserve(
                    request,
                    &authenticated_via,
                    requester_is_roster,
                    admission_ordinal,
                ) {
                    Ok(reservation) => reservation,
                    Err(v2_worker::CertifiedServeIngressReserveError::Busy) => {
                        return Err(FairV2IngressPushError::Full(inbound));
                    }
                    Err(v2_worker::CertifiedServeIngressReserveError::Rejected) => {
                        reject_after_leader_wire_admission!();
                    }
                    Err(v2_worker::CertifiedServeIngressReserveError::Closed) => {
                        state.open = false;
                        return Err(FairV2IngressPushError::Closed(inbound));
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        occurrence.lifecycle_ordinal = if let Some(token) = leader_wire_token.as_ref() {
            Some(token.scheduler_ordinal())
        } else if let Some(reservation) = certified_serve_reservation.as_ref() {
            Some(reservation.scheduler_ordinal())
        } else {
            match fair_v2_ingress_reserve_ordinary_lifecycle_ordinal(&state) {
                Ok(ordinal) => ordinal,
                Err(_) => {
                    state.open = false;
                    return Err(FairV2IngressPushError::FailStop(inbound));
                }
            }
        };
        if !occurrence.validate_exact() {
            state.open = false;
            return Err(FairV2IngressPushError::FailStop(inbound));
        }
        debug_assert!(state.last_admission_ordinal <= admission_ordinal);
        state.last_admission_ordinal = admission_ordinal;
        inbound.ingress_ownership = Some(FairV2IngressOwnershipEvidence::new(
            occurrence,
            leader_wire_token.clone(),
        ));
        let queue_was_empty = state.len == 0;
        if let Some(key) = &wire_key {
            assert!(
                state
                    .pending_wire_owners
                    .insert(key.clone(), source.clone())
                    .is_none(),
                "global coalescing key was checked absent while holding the ingress lock"
            );
        }
        let lane = state.lanes.entry(source.clone()).or_default();
        let was_empty = lane.entries.is_empty();
        if class == FairV2IngressClass::Progress {
            lane.progress_len += 1;
        }
        if uses_certified_fence_escape_reserve {
            lane.certified_fence_escape_len += 1;
            lane.certified_fence_escape_bytes = lane
                .certified_fence_escape_bytes
                .checked_add(encoded_len)
                .expect("certified fence-escape byte reserve prevents overflow");
        }
        if is_timeout_vote {
            lane.timeout_vote_len += 1;
            if is_validator_source {
                lane.timeout_vote_bytes = lane
                    .timeout_vote_bytes
                    .checked_add(encoded_len)
                    .expect("timeout-vote byte reserve prevents overflow");
            }
        }
        if is_transport_completion {
            lane.transport_completion_len += 1;
            lane.transport_completion_bytes = lane
                .transport_completion_bytes
                .checked_add(encoded_len)
                .expect("transport-completion byte reserve prevents overflow");
        }
        lane.bytes = lane
            .bytes
            .checked_add(encoded_len)
            .expect("configured per-source byte limit prevents overflow");
        if let Some(key) = &wire_key {
            assert!(
                lane.pending_wire.insert(key.clone()),
                "coalescing key was checked absent while holding the ingress lock"
            );
        }
        lane.entries.push_back(FairV2IngressEntry {
            inbound: Arc::new(inbound),
            enqueued_at,
            admission_ordinal,
            certified_serve_reservation,
            class,
            wire_key,
            leader_wire_token,
            encoded_bytes: encoded,
            encoded_len,
        });
        state.len += 1;
        state.bytes = state
            .bytes
            .checked_add(encoded_len)
            .expect("configured aggregate byte limit prevents overflow");
        if queue_was_empty {
            state.nonempty_since = Some(enqueued_at);
            state.last_service_attempt_at = None;
        }
        if was_empty {
            state.ready.push_back(source);
        }
        self.debug_assert_consistent(&state);
        Ok(FairV2IngressPushDisposition::Enqueued)
    }

    /// Pop one conditionally admitted message while rotating its source.
    ///
    /// Consumers are serialized, but the predicate executes without the
    /// ingress-state mutex. Producers can therefore continue bounded
    /// admission while certificate verification or downstream preparation is
    /// in progress. For every ready source, the method selects its oldest
    /// currently admissible entry.
    /// Earlier blocked entries remain in place, and the source still consumes
    /// only one round-robin turn. A later control for the same semantic
    /// source, context, height, and protocol class remains behind its immutable
    /// predecessor. After that predecessor crosses into the runtime queue, its
    /// frozen physical source/cut excludes later replays before logical rank is
    /// compared inside the retained predecessor set. Other proposal,
    /// certificate, body-response, or payload work may bypass an unrelated
    /// auxiliary request waiting for I/O capacity without dropping or duplicating
    /// that request. If an active-height certified-body request is queued,
    /// entries newer than its reservation-bearing carrier are excluded before
    /// the downstream predicate runs. Historical requests do not own that
    /// Serve gate and therefore cannot hide its exact target behind the cutoff;
    /// ungated test queues retain the all-request cutoff. Once a blocked entry
    /// becomes admissible, the head-first search selects it before later
    /// entries. When every entry is rejected, the source order and total length
    /// remain unchanged.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn try_recv_if(
        &self,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Option<InboundBlockMessage> {
        self.try_recv_if_checked(predicate).ok().flatten()
    }

    /// Checked production dequeue which preserves persistence-gate failures.
    pub(crate) fn try_recv_if_checked(
        &self,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<InboundBlockMessage>, String> {
        self.try_recv_if_at_checked(Instant::now(), predicate)
    }

    /// Test-only ordinary dequeue baseline which also releases a productive
    /// wire made permanently obsolete by the monotone safety-WAL recovery cut.
    ///
    /// An obsolete carrier still crosses the ordinary durable
    /// `Ingress -> Runtime` handoff. The caller must immediately publish its
    /// `Runtime -> VolatileTerminal` transition instead of sending the payload
    /// to the reducer. Temporary downstream backpressure remains queued.
    #[cfg(test)]
    pub(crate) fn try_recv_if_checked_retiring_obsolete(
        &self,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<(InboundBlockMessage, FairV2IngressDequeueDisposition)>, String> {
        self.try_recv_if_at_checked_classified(
            Instant::now(),
            true,
            FairV2IngressBarrierBypass::None,
            predicate,
        )
    }

    /// Checked production dequeue with one explicitly selected internal
    /// barrier-bypass policy.
    pub(crate) fn try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
        &self,
        barrier_bypass: FairV2IngressBarrierBypass,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<(InboundBlockMessage, FairV2IngressDequeueDisposition)>, String> {
        self.try_recv_if_at_checked_classified(Instant::now(), true, barrier_bypass, predicate)
    }

    #[cfg(test)]
    fn try_recv_if_at(
        &self,
        service_attempt_at: Instant,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Option<InboundBlockMessage> {
        self.try_recv_if_at_checked(service_attempt_at, predicate)
            .ok()
            .flatten()
    }

    fn try_recv_if_at_checked(
        &self,
        service_attempt_at: Instant,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<InboundBlockMessage>, String> {
        self.try_recv_if_at_checked_classified(
            service_attempt_at,
            false,
            FairV2IngressBarrierBypass::None,
            predicate,
        )
        .map(|selected| selected.map(|(inbound, _)| inbound))
    }

    fn try_recv_if_at_checked_classified(
        &self,
        service_attempt_at: Instant,
        retire_obsolete_leader_wire: bool,
        barrier_bypass: FairV2IngressBarrierBypass,
        mut predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<(InboundBlockMessage, FairV2IngressDequeueDisposition)>, String> {
        let _service_guard = self.service_lock.lock();
        let (ready_sources, candidates) = {
            let mut state = self.state.lock();
            if state.len != 0 {
                // A rejected scan is still proof that the outer runner
                // scheduler reached this queue. Downstream admission owns any
                // remaining delay; queue age alone does not establish
                // scheduler starvation.
                state.last_service_attempt_at = Some(service_attempt_at);
            }
            let selected_serve_barrier = match state.certified_serve_gate.as_ref() {
                Some(gate) => gate.selected_barrier()?,
                None if state.requires_certified_serve_gate => {
                    return Err("Serve selector crossed an unbound durable gate".to_owned());
                }
                None => None,
            };
            if let Some(barrier) = selected_serve_barrier {
                let matching_carriers = state
                    .lanes
                    .values()
                    .flat_map(|lane| lane.entries.iter())
                    .filter(|entry| {
                        entry.admission_ordinal == barrier.carrier_ordinal()
                            && entry
                                .certified_serve_reservation
                                .as_ref()
                                .is_some_and(|reservation| reservation.matches_barrier(barrier))
                            && matches!(
                                entry.inbound.message(),
                                BlockMessage::V2(ConsensusMessageV2 {
                                    payload:
                                        ConsensusMessageV2Payload::CertifiedBodyRequest(request),
                                    ..
                                }) if HashOf::new(request) == barrier.request_hash()
                            )
                    })
                    .count();
                if matching_carriers != 1 {
                    return Err(
                        "Serve selector changed its exact fair-ingress carrier identity".to_owned(),
                    );
                }
            }
            let certified_body_request_cutoff = selected_serve_barrier
                .is_none()
                .then(|| {
                    state
                        .lanes
                        .values()
                        .flat_map(|lane| lane.entries.iter())
                        .filter(|entry| {
                            fair_v2_ingress_is_certified_body_request(&entry.inbound)
                                && (!state.requires_certified_serve_gate
                                    || entry.certified_serve_reservation.is_some())
                        })
                        .map(|entry| entry.admission_ordinal)
                        .min()
                })
                .flatten();
            let active_leader_wire_owners = state
                .leader_wire_lifecycles
                .values()
                .filter(|record| record.status == FairV2IngressLeaderWireStatus::Ingress)
                .cloned()
                .collect::<Vec<_>>();
            let mut obsolete_leader_wire_tokens = BTreeSet::new();
            if state.requires_leader_wire_lifecycle_gate {
                let gate = state.leader_wire_lifecycle_gate.as_ref().ok_or_else(|| {
                    "leader-wire selector crossed an unbound durable gate".to_owned()
                })?;
                let durable_ordinals = gate.ingress_scheduler_ordinals()?;
                let active_ordinals = active_leader_wire_owners
                    .iter()
                    .map(|record| record.token.scheduler_ordinal)
                    .collect::<BTreeSet<_>>();
                if durable_ordinals != active_ordinals {
                    return Err(
                        "leader-wire selector changed its durable Ingress owner set".to_owned()
                    );
                }
                if retire_obsolete_leader_wire {
                    for record in &active_leader_wire_owners {
                        if gate.identity_is_obsolete(&record.token.identity)? {
                            obsolete_leader_wire_tokens.insert(record.token.clone());
                        }
                    }
                }
            }
            let mut leader_wire_carrier_ordinals = BTreeMap::new();
            for entry in state.lanes.values().flat_map(|lane| lane.entries.iter()) {
                let Some(token) = entry.leader_wire_token.as_ref() else {
                    continue;
                };
                if leader_wire_carrier_ordinals
                    .insert(token.clone(), entry.admission_ordinal)
                    .is_some()
                {
                    return Err(
                        "leader-wire selector duplicated its exact fair-ingress carrier".to_owned(),
                    );
                }
            }
            let mut active_leader_wire_carriers =
                Vec::with_capacity(active_leader_wire_owners.len());
            for owner in active_leader_wire_owners {
                let carrier_ordinal = leader_wire_carrier_ordinals
                    .remove(&owner.token)
                    .ok_or_else(|| {
                        "leader-wire selector lost its exact fair-ingress carrier".to_owned()
                    })?;
                active_leader_wire_carriers.push((owner, carrier_ordinal));
            }
            if !leader_wire_carrier_ordinals.is_empty() {
                return Err("leader-wire carrier has no matching active lifecycle owner".to_owned());
            }
            active_leader_wire_carriers.sort_by_key(|(_, ordinal)| *ordinal);
            if active_leader_wire_carriers
                .windows(2)
                .any(|pair| pair[0].1 == pair[1].1)
            {
                return Err("leader-wire selector reused a physical carrier ordinal".to_owned());
            }
            let (mut leader_wire_barrier, leader_wire_carrier_ordinal) =
                match active_leader_wire_carriers.into_iter().next() {
                    Some((owner, carrier_ordinal)) => (Some(owner), Some(carrier_ordinal)),
                    None => (None, None),
                };
            // Physical admission order arbitrates the two independently
            // durable reservation classes. A Dormant leader-wire retains its
            // lifecycle and runtime scheduler identity across restart, but
            // owns no queue position until an exact retry acquires a fresh
            // carrier. That later carrier cannot pass an already-admitted
            // selected Serve occurrence.
            if selected_serve_barrier.is_some_and(|serve| {
                leader_wire_carrier_ordinal
                    .is_some_and(|leader_ordinal| serve.carrier_ordinal() <= leader_ordinal)
            }) {
                leader_wire_barrier = None;
            }
            let selected_serve_predecessors_cleared = selected_serve_barrier.is_none_or(|serve| {
                state
                    .lanes
                    .values()
                    .flat_map(|lane| lane.entries.iter())
                    .all(|entry| entry.admission_ordinal >= serve.carrier_ordinal())
            });

            let leader_wire_body_dependency = leader_wire_barrier.as_ref().and_then(|owner| {
                state
                    .lanes
                    .values()
                    .flat_map(|lane| lane.entries.iter())
                    .find(|entry| entry.leader_wire_token.as_ref() == Some(&owner.token))
                    .and_then(|entry| {
                        let BlockMessage::V2(message) = entry.inbound.message() else {
                            return None;
                        };
                        match &message.payload {
                            ConsensusMessageV2Payload::Proposal(proposal) => {
                                Some((proposal.round, proposal.subject))
                            }
                            ConsensusMessageV2Payload::Vote(vote) => {
                                Some((vote.proposal_round, vote.subject))
                            }
                            ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                                Some((certificate.proposal_round, certificate.subject))
                            }
                            ConsensusMessageV2Payload::TimeoutVote(_)
                            | ConsensusMessageV2Payload::TimeoutCertificate(_)
                            | ConsensusMessageV2Payload::PayloadManifest(_)
                            | ConsensusMessageV2Payload::PayloadChunk(_)
                            | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                            | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                            | ConsensusMessageV2Payload::CommitCertificateRequest(_)
                            | ConsensusMessageV2Payload::CommitCertificateResponse(_)
                            | ConsensusMessageV2Payload::VrfCommit(_)
                            | ConsensusMessageV2Payload::VrfReveal(_) => None,
                        }
                    })
            });
            let leader_wire_control_barrier = leader_wire_barrier.as_ref().is_some_and(|owner| {
                owner.token.source_class == FairV2IngressLeaderWireSourceClass::Control
            });
            let ready_sources = state.ready.iter().cloned().collect::<Vec<_>>();
            let candidates = ready_sources
                .iter()
                .map(|source| {
                    state
                        .lanes
                        .get(source)
                        .into_iter()
                        .flat_map(|lane| {
                            lane.entries
                                .iter()
                                .enumerate()
                                .filter_map(|(index, entry)| {
                                    // A control occurrence may wait for
                                    // downstream capacity, but a later view or
                                    // conflicting carrier in the same semantic
                                    // slot cannot replace or bypass it.
                                    let has_live_control_predecessor =
                                        lane.entries.iter().take(index).any(|prior| {
                                            fair_v2_ingress_same_control_slot(
                                                &prior.inbound,
                                                &entry.inbound,
                                            )
                                        });
                                    let ingress_barrier_allows =
                                        if let Some(owner) = leader_wire_barrier.as_ref() {
                                            // A physically selected leader turn
                                            // exclusively drains its immutable
                                            // ingress-prefix episode.
                                            index
                                                < owner
                                                    .ingress_predecessors
                                                    .get(source)
                                                    .copied()
                                                    .unwrap_or(0)
                                                || (owner
                                                    .ingress_predecessors
                                                    .values()
                                                    .all(|count| *count == 0)
                                                    && entry.leader_wire_token.as_ref()
                                                        == Some(&owner.token))
                                        } else if let Some(serve) = selected_serve_barrier {
                                            // The exact Serve target and its
                                            // immutable earlier physical prefix
                                            // form one finite rank goal.
                                            entry.admission_ordinal < serve.carrier_ordinal()
                                                || (selected_serve_predecessors_cleared
                                                    && entry.admission_ordinal
                                                        == serve.carrier_ordinal()
                                                    && entry
                                                        .certified_serve_reservation
                                                        .as_ref()
                                                        .is_some_and(|reservation| {
                                                            reservation.matches_barrier(serve)
                                                        })
                                                    && matches!(
                                                        entry.inbound.message(),
                                                        BlockMessage::V2(ConsensusMessageV2 {
                                                            payload:
                                                                ConsensusMessageV2Payload::CertifiedBodyRequest(
                                                                    request
                                                                ),
                                                            ..
                                                        }) if HashOf::new(request)
                                                            == serve.request_hash()
                                                    ))
                                        } else {
                                            certified_body_request_cutoff.is_none_or(|cutoff| {
                                                entry.admission_ordinal <= cutoff
                                            })
                                        };
                                    let selected_serve_control_dependency =
                                        leader_wire_body_dependency.is_some_and(
                                            |(round, subject)| {
                                                selected_serve_barrier.is_some_and(|serve| {
                                                entry.admission_ordinal == serve.carrier_ordinal()
                                                    && entry
                                                        .certified_serve_reservation
                                                        .as_ref()
                                                        .is_some_and(|reservation| {
                                                            reservation.matches_barrier(serve)
                                                        })
                                                    && matches!(
                                                        entry.inbound.message(),
                                                        BlockMessage::V2(ConsensusMessageV2 {
                                                            payload:
                                                                ConsensusMessageV2Payload::CertifiedBodyRequest(
                                                                    request
                                                                ),
                                                            ..
                                                        }) if request.round == round
                                                            && request.subject == subject
                                                            && HashOf::new(request)
                                                                == serve.request_hash()
                                                    )
                                                })
                                            },
                                        );
                                    let earlier_dependency = selected_serve_barrier
                                        .is_none_or(|serve| {
                                            entry.admission_ordinal < serve.carrier_ordinal()
                                        })
                                        && (entry.class
                                            == FairV2IngressClass::TransportCompletion
                                            || leader_wire_body_dependency.is_some_and(
                                                |(round, subject)| {
                                                    leader_wire_barrier.as_ref().is_some_and(
                                                        |owner| {
                                                            entry.leader_wire_token.as_ref()
                                                                != Some(&owner.token)
                                                        },
                                                    ) && matches!(
                                                        entry.inbound.message(),
                                                        BlockMessage::V2(ConsensusMessageV2 {
                                                            payload:
                                                                ConsensusMessageV2Payload::Proposal(
                                                                    proposal
                                                                ),
                                                            ..
                                                        }) if proposal.round == round
                                                            && proposal.subject == subject
                                                    )
                                                },
                                            ));
                                    let timeout_control_dependency = leader_wire_barrier
                                        .as_ref()
                                        .is_some_and(|owner| {
                                            fair_v2_ingress_timeout_control_advances_owner(
                                                &owner.token,
                                                &entry.inbound,
                                            )
                                        });
                                    let authenticated_certified_fence_escape = !matches!(
                                        source,
                                        FairV2IngressSource::Anonymous
                                    ) && fair_v2_ingress_is_certified_fence_escape(&entry.inbound);
                                    let certified_fence_escape_dependency =
                                        authenticated_certified_fence_escape
                                            && leader_wire_barrier.as_ref().is_some_and(|owner| {
                                                fair_v2_ingress_certified_fence_escape_advances_owner(
                                                    &owner.token,
                                                    &entry.inbound,
                                                )
                                            });
                                    let serve_fence_escape_dependency =
                                        authenticated_certified_fence_escape
                                            && (selected_serve_barrier.is_some()
                                                || certified_body_request_cutoff.is_some());
                                    let timeout_vote_episode_dependency =
                                        barrier_bypass
                                            == FairV2IngressBarrierBypass::TimeoutVoteEpisode
                                            && fair_v2_ingress_is_direct_validator_timeout_vote_owner(
                                                source, entry,
                                            )
                                            && (leader_wire_barrier.as_ref().is_some_and(|owner| {
                                                owner.token.identity.phase
                                                    == FairV2IngressLeaderWirePhase::CertifiedResponse
                                            }) || (leader_wire_barrier.is_none()
                                                && (selected_serve_barrier.is_some()
                                                    || certified_body_request_cutoff.is_some())));
                                    let dependency_bypass = !ingress_barrier_allows
                                        && (serve_fence_escape_dependency
                                            || timeout_vote_episode_dependency
                                            || (leader_wire_control_barrier
                                                && (earlier_dependency
                                                    || selected_serve_control_dependency
                                                    || timeout_control_dependency
                                                    || certified_fence_escape_dependency)));
                                    (!has_live_control_predecessor
                                        && (ingress_barrier_allows || dependency_bypass))
                                        .then(|| {
                                            (
                                                entry.admission_ordinal,
                                                Arc::clone(&entry.inbound),
                                                dependency_bypass,
                                                entry.leader_wire_token.as_ref().is_some_and(
                                                    |token| {
                                                        obsolete_leader_wire_tokens.contains(token)
                                                    },
                                                ),
                                            )
                                        })
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            (ready_sources, candidates)
        };

        let mut selected = None;
        // Preserve the durable physical prefix whenever its selected owner is
        // currently admissible. Only after downstream admission rejects that
        // entire strict set may a dependency cross the control barrier.
        'sources: for (source_index, source_candidates) in candidates.iter().enumerate() {
            for (admission_ordinal, inbound, dependency_bypass, obsolete) in source_candidates {
                if !dependency_bypass && (*obsolete || predicate(inbound.as_ref())) {
                    let disposition = if *obsolete {
                        FairV2IngressDequeueDisposition::RetireObsolete
                    } else {
                        FairV2IngressDequeueDisposition::Admit
                    };
                    selected = Some((source_index, *admission_ordinal, disposition));
                    break 'sources;
                }
            }
        }
        if selected.is_none() {
            // Retained body-dependent control can depend on a matching
            // Proposal or the exact selected Serve request which produces its
            // missing body, and reducer control can depend on bounded body
            // completion. No dependency replaces the durable owner; it only
            // makes that owner admissible on a later turn.
            'bypass: for (source_index, source_candidates) in candidates.iter().enumerate() {
                for (admission_ordinal, inbound, dependency_bypass, obsolete) in source_candidates {
                    if *dependency_bypass && (*obsolete || predicate(inbound.as_ref())) {
                        let disposition = if *obsolete {
                            FairV2IngressDequeueDisposition::RetireObsolete
                        } else {
                            FairV2IngressDequeueDisposition::Admit
                        };
                        selected = Some((source_index, *admission_ordinal, disposition));
                        break 'bypass;
                    }
                }
            }
        }
        let Some((selected_source_index, admission_ordinal, mut disposition)) = selected else {
            return Ok(None);
        };
        drop(candidates);
        let source = ready_sources
            .get(selected_source_index)
            .cloned()
            .expect("selected fair-ingress source came from the ready snapshot");

        let mut state = self.state.lock();
        assert!(
            state
                .ready
                .iter()
                .take(ready_sources.len())
                .eq(ready_sources.iter()),
            "serialized fair-ingress service must preserve the snapshotted ready prefix"
        );
        let admitted_index = state
            .lanes
            .get(&source)
            .and_then(|lane| {
                lane.entries
                    .iter()
                    .position(|entry| entry.admission_ordinal == admission_ordinal)
            })
            .expect("serialized fair-ingress candidate must remain queued until selection");
        if retire_obsolete_leader_wire {
            let selected_token = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .and_then(|entry| entry.leader_wire_token.as_ref());
            let is_obsolete = match selected_token {
                Some(token) => state
                    .leader_wire_lifecycle_gate
                    .as_ref()
                    .ok_or_else(|| {
                        "obsolete leader-wire dequeue crossed an unbound durable gate".to_owned()
                    })?
                    .identity_is_obsolete(&token.identity)?,
                None => false,
            };
            if disposition == FairV2IngressDequeueDisposition::RetireObsolete && !is_obsolete {
                return Err(
                    "leader-wire recovery authority regressed during classified dequeue".to_owned(),
                );
            }
            if is_obsolete {
                // The recovery authority is monotone. It may advance while the
                // downstream predicate runs, so upgrade a normally admitted
                // selection rather than allowing newly obsolete control into
                // the reducer.
                disposition = FairV2IngressDequeueDisposition::RetireObsolete;
            }
        }
        let leader_wire_ownership = {
            let entry = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .expect("selected fair-ingress entry remains queued for runtime handoff");
            match entry.leader_wire_token.as_ref() {
                None => None,
                Some(token) => {
                    let ownership = entry
                        .inbound
                        .ingress_ownership
                        .as_ref()
                        .cloned()
                        .ok_or_else(|| {
                            "leader-wire dequeue lost its fair-ingress ownership carrier".to_owned()
                        })?;
                    if ownership.leader_wire_token() != Some(token)
                        || ownership.leader_wire_runtime_receipt().is_some()
                    {
                        return Err(
                            "leader-wire dequeue changed its exact ingress ownership".to_owned()
                        );
                    }
                    Some(ownership)
                }
            }
        };
        if let Some(mut ownership) = leader_wire_ownership {
            // Persist and install the deterministic runtime owner while the
            // physical carrier, durable Ingress record, and queue lock still
            // form one atomic handoff. Existing downstream bind calls then
            // validate this receipt idempotently.
            Self::bind_leader_wire_runtime_ownership_locked(&mut state, &mut ownership)?;
            let entry = state
                .lanes
                .get_mut(&source)
                .and_then(|lane| lane.entries.get_mut(admitted_index))
                .expect("selected leader-wire entry remains queued through durable handoff");
            Arc::make_mut(&mut entry.inbound).ingress_ownership = Some(ownership);
        }
        if let Some(reservation) = state
            .lanes
            .get(&source)
            .and_then(|lane| lane.entries.get(admitted_index))
            .and_then(|entry| entry.certified_serve_reservation.as_ref())
        {
            let evidence_lifecycle_ordinal = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .and_then(|entry| entry.inbound.ingress_ownership.as_ref())
                .and_then(FairV2IngressOwnershipEvidence::runtime_lifecycle_ordinal);
            if evidence_lifecycle_ordinal != Some(reservation.scheduler_ordinal()) {
                return Err(
                    "Serve carrier ownership disagreed with its reserved lifecycle ordinal"
                        .to_owned(),
                );
            }
            // Publish exact physical retirement while the carrier and every
            // capacity/index owner remain intact. Failure leaves the entry
            // retryable; success makes the following in-memory dequeue
            // bookkeeping crash-safe.
            reservation.publish_physical_drain()?;
        }
        for record in state
            .leader_wire_lifecycles
            .values_mut()
            .filter(|record| record.status == FairV2IngressLeaderWireStatus::Ingress)
        {
            if let Some(predecessors) = record.ingress_predecessors.get_mut(&source)
                && admitted_index < *predecessors
            {
                *predecessors = predecessors
                    .checked_sub(1)
                    .expect("selected predecessor count is non-zero");
            }
        }
        let (entry, remains_ready) = {
            let lane = state
                .lanes
                .get_mut(&source)
                .expect("ready fair-ingress source must own a configured lane");
            let entry = lane
                .entries
                .remove(admitted_index)
                .expect("selected fair-ingress entry must remain in its source lane");
            if entry.class == FairV2IngressClass::Progress {
                lane.progress_len = lane
                    .progress_len
                    .checked_sub(1)
                    .expect("Progress count includes every Progress entry");
            }
            if !matches!(source, FairV2IngressSource::Anonymous)
                && fair_v2_ingress_is_certified_fence_escape(&entry.inbound)
            {
                lane.certified_fence_escape_len = lane
                    .certified_fence_escape_len
                    .checked_sub(1)
                    .expect("certified fence-escape count includes every reserved owner");
                lane.certified_fence_escape_bytes = lane
                    .certified_fence_escape_bytes
                    .checked_sub(entry.encoded_len)
                    .expect("certified fence-escape bytes include every reserved owner");
            }
            if fair_v2_ingress_is_timeout_vote(&entry.inbound) {
                lane.timeout_vote_len = lane
                    .timeout_vote_len
                    .checked_sub(1)
                    .expect("TimeoutVote count includes every TimeoutVote entry");
                if matches!(source, FairV2IngressSource::Validator(_)) {
                    lane.timeout_vote_bytes = lane
                        .timeout_vote_bytes
                        .checked_sub(entry.encoded_len)
                        .expect("validator TimeoutVote bytes include every reserved owner");
                }
            }
            if entry.class == FairV2IngressClass::TransportCompletion {
                lane.transport_completion_len = lane
                    .transport_completion_len
                    .checked_sub(1)
                    .expect("transport-completion count includes every payload completion");
                lane.transport_completion_bytes = lane
                    .transport_completion_bytes
                    .checked_sub(entry.encoded_len)
                    .expect("transport-completion bytes include every payload completion");
            }
            lane.bytes = lane
                .bytes
                .checked_sub(entry.encoded_len)
                .expect("lane byte ownership includes every queued entry");
            if let Some(key) = &entry.wire_key {
                assert!(
                    lane.pending_wire.remove(key),
                    "queued wire key must remain indexed until service"
                );
            }
            let remains_ready = !lane.entries.is_empty();
            (entry, remains_ready)
        };
        if let Some(key) = &entry.wire_key {
            assert_eq!(
                state.pending_wire_owners.remove(key),
                Some(source.clone()),
                "global wire owner must remain indexed until service"
            );
        }
        state.len -= 1;
        state.bytes = state
            .bytes
            .checked_sub(entry.encoded_len)
            .expect("aggregate byte ownership includes every queued entry");
        if state.len == 0 {
            state.nonempty_since = None;
            state.last_service_attempt_at = None;
        }
        let runtime_physical_cut = u128::from(state.last_admission_ordinal) + 1;

        // Apply the same source rotation as the original lock-held scan. Any
        // producer that materialized a new source while the predicate ran is
        // ordered after the complete snapshotted service turn.
        let mut newly_ready = state.ready.split_off(ready_sources.len());
        state.ready.clear();
        state.ready.extend(
            ready_sources
                .iter()
                .skip(selected_source_index.saturating_add(1))
                .cloned(),
        );
        state
            .ready
            .extend(ready_sources.iter().take(selected_source_index).cloned());
        if remains_ready {
            state.ready.push_back(source.clone());
        } else if matches!(&source, FairV2IngressSource::Authenticated(_)) {
            let removed = state.lanes.remove(&source).expect(
                "an emptied authenticated non-validator lane remains indexed until dequeue",
            );
            debug_assert!(removed.entries.is_empty());
        }
        state.ready.append(&mut newly_ready);
        self.debug_assert_consistent(&state);
        let physical_admission_ordinal = entry.admission_ordinal;
        let mut inbound = Arc::try_unwrap(entry.inbound)
            .expect("serialized fair-ingress service must own the selected envelope");
        let Some(ownership) = inbound.ingress_ownership.as_mut() else {
            state.open = false;
            return Err(
                "selected fair-ingress envelope lost its physical ownership evidence".to_owned(),
            );
        };
        if ownership.physical_admission_ordinal() != Some(physical_admission_ordinal)
            || !ownership.freeze_runtime_physical_cut(runtime_physical_cut)
        {
            state.open = false;
            return Err(
                "selected fair-ingress envelope changed its immutable physical cut".to_owned(),
            );
        }
        Ok(Some((inbound, disposition)))
    }

    /// First receiver-local physical ordinal not yet allocated at this
    /// instant. The value is monotone and remains internal to local scheduling.
    pub(crate) fn next_physical_admission_ordinal(&self) -> u128 {
        let state = self.state.lock();
        u128::from(state.last_admission_ordinal) + 1
    }

    /// Snapshot live bounded ingress ownership at one local monotonic instant.
    pub(crate) fn snapshot_at(&self, now: Instant) -> FairV2IngressSnapshot {
        let state = self.state.lock();
        let oldest_enqueued_at = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter().map(|entry| entry.enqueued_at))
            .min();
        let service_baseline = state.last_service_attempt_at.or(state.nonempty_since);
        FairV2IngressSnapshot {
            depth: state.len,
            capacity: self.capacity,
            oldest_age: oldest_enqueued_at.map(|at| now.saturating_duration_since(at)),
            service_idle_age: service_baseline.map(|at| now.saturating_duration_since(at)),
        }
    }

    /// Pop one message while rotating its source behind every other ready source.
    #[cfg(test)]
    pub(crate) fn try_recv(&self) -> Option<InboundBlockMessage> {
        self.try_recv_if(|_| true)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.state.lock().len
    }
}

/// Admit one test envelope through the real fair-ingress ownership seam.
#[cfg(test)]
pub(crate) fn fair_v2_ingress_admit_for_test(inbound: InboundBlockMessage) -> InboundBlockMessage {
    let roster = inbound.via().cloned().into_iter().collect::<Vec<_>>();
    fair_v2_ingress_admit_with_roster_for_test(inbound, roster)
}

/// Admit one test envelope with an explicit frozen semantic validator roster.
#[cfg(test)]
pub(crate) fn fair_v2_ingress_admit_with_roster_for_test(
    inbound: InboundBlockMessage,
    roster: Vec<PeerId>,
) -> InboundBlockMessage {
    let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        64,
        128 * 1024 * 1024,
        64 * 1024 * 1024,
        CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        None,
    );
    ingress
        .configure_roster(roster)
        .expect("test fair-ingress geometry fits one exact envelope");
    ingress.open().expect("open test fair ingress");
    assert!(matches!(
        ingress.try_push(inbound),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    ingress
        .try_recv()
        .expect("test fair ingress returns its admitted owner")
}

/// Bounded ingress handle for the serialized Sumeragi v2 runner.
///
/// Global v1 frames are decode-only and are rejected before any queue handoff.
/// Fixed-small live auxiliary messages share the exact fair-ingress ownership
/// path but are terminalized before either consensus reducer.
/// All accepted queues are bounded and non-blocking. Reducer- and lane-owned
/// durable intents reconstruct their own retransmissions; live transport
/// callers additionally retain the exact item returned by [`SumeragiIngressDisposition::Retry`]
/// until this handle accepts, coalesces, obsoletes, or permanently rejects it.
#[derive(Clone)]
pub struct SumeragiHandle {
    block: Arc<FairV2Ingress>,
    lane_relay: mpsc::SyncSender<LaneRelayMessage>,
    wake: mpsc::SyncSender<()>,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
}

impl SumeragiHandle {
    fn new(
        block: Arc<FairV2Ingress>,
        lane_relay: mpsc::SyncSender<LaneRelayMessage>,
        wake: mpsc::SyncSender<()>,
        ingress_ready: Arc<AtomicBool>,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            block,
            lane_relay,
            wake,
            ingress_ready,
            output_guard,
        }
    }

    fn wake(&self) {
        let _ = self.wake.try_send(());
    }

    /// Wake the serialized v2 owner after a QueuePlan admission certificate
    /// has been durably published in Kura.
    ///
    /// The certificate itself is never transferred through an in-memory
    /// channel: the runner re-reads bounded, hash-addressed Kura evidence when
    /// constructing the next canonical carrier. A saturated wake channel is
    /// already an equivalent outstanding notification.
    #[must_use]
    pub fn notify_pending_queue_plan_admission(&self) -> bool {
        let Some(_permit) = self.output_guard.acquire() else {
            return false;
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            return false;
        }
        self.wake();
        true
    }

    /// Try to transfer one exact normalized block envelope to the serialized owner.
    pub fn try_incoming_block_message_owned(
        &self,
        inbound: InboundBlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(inbound);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            iroha_logger::debug!(
                "deferring Sumeragi ingress until context and safety WAL replay complete"
            );
            return SumeragiIngressDisposition::Retry(inbound);
        }

        if matches!(inbound.message(), BlockMessage::V2(_))
            || inbound.message().is_lane_local()
            || inbound.message().is_live_auxiliary()
        {
            let queue = status::WorkerQueueKind::Blocks;
            return match self.block.try_push(inbound) {
                Ok(FairV2IngressPushDisposition::Enqueued) => {
                    status::record_worker_queue_enqueue(queue);
                    self.wake();
                    SumeragiIngressDisposition::Accepted
                }
                Ok(FairV2IngressPushDisposition::Coalesced) => {
                    SumeragiIngressDisposition::Coalesced
                }
                Err(FairV2IngressPushError::Full(inbound)) => {
                    iroha_logger::debug!(
                        ?queue,
                        "bounded per-source Sumeragi ingress queue is full; retaining caller ownership"
                    );
                    SumeragiIngressDisposition::Retry(inbound)
                }
                Err(FairV2IngressPushError::Closed(inbound)) => {
                    iroha_logger::debug!(
                        ?queue,
                        "Sumeragi ingress queue closed during height rollover; retaining caller ownership"
                    );
                    SumeragiIngressDisposition::Retry(inbound)
                }
                Err(FairV2IngressPushError::FailStop(inbound)) => {
                    iroha_logger::error!(
                        ?queue,
                        "durable Sumeragi ingress lifecycle failed; requiring process restart"
                    );
                    self.output_guard
                        .activate_restart_required_from_permit(permit);
                    SumeragiIngressDisposition::FailStop(inbound)
                }
                Err(FairV2IngressPushError::Rejected(rejection)) => {
                    let message_kind =
                        FairV2IngressMessageKind::classify(rejection.inbound.message());
                    let round = fair_v2_ingress_consensus_round(rejection.inbound.message());
                    iroha_logger::warn!(
                        ?queue,
                        reason = ?rejection.reason,
                        ?message_kind,
                        ?round,
                        semantic_origin = ?rejection.inbound.sender(),
                        authenticated_via = ?rejection.inbound.via(),
                        "permanently rejected Sumeragi ingress envelope"
                    );
                    SumeragiIngressDisposition::Rejected(rejection.inbound)
                }
            };
        }

        iroha_logger::debug!("rejecting decode-only Sumeragi v1 frame on the v2 live ingress");
        SumeragiIngressDisposition::Obsolete
    }

    /// Try to enqueue a canonical message and preserve it on retryable pressure.
    pub fn try_incoming_block_message_from_owned(
        &self,
        sender: PeerId,
        message: BlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        self.try_incoming_block_message_owned(InboundBlockMessage::new(message, Some(sender)))
    }

    /// Enqueue a canonical v2, live auxiliary, or retained lane-local message without blocking.
    pub fn incoming_block_message(&self, message: BlockMessage) -> bool {
        self.try_incoming_block_message_owned(InboundBlockMessage::new(message, None))
            .accepted_or_coalesced()
    }

    /// Enqueue a canonical message from an authenticated transport peer.
    pub fn incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) {
        let _ = self.try_incoming_block_message_from_owned(sender, message);
    }

    /// Try to enqueue a canonical message without blocking.
    pub fn try_incoming_block_message(&self, message: BlockMessage) -> bool {
        self.try_incoming_block_message_owned(InboundBlockMessage::new(message, None))
            .accepted_or_coalesced()
    }

    /// Try to enqueue a canonical message from an authenticated transport peer.
    pub fn try_incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) -> bool {
        self.try_incoming_block_message_from_owned(sender, message)
            .accepted_or_coalesced()
    }

    /// Reject retired v1 control-flow frames.
    pub fn incoming_consensus_control_flow_message(&self, _message: ControlFlow) {
        iroha_logger::debug!("rejecting decode-only Sumeragi v1 control-flow frame");
    }

    /// Reject retired v1 control-flow frames.
    pub fn try_incoming_consensus_control_flow_message(&self, _message: ControlFlow) -> bool {
        false
    }

    /// Try to transfer one exact lane-relay item to its serialized owner.
    pub fn try_incoming_lane_relay_owned(
        &self,
        message: LaneRelayMessage,
    ) -> SumeragiIngressDisposition<LaneRelayMessage> {
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(message);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            return SumeragiIngressDisposition::Retry(message);
        }
        if let LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route,
            message: sidecar,
        } = &message
        {
            let allocating_requester = match sidecar {
                CertifiedMergeSidecarMessage::Request(request) => Some(&request.requester),
                CertifiedMergeSidecarMessage::Close(close) => Some(&close.requester),
                CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => None,
            };
            // The handle can authenticate only the semantic transport
            // identity and reply capability. A removed validator's exact
            // Kura/finality authority is verified by the serialized lane
            // adapter before it may allocate responder state; the sync
            // channel below remains the bounded handoff corridor.
            if allocating_requester.is_some_and(|requester| {
                requester != sender
                    || !reply_route
                        .as_ref()
                        .is_some_and(|route| route.is_active() && route.semantic_target() == sender)
            }) {
                iroha_logger::debug!(
                    %sender,
                    "rejecting unauthenticated certified merge-sidecar allocation before lane ingress"
                );
                return SumeragiIngressDisposition::Rejected(message);
            }
        }
        if let LaneRelayMessage::DrainVote { sender, vote } = &message
            && (sender != &vote.signer || vote.validate_ingress().is_err())
        {
            iroha_logger::debug!(
                %sender,
                signer = %vote.signer,
                "rejecting unauthenticated or invalid lane-drain vote before bounded ingress"
            );
            return SumeragiIngressDisposition::Rejected(message);
        }
        match self.lane_relay.try_send(message) {
            Ok(()) => {
                status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                self.wake();
                SumeragiIngressDisposition::Accepted
            }
            Err(mpsc::TrySendError::Full(message)) => {
                iroha_logger::debug!(
                    "bounded lane-local ingress queue is full; retaining caller ownership"
                );
                SumeragiIngressDisposition::Retry(message)
            }
            Err(mpsc::TrySendError::Disconnected(message)) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                iroha_logger::warn!("lane-local ingress queue is disconnected");
                self.output_guard
                    .activate_restart_required_from_permit(permit);
                SumeragiIngressDisposition::Closed(message)
            }
        }
    }

    /// Enqueue an inbound lane relay envelope.
    pub fn incoming_lane_relay(&self, envelope: LaneRelayEnvelope) {
        let _ = self.try_incoming_lane_relay(envelope);
    }

    /// Try to enqueue an inbound lane relay envelope.
    pub fn try_incoming_lane_relay(&self, envelope: LaneRelayEnvelope) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::Envelope(envelope))
            .accepted_or_coalesced()
    }

    /// Enqueue an inbound merge-committee signature.
    pub fn incoming_merge_signature(&self, signature: MergeCommitteeSignature) {
        let _ = self.try_incoming_merge_signature(signature);
    }

    /// Try to enqueue an inbound merge-committee signature.
    pub fn try_incoming_merge_signature(&self, signature: MergeCommitteeSignature) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::MergeSignature(signature))
            .accepted_or_coalesced()
    }

    /// Try to enqueue an authenticated lane-drain vote.
    pub fn try_incoming_lane_drain_vote(
        &self,
        sender: PeerId,
        vote: crate::lane_consensus::LaneDrainVoteV1,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::DrainVote { sender, vote })
            .accepted_or_coalesced()
    }

    /// Try to enqueue authenticated certified merge-sidecar traffic.
    pub fn try_incoming_certified_merge_sidecar(
        &self,
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route: None,
            message,
        })
        .accepted_or_coalesced()
    }

    /// Enqueue an authenticated Native AMX control message.
    pub fn incoming_native_amx(
        &self,
        sender: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) {
        let _ = self.try_incoming_native_amx(sender, message);
    }

    /// Try to enqueue an authenticated Native AMX control message.
    pub fn try_incoming_native_amx(
        &self,
        sender: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::NativeAmx {
            sender,
            reply_route: None,
            message,
        })
        .accepted_or_coalesced()
    }

    /// Return whether a fatal consensus failure requires process restart.
    #[must_use]
    pub fn restart_required(&self) -> bool {
        self.output_guard.restart_required()
    }
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn test_sumeragi_handle(
    block_capacity: usize,
) -> (
    SumeragiHandle,
    Arc<FairV2Ingress>,
    mpsc::Receiver<LaneRelayMessage>,
) {
    test_sumeragi_handle_with_source_geometry(block_capacity, None)
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn test_sumeragi_handle_with_source_geometry(
    block_capacity: usize,
    authenticated_non_validator_source_capacity: Option<usize>,
) -> (
    SumeragiHandle,
    Arc<FairV2Ingress>,
    mpsc::Receiver<LaneRelayMessage>,
) {
    const TEST_SOURCE_BYTE_CAPACITY: usize = 32 * 1024 * 1024;
    const TEST_AGGREGATE_BYTE_CAPACITY: usize = 1024 * 1024 * 1024;
    const TEST_TRANSPORT_COMPLETION_BYTE_RESERVE: usize =
        iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get()
            + BODY_ENVELOPE_HEADROOM_BYTES;
    let block = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            block_capacity,
            TEST_AGGREGATE_BYTE_CAPACITY,
            TEST_SOURCE_BYTE_CAPACITY,
            CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            TIMEOUT_VOTE_RESERVE_BYTES,
            TEST_TRANSPORT_COMPLETION_BYTE_RESERVE,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            authenticated_non_validator_source_capacity,
        ),
    );
    block
        .configure_roster(std::iter::empty())
        .expect("test anonymous lane fits configured capacity");
    block.open().expect("open configured test ingress");
    let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(block_capacity);
    let (wake_tx, _wake_rx) = mpsc::sync_channel(1);
    let handle = SumeragiHandle::new(
        Arc::clone(&block),
        lane_relay_tx,
        wake_tx,
        Arc::new(AtomicBool::new(true)),
        ConsensusOutputGuard::isolated(),
    );
    (handle, block, lane_relay_rx)
}

/// Feature-gated real ingress owner used by dependent-crate liveness tests.
///
/// The harness exposes only ordinary public ingress attempts and one exact
/// dequeue operation; production queue internals remain private.
#[cfg(feature = "iroha-core-tests")]
pub struct SumeragiIngressTestHarness {
    handle: SumeragiHandle,
    block: Arc<FairV2Ingress>,
    _lane_relay: mpsc::Receiver<LaneRelayMessage>,
}

#[cfg(feature = "iroha-core-tests")]
impl SumeragiIngressTestHarness {
    /// Construct an open bounded ingress with an empty validator roster.
    #[must_use]
    pub fn new(block_capacity: usize) -> Self {
        let (handle, block, lane_relay) = test_sumeragi_handle(block_capacity);
        Self {
            handle,
            block,
            _lane_relay: lane_relay,
        }
    }

    /// Clone the genuine production ingress handle.
    #[must_use]
    pub fn handle(&self) -> SumeragiHandle {
        self.handle.clone()
    }

    /// Remove one exact block occurrence and release its bounded inner owner.
    #[must_use]
    pub fn pop_block(&self) -> Option<InboundBlockMessage> {
        self.block.try_recv_if(|_| true)
    }
}

/// Spawn configuration for the authoritative serialized Sumeragi v2 worker.
pub struct SumeragiStartArgs {
    /// Frozen-compatible v2 consensus configuration.
    pub config: SumeragiConfig,
    /// Common configuration shared with other subsystems (keys, peers, chain id).
    pub common_config: CommonConfig,
    /// Channel used to emit consensus lifecycle events to observers.
    pub events_sender: EventsSender,
    /// Handle to the world state view.
    pub state: Arc<State>,
    /// Transaction queue shared with the pipeline.
    pub queue: Arc<Queue>,
    /// Persistent block store interface.
    pub kura: Arc<Kura>,
    /// Optional exact finalized provider-ingest archive captured at every v2
    /// WSV commit boundary.
    pub provider_ingest_finalized_archive:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    /// Optional exact finalized reputation archive captured at every v2 WSV
    /// commit boundary.
    pub reputation_finalized_archive:
        Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
    /// Exact startup replay boundary authenticated before Kura replay and
    /// moved into active-height recovery without a historical rescan.
    pub startup_replay_plan: V2StartupReplayPlan,
    /// Ownership guard for the startup-only O(H) finality inventory.
    ///
    /// The caller must acquire this immediately after replay planning so every
    /// later startup error clears the inventory, then move it here for the
    /// recovery handoff.
    pub startup_replay_inventory_guard: V2StartupReplayInventoryGuard,
    /// Network transport handle for broadcasting consensus messages.
    pub network: IrohaNetwork,
    /// Maximum encrypted P2P frame bytes accepted by the transport.
    pub max_frame_bytes: usize,
    /// Maximum plaintext bytes accepted on the consensus-recovery topic.
    pub max_frame_bytes_consensus: usize,
    /// Maximum plaintext bytes accepted on the consensus-safety/control topic.
    pub max_frame_bytes_control: usize,
    /// Maximum plaintext bytes accepted on payload/chunk/block-sync topics.
    pub max_frame_bytes_block_sync: usize,
    /// Maximum encrypted high-priority outbound frame bytes retained per peer.
    pub outbound_frame_queue_max_high_bytes: usize,
    /// Genesis network data augmented with leader public keys.
    pub genesis_network: GenesisWithPubKey,
}

fn spawn_sumeragi_thread(
    builder: std::thread::Builder,
    work: SumeragiThreadWork,
) -> std::io::Result<SumeragiThreadCompletion> {
    Ok(Box::pin(try_spawn_os_thread_as_future(builder, work)?))
}

fn launch_sumeragi_thread(
    output_guard: &ConsensusOutputGuard,
    work: SumeragiThreadWork,
    publish_queue_wake: impl FnOnce(),
    spawn: SumeragiThreadSpawner,
) -> Result<Child> {
    let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
        eyre::eyre!("Sumeragi consensus requires restart before another worker can start")
    })?;
    if !output_guard.claim_authoritative_worker_launch() {
        operation.complete();
        return Err(eyre::eyre!(
            "an authoritative Sumeragi worker already launched in this process; a full process restart is required"
        ));
    }
    let (start_tx, start_rx) = mpsc::sync_channel(0);
    let gated_work: SumeragiThreadWork = Box::new(move || {
        if start_rx.recv().is_ok() {
            work();
        }
    });
    let completion = spawn(sumeragi_thread_builder("sumeragi"), gated_work)
        .map_err(|error| eyre::eyre!("failed to spawn authoritative Sumeragi worker: {error}"))?;
    let join_handle = tokio::task::spawn(completion);
    let child = Child::new(join_handle, OnShutdown::Wait(Duration::from_secs(5)));

    // Queue wake publication uses an irreversible OnceLock. Publish only after
    // the OS thread and its async monitor both exist; the start gate keeps the
    // worker from observing a partially published launch.
    publish_queue_wake();
    start_tx
        .send(())
        .expect("freshly spawned Sumeragi worker must be waiting on its start gate");
    operation.complete();
    Ok(child)
}

impl SumeragiStartArgs {
    /// Launch the serialized v2 reducer worker and its bounded ingress handle.
    ///
    /// # Errors
    ///
    /// Returns an error when the authoritative v2 worker cannot be spawned.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> Result<(SumeragiHandle, Child)> {
        let SumeragiStartArgs {
            config,
            common_config,
            events_sender,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            startup_replay_plan,
            startup_replay_inventory_guard,
            network,
            max_frame_bytes,
            max_frame_bytes_consensus,
            max_frame_bytes_control,
            max_frame_bytes_block_sync,
            outbound_frame_queue_max_high_bytes,
            genesis_network,
        } = self;
        #[cfg(not(test))]
        let output_guard = process_consensus_output_guard();
        #[cfg(test)]
        let output_guard = ConsensusOutputGuard::isolated();
        if output_guard.restart_required() {
            return Err(eyre::eyre!(
                "Sumeragi consensus is restart-required after a fatal live-runner failure"
            ));
        }
        kura.bind_consensus_output_guard(Arc::clone(&output_guard))
            .map_err(|error| {
                eyre::eyre!("failed to bind Kura fail-stop admission guard: {error}")
            })?;
        if output_guard.restart_required() {
            return Err(eyre::eyre!(
                "Sumeragi consensus is restart-required after Kura canonical storage poison"
            ));
        }

        let block_channel_cap = config.queues.bodies.get();
        let block_byte_cap = config.queues.body_bytes.get();
        let block_source_byte_cap = config.queues.body_source_bytes.get();
        let ordinary_wire_byte_reserve = config
            .block
            .max_payload_bytes
            .get()
            .checked_add(BODY_ENVELOPE_HEADROOM_BYTES)
            .map(|bytes| bytes.max(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES))
            .ok_or_else(|| {
                eyre::eyre!(
                    "Sumeragi ordinary canonical wire-byte reserve exceeds platform capacity"
                )
            })?;
        let transport_completion_byte_reserve = block_source_byte_cap
            .checked_sub(CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES)
            .and_then(|bytes| bytes.checked_sub(TIMEOUT_VOTE_RESERVE_BYTES))
            .and_then(|bytes| bytes.checked_sub(ordinary_wire_byte_reserve))
            .ok_or_else(|| {
                eyre::eyre!(
                    "Sumeragi per-source ingress bytes do not contain disjoint ordinary, timeout, and payload-completion partitions"
                )
            })?;
        let global_plaintext_frame_capacity = iroha_p2p::frame_plaintext_cap(max_frame_bytes);
        let consensus_frame_byte_capacity =
            global_plaintext_frame_capacity.min(max_frame_bytes_consensus);
        let control_frame_byte_capacity =
            global_plaintext_frame_capacity.min(max_frame_bytes_control);
        let block_sync_frame_byte_capacity =
            global_plaintext_frame_capacity.min(max_frame_bytes_block_sync);
        let lane_relay_channel_cap = config.queues.ready_bodies.get();
        let authenticated_non_validator_source_capacity =
            config.queues.authenticated_non_validator_sources.get();
        let block = Arc::new(
            FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                block_channel_cap,
                block_byte_cap,
                block_source_byte_cap,
                CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
                TIMEOUT_VOTE_RESERVE_BYTES,
                transport_completion_byte_reserve,
                consensus_frame_byte_capacity,
                control_frame_byte_capacity,
                block_sync_frame_byte_capacity,
                outbound_frame_queue_max_high_bytes,
                Some(authenticated_non_validator_source_capacity),
            ),
        );
        block.require_certified_serve_gate();
        block.require_leader_wire_lifecycle_gate();
        let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(lane_relay_channel_cap);
        let (wake_tx, wake_rx) = mpsc::sync_channel(WORKER_WAKE_CHANNEL_CAP);
        let queue_wake = Arc::clone(&queue);
        let queue_wake_tx = wake_tx.clone();
        let ingress_ready = Arc::new(AtomicBool::new(false));

        let handle = SumeragiHandle::new(
            Arc::clone(&block),
            lane_relay_tx,
            wake_tx,
            Arc::clone(&ingress_ready),
            Arc::clone(&output_guard),
        );

        let worker = SumeragiWorker {
            config,
            common_config,
            events_sender,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            startup_replay_plan,
            startup_replay_inventory_guard,
            network,
            genesis_network,
            lane_relay_rx,
            ingress_ready,
            output_guard: Arc::clone(&output_guard),
            block_rx: block,
            wake_rx,
            shutdown_signal,
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
        };

        let child = launch_sumeragi_thread(
            output_guard.as_ref(),
            Box::new(move || worker.run()),
            move || queue_wake.set_sumeragi_wake(queue_wake_tx),
            spawn_sumeragi_thread,
        )?;

        Ok((handle, child))
    }
}

#[cfg(test)]
mod worker_launch_tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use super::*;

    fn fail_spawn(
        _builder: std::thread::Builder,
        _work: SumeragiThreadWork,
    ) -> std::io::Result<SumeragiThreadCompletion> {
        Err(std::io::Error::other("injected synchronous spawn failure"))
    }

    static RESTART_ISOLATION_SPAWN_CALLED: AtomicBool = AtomicBool::new(false);
    static PROCESS_LIFETIME_SPAWN_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);
    static PROCESS_LIFETIME_COMPLETION_OBSERVATIONS: AtomicUsize = AtomicUsize::new(0);

    fn record_restart_isolation_spawn(
        _builder: std::thread::Builder,
        _work: SumeragiThreadWork,
    ) -> std::io::Result<SumeragiThreadCompletion> {
        RESTART_ISOLATION_SPAWN_CALLED.store(true, Ordering::Release);
        Err(std::io::Error::other(
            "restart isolation must reject before spawning",
        ))
    }

    fn count_successful_spawn(
        builder: std::thread::Builder,
        work: SumeragiThreadWork,
    ) -> std::io::Result<SumeragiThreadCompletion> {
        PROCESS_LIFETIME_SPAWN_INVOCATIONS.fetch_add(1, Ordering::AcqRel);
        let completion = spawn_sumeragi_thread(builder, work)?;
        Ok(Box::pin(async move {
            completion.await;
            PROCESS_LIFETIME_COMPLETION_OBSERVATIONS.fetch_add(1, Ordering::AcqRel);
        }))
    }

    #[test]
    fn synchronous_spawn_failure_precedes_queue_wake_publication() {
        let output_guard = ConsensusOutputGuard::isolated();
        let wake_published = Arc::new(AtomicBool::new(false));
        let published = Arc::clone(&wake_published);

        let result = launch_sumeragi_thread(
            output_guard.as_ref(),
            Box::new(|| panic!("failed spawn must never run worker")),
            move || published.store(true, Ordering::Release),
            fail_spawn,
        );

        assert!(result.is_err());
        assert!(!wake_published.load(Ordering::Acquire));
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn restart_required_relaunch_is_rejected_before_any_publication() {
        let output_guard = ConsensusOutputGuard::isolated();
        output_guard.activate_restart_required();
        RESTART_ISOLATION_SPAWN_CALLED.store(false, Ordering::Release);

        let work_called = Arc::new(AtomicBool::new(false));
        let work_observer = Arc::clone(&work_called);
        let wake_published = Arc::new(AtomicBool::new(false));
        let wake_observer = Arc::clone(&wake_published);

        let error = launch_sumeragi_thread(
            output_guard.as_ref(),
            Box::new(move || work_observer.store(true, Ordering::Release)),
            move || wake_observer.store(true, Ordering::Release),
            record_restart_isolation_spawn,
        )
        .expect_err("a poisoned process must not host a fresh generation-zero worker");

        assert!(
            error
                .to_string()
                .contains("requires restart before another worker can start")
        );
        assert!(!RESTART_ISOLATION_SPAWN_CALLED.load(Ordering::Acquire));
        assert!(!work_called.load(Ordering::Acquire));
        assert!(!wake_published.load(Ordering::Acquire));
        assert!(output_guard.restart_required());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn process_lifetime_worker_claim_rejects_relaunch_after_worker_exit() {
        let output_guard = ConsensusOutputGuard::isolated();
        PROCESS_LIFETIME_SPAWN_INVOCATIONS.store(0, Ordering::Release);
        PROCESS_LIFETIME_COMPLETION_OBSERVATIONS.store(0, Ordering::Release);
        let work_invocations = Arc::new(AtomicUsize::new(0));
        let wake_publications = Arc::new(AtomicUsize::new(0));

        let first_work_invocations = Arc::clone(&work_invocations);
        let first_wake_publications = Arc::clone(&wake_publications);
        let first_child = launch_sumeragi_thread(
            output_guard.as_ref(),
            Box::new(move || {
                first_work_invocations.fetch_add(1, Ordering::AcqRel);
            }),
            move || {
                first_wake_publications.fetch_add(1, Ordering::AcqRel);
            },
            count_successful_spawn,
        )
        .expect("the first process-lifetime worker launch succeeds");
        tokio::time::timeout(Duration::from_secs(5), async {
            while PROCESS_LIFETIME_COMPLETION_OBSERVATIONS.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the first OS worker completion is observed before relaunch");
        drop(first_child);

        let second_work_invocations = Arc::clone(&work_invocations);
        let second_wake_publications = Arc::clone(&wake_publications);
        let error = launch_sumeragi_thread(
            output_guard.as_ref(),
            Box::new(move || {
                second_work_invocations.fetch_add(1, Ordering::AcqRel);
            }),
            move || {
                second_wake_publications.fetch_add(1, Ordering::AcqRel);
            },
            count_successful_spawn,
        )
        .expect_err("worker generation zero cannot be reused inside one process");

        assert!(
            error
                .to_string()
                .contains("an authoritative Sumeragi worker already launched in this process")
        );
        assert_eq!(
            PROCESS_LIFETIME_SPAWN_INVOCATIONS.load(Ordering::Acquire),
            1
        );
        assert_eq!(
            PROCESS_LIFETIME_COMPLETION_OBSERVATIONS.load(Ordering::Acquire),
            1
        );
        assert_eq!(work_invocations.load(Ordering::Acquire), 1);
        assert_eq!(wake_publications.load(Ordering::Acquire), 1);
        assert!(!output_guard.restart_required());
    }

    #[test]
    fn startup_inventory_cleanup_guard_clears_on_launch_path_drop() {
        let kura = Kura::blank_kura_for_testing();
        let _plan =
            plan_v2_startup_replay(kura.as_ref()).expect("plan and install startup inventory");
        assert!(
            kura.begin_v2_startup_finality_verification()
                .expect("inspect installed startup binding")
                .is_some()
        );

        // Model any fallible irohad startup step between replay planning and
        // the Sumeragi recovery handoff.
        drop(V2StartupReplayInventoryGuard::new(Arc::clone(&kura)));

        assert!(
            kura.begin_v2_startup_finality_verification()
                .expect("inspect cleared startup binding")
                .is_none(),
            "every launch-path guard drop must release startup-only metadata"
        );
    }
}

/// RAII owner for Kura's startup-only O(H) finality inventory.
///
/// Construct this immediately after [`plan_v2_startup_replay`] succeeds and
/// retain it across all subsequent fallible startup work. Dropping the guard
/// releases the inventory; Sumeragi moves it into active-height recovery and
/// explicitly finishes it once the handoff completes.
#[must_use = "dropping the guard immediately clears the startup replay inventory"]
pub struct V2StartupReplayInventoryGuard {
    kura: Option<Arc<Kura>>,
}

impl V2StartupReplayInventoryGuard {
    /// Own cleanup of the startup replay inventory installed in `kura`.
    #[must_use]
    pub fn new(kura: Arc<Kura>) -> Self {
        Self { kura: Some(kura) }
    }

    fn finish(&mut self) {
        if let Some(kura) = self.kura.take() {
            kura.finish_v2_startup_finality_verification();
        }
    }
}

impl Drop for V2StartupReplayInventoryGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

struct SumeragiWorker {
    config: SumeragiConfig,
    common_config: CommonConfig,
    events_sender: EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    provider_ingest_finalized_archive:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    reputation_finalized_archive:
        Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
    startup_replay_plan: V2StartupReplayPlan,
    startup_replay_inventory_guard: V2StartupReplayInventoryGuard,
    network: IrohaNetwork,
    genesis_network: GenesisWithPubKey,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
    block_rx: Arc<FairV2Ingress>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
    consensus_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
}

#[cfg(test)]
mod authoritative_runtime_gate_tests {
    include!("tests/mod_authoritative_runtime_gate_01_support.rs");
    include!("tests/mod_authoritative_runtime_gate_02_carrierless_replay.rs");
    #[test]
    fn timeout_vote_episode_crosses_only_the_bounded_certified_response_barrier() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
        let validator = PeerId::new(KeyPair::random().public_key().clone());
        let response = v2_certified_body_response(0, 0, 1);
        let round = match &response {
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                ..
            }) => response.manifest.round,
            _ => unreachable!("certified response fixture is a v2 envelope"),
        };
        let mut timeout = v2_timeout_vote();
        match &mut timeout {
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                ..
            }) => vote.round = round,
            _ => unreachable!("timeout fixture is a v2 envelope"),
        }
        let _gate_directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(response, Some(validator.clone()),)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(timeout, Some(validator))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        {
            let state = ingress.state.lock();
            let earliest = state
                .leader_wire_lifecycles
                .values()
                .filter(|record| record.status == super::FairV2IngressLeaderWireStatus::Ingress)
                .min_by_key(|record| record.token.scheduler_ordinal)
                .expect("one leader-wire barrier is active");
            assert_eq!(
                earliest.token.identity.phase,
                super::FairV2IngressLeaderWirePhase::CertifiedResponse
            );
        }

        let is_timeout_vote = |inbound: &InboundBlockMessage| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                    ..
                })
            )
        };
        assert!(
            ingress
                .try_recv_if_checked_retiring_obsolete(is_timeout_vote)
                .expect("ordinary selection preserves the response barrier")
                .is_none()
        );
        assert!(
            ingress
                .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                    super::FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                    |_| false,
                )
                .expect("the internal episode policy still needs its runtime predicate")
                .is_none()
        );
        let (mut selected, disposition) = ingress
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                super::FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                is_timeout_vote,
            )
            .expect("the response barrier preserves the checked dequeue")
            .expect("the exact timeout vote reaches its episode predicate");
        assert_eq!(disposition, super::FairV2IngressDequeueDisposition::Admit);
        assert!(is_timeout_vote(&selected));
        let ownership = selected
            .take_ingress_ownership()
            .expect("the selected timeout vote retains exact ingress ownership");
        assert!(ownership.validate_exact());
        assert!(ownership.leader_wire_runtime_receipt().is_some());
        assert_eq!(ingress.len(), 1, "the certified response stays retained");
        assert!(
            ingress
                .state
                .lock()
                .leader_wire_lifecycles
                .values()
                .any(|record| {
                    record.status == super::FairV2IngressLeaderWireStatus::Ingress
                        && record.token.identity.phase
                            == super::FairV2IngressLeaderWirePhase::CertifiedResponse
                })
        );
    }

    #[test]
    fn restored_productive_retry_stays_behind_an_earlier_certified_request_carrier() {
        let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
        assert_eq!(fixture.token.admission_ordinal(), 7);
        assert!(matches!(
            fixture
                .ingress
                .try_push(v2_certified_body_request_inbound(&fixture.validator)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let target_ordinal = fixture
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| fair_v2_ingress_is_certified_body_request(&entry.inbound))
            .expect("certified request owns its fresh physical occurrence")
            .admission_ordinal;
        assert!(target_ordinal > fixture.token.admission_ordinal());

        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                fixture.message.clone(),
                Some(fixture.validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let retry_ordinal = fixture
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
            .expect("restored productive lifecycle regained one physical carrier")
            .admission_ordinal;
        assert!(
            retry_ordinal > target_ordinal,
            "a retained lifecycle token cannot reuse its old ordinal as a new physical position"
        );
        assert!(
            fixture
                .ingress
                .try_recv_if(|inbound| !fair_v2_ingress_is_certified_body_request(inbound))
                .is_none(),
            "the later productive retry cannot cross the certified target cutoff"
        );

        let target = fixture
            .ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the refrozen leader prefix admits the earlier certified carrier");
        assert_eq!(target.sender(), Some(&fixture.validator));
        let retry = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("the productive retry drains after its frozen predecessor");
        assert!(
            retry
                .ingress_ownership()
                .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
        );
    }

    #[test]
    fn restored_productive_retry_freezes_the_current_physical_source_prefix() {
        let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
        let earlier = v2_commit_certificate_request(0, &fixture.validator);
        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                earlier,
                Some(fixture.validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let earlier_ordinal = fixture
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.is_none())
            .expect("ordinary traffic owns its physical occurrence")
            .admission_ordinal;

        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                fixture.message.clone(),
                Some(fixture.validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let retry_ordinal = fixture
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
            .expect("restored lifecycle acquired one fresh carrier")
            .admission_ordinal;
        assert!(earlier_ordinal < retry_ordinal);

        assert!(
            fixture
                .ingress
                .try_recv_if(|inbound| {
                    inbound
                        .ingress_ownership()
                        .is_some_and(|ownership| ownership.leader_wire_token().is_some())
                })
                .is_none(),
            "a predicate which rejects the predecessor cannot select the leader-wire target"
        );
        let first = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("the replay-frozen physical predecessor drains first");
        assert!(matches!(
            first.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(_),
                ..
            })
        ));
        let replay = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("the exact replay drains after its frozen source prefix");
        assert!(
            replay
                .ingress_ownership()
                .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
        );
    }

    #[test]
    fn restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire() {
        let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
        let round = match &fixture.message {
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
                ..
            }) => proposal.round,
            _ => unreachable!("restart fixture carries a proposal"),
        };
        let mut earlier_message = v2_vote(wire::GlobalPhase::Prepare);
        let BlockMessage::V2(earlier_envelope) = &mut earlier_message else {
            unreachable!("vote fixture is a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Vote(earlier_vote) = &mut earlier_envelope.payload
        else {
            unreachable!("vote fixture carries a vote");
        };
        earlier_vote.round = round;
        earlier_vote.proposal_round = round;

        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                earlier_message,
                Some(fixture.alternate_validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let (earlier_token, earlier_physical_ordinal) = {
            let state = fixture.ingress.state.lock();
            let entry = state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .find(|entry| {
                    entry
                        .leader_wire_token
                        .as_ref()
                        .is_some_and(|token| token != &fixture.token)
                })
                .expect("fresh vote owns one leader-wire carrier");
            (
                entry
                    .leader_wire_token
                    .clone()
                    .expect("selected entry has a leader-wire token"),
                entry.admission_ordinal,
            )
        };
        assert!(
            earlier_token.scheduler_ordinal > fixture.token.scheduler_ordinal,
            "the fresh lifecycle has a newer logical identity"
        );

        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                fixture.message.clone(),
                Some(fixture.validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let replay_physical_ordinal = fixture
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
            .expect("restored lifecycle owns one replay carrier")
            .admission_ordinal;
        assert!(
            earlier_physical_ordinal < replay_physical_ordinal,
            "the replay carrier is physically newer despite its older logical identity"
        );
        assert_eq!(
            fixture
                .gate
                .ingress_scheduler_ordinals()
                .expect("read exact durable Ingress owner set"),
            std::collections::BTreeSet::from([
                fixture.token.scheduler_ordinal,
                earlier_token.scheduler_ordinal,
            ])
        );
        {
            // Round-robin history is independent of physical admission order.
            // Put the replay source first to ensure only the physical barrier,
            // rather than incidental ready-source order, protects the earlier
            // carrier.
            let mut state = fixture.ingress.state.lock();
            state.ready = std::collections::VecDeque::from([
                super::FairV2IngressSource::Validator(fixture.validator.clone()),
                super::FairV2IngressSource::Validator(fixture.alternate_validator.clone()),
            ]);
            fixture.ingress.debug_assert_consistent(&state);
        }
        assert!(
            fixture
                .ingress
                .try_recv_if(|inbound| {
                    inbound.ingress_ownership().is_some_and(|ownership| {
                        ownership.leader_wire_token() == Some(&fixture.token)
                    })
                })
                .is_none(),
            "the physically later replay cannot be selected merely because its logical ordinal is older"
        );

        let mut first = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("the physically earlier leader-wire carrier drains first");
        let mut first_ownership = first
            .take_ingress_ownership()
            .expect("leader-wire carrier retains ingress ownership");
        assert_eq!(
            first_ownership.leader_wire_token(),
            Some(&earlier_token),
            "physical order, not retained logical order, selects the owner"
        );
        fixture
            .ingress
            .bind_leader_wire_runtime_ownership(&mut first_ownership)
            .expect("bind the selected fresh lifecycle");

        let second = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("the older logical replay drains on the next turn");
        assert!(
            second
                .ingress_ownership()
                .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
        );
    }

    #[test]
    fn restored_productive_retry_ordinal_exhaustion_keeps_the_owner_dormant() {
        let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
        fixture.ingress.state.lock().last_admission_ordinal = u64::MAX;

        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                fixture.message.clone(),
                Some(fixture.validator.clone()),
            )),
            Err(super::FairV2IngressPushError::FailStop(_))
        ));
        {
            let state = fixture.ingress.state.lock();
            assert!(
                !state.open,
                "physical ordinal exhaustion fails admission closed"
            );
            assert_eq!(state.len, 0, "no carrier was admitted");
            let record = state
                .leader_wire_lifecycles
                .get(&fixture.token.slot)
                .expect("restored lifecycle remains retained");
            assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Dormant);
            assert_eq!(record.token, fixture.token);
        }
        assert_eq!(
            fixture
                .gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read dormant durable selector"),
            None,
            "ordinal exhaustion cannot publish a carrierless scheduler owner"
        );
    }

    include!("tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs");
    #[test]
    fn authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains() {
        const SOURCE_BYTES: usize = 1024 * 1024;
        let ingress = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            13,
            4 * SOURCE_BYTES,
            SOURCE_BYTES,
            0,
            0,
            0,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            Some(2),
        );
        let mut peers = validator_peers(5);
        let source_c = peers.pop().expect("source C");
        let source_b = peers.pop().expect("source B");
        let source_a = peers.pop().expect("source A");
        let origin = peers.pop().expect("semantic origin");
        let validator = peers.pop().expect("validator");
        ingress
            .configure_roster([validator])
            .expect("one validator, two authenticated relays, and anonymous fit exactly");
        ingress.open().expect("open exact source geometry");
        {
            let state = ingress.state.lock();
            assert_eq!(
                state.len
                    + super::fair_v2_ingress_current_protected_slots(
                        &state,
                        ingress.authenticated_non_validator_source_capacity,
                    ),
                13,
                "unmaterialized authenticated-source lanes retain their exact reservation"
            );
        }

        let inbound = |index, via: PeerId| {
            InboundBlockMessage::from_transport(v2_auxiliary_prepare(index), origin.clone(), via)
        };
        assert!(matches!(
            ingress.try_push(inbound(1, source_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(inbound(2, source_b.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        {
            let state = ingress.state.lock();
            assert_eq!(
                state.len
                    + super::fair_v2_ingress_current_protected_slots(
                        &state,
                        ingress.authenticated_non_validator_source_capacity,
                    ),
                13,
                "materializing both lanes consumes, but does not erase, their reservations"
            );
        }
        assert!(matches!(
            ingress.try_push(inbound(3, source_c.clone())),
            Err(super::FairV2IngressPushError::Full(_))
        ));

        let first = ingress
            .try_recv()
            .expect("source A owns the first fair turn");
        assert_eq!(first.via(), Some(&source_a));
        {
            let state = ingress.state.lock();
            assert_eq!(
                state.len
                    + super::fair_v2_ingress_current_protected_slots(
                        &state,
                        ingress.authenticated_non_validator_source_capacity,
                    ),
                13,
                "draining one source restores its latent first-message reservation"
            );
        }
        assert!(matches!(
            ingress.try_push(inbound(3, source_c.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let second = ingress
            .try_recv()
            .expect("source B remains ahead of newly admitted source C");
        assert_eq!(second.via(), Some(&source_b));
        let third = ingress.try_recv().expect("source C receives its fair turn");
        assert_eq!(third.via(), Some(&source_c));
        assert!(ingress.try_recv().is_none());
    }

    include!("tests/mod_authoritative_runtime_gate_04_routes_and_dequeue.rs");
    #[test]
    fn transport_reply_route_construction_is_fallible_and_target_bound() {
        let semantic_origin = PeerId::from(KeyPair::random().public_key().clone());
        let other_origin = PeerId::from(KeyPair::random().public_key().clone());
        let authenticated_via = PeerId::from(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(authenticated_via.clone());
        let live = routes.mint_via(semantic_origin.clone(), authenticated_via.clone());
        let message = v2_auxiliary_prepare(0);

        let inbound = InboundBlockMessage::try_from_transport_with_reply_route(
            message.clone(),
            semantic_origin.clone(),
            authenticated_via.clone(),
            live,
        )
        .expect("live target-bound route must be retained");
        let (_, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        assert_eq!(sender, Some(semantic_origin.clone()));
        assert_eq!(reply_routes.expect("live reply route set").len(), 1);

        let inactive = routes.mint_via(semantic_origin.clone(), authenticated_via.clone());
        assert!(routes.retire(&inactive));
        assert!(matches!(
            InboundBlockMessage::try_from_transport_with_reply_route(
                message.clone(),
                semantic_origin.clone(),
                authenticated_via.clone(),
                inactive,
            ),
            Err(NetworkReplyRouteError::Inactive)
        ));

        let retargeted = routes.mint_via(other_origin, authenticated_via.clone());
        assert!(matches!(
            InboundBlockMessage::try_from_transport_with_reply_route(
                message.clone(),
                semantic_origin.clone(),
                authenticated_via.clone(),
                retargeted,
            ),
            Err(NetworkReplyRouteError::Retargeted)
        ));

        let wrong_via = PeerId::from(KeyPair::random().public_key().clone());
        let mismatched_delivery = routes.mint_via(semantic_origin.clone(), wrong_via);
        assert!(matches!(
            InboundBlockMessage::try_from_transport_with_reply_route(
                message.clone(),
                semantic_origin.clone(),
                authenticated_via.clone(),
                mismatched_delivery,
            ),
            Err(NetworkReplyRouteError::DifferentSource)
        ));

        let direct =
            InboundBlockMessage::from_transport(message, semantic_origin, authenticated_via);
        assert!(direct.into_message_sender_and_reply_routes().2.is_none());
    }

    #[test]
    fn fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
        let mut sources = validator_peers(2);
        let source_b = sources.pop().expect("second authenticated source");
        let source_a = sources.pop().expect("first authenticated source");
        let semantic_origin = PeerId::from(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source_a.clone());
        let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
        ingress.close();
        ingress
            .configure_roster([source_a.clone(), source_b.clone()])
            .expect("two authenticated lanes plus anonymous reserve fit");
        ingress.open().expect("open configured roster");
        let request = v2_auxiliary_prepare(0);

        for (via, route) in [
            (source_a.clone(), route_a.clone()),
            (source_b, route_b.clone()),
        ] {
            assert!(matches!(
                ingress.try_push(
                    InboundBlockMessage::try_from_transport_with_reply_route(
                        request.clone(),
                        semantic_origin.clone(),
                        via,
                        route,
                    )
                    .expect("live route matches its semantic request target")
                ),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
                    | Ok(super::FairV2IngressPushDisposition::Coalesced)
            ));
        }
        let later_a = routes
            .redeliver(&route_a)
            .expect("same-source later delivery capability");
        assert!(matches!(
            ingress.try_push(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    request.clone(),
                    semantic_origin.clone(),
                    source_a.clone(),
                    later_a.clone(),
                )
                .expect("later delivery remains a live route")
            ),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert_eq!(
            ingress.len(),
            1,
            "one semantic request owns all authenticated return routes"
        );

        let delivered = ingress.try_recv().expect("deliver the queued occurrence");
        assert_eq!(delivered.sender(), Some(&semantic_origin));
        let (_, _, delivered_routes) = delivered.into_message_sender_and_reply_routes();
        let delivered_routes = delivered_routes.expect("authenticated routes survive dequeue");
        assert_eq!(delivered_routes.len(), 2);
        assert!(
            delivered_routes
                .iter()
                .any(|route| route.same_delivery(&later_a))
        );
        assert!(
            delivered_routes
                .iter()
                .any(|route| route.same_delivery(&route_b))
        );
        assert_eq!(ingress.len(), 0);
        let next_a = routes
            .redeliver(&later_a)
            .expect("new queue-scoped delivery capability");
        assert!(matches!(
            ingress.try_push(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    request,
                    semantic_origin,
                    source_a,
                    next_a,
                )
                .expect("next delivery remains a live route")
            ),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            ingress.len(),
            1,
            "coalescing is queue-scoped and ends when the consumer takes ownership"
        );
    }

    #[test]
    fn alternate_reply_route_attaches_before_authenticated_source_lane_cap() {
        const SOURCE_BYTES: usize = 1024 * 1024;
        let ingress = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            10,
            3 * SOURCE_BYTES,
            SOURCE_BYTES,
            0,
            0,
            0,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            Some(1),
        );
        let mut peers = validator_peers(3);
        let source_b = peers.pop().expect("source B");
        let source_a = peers.pop().expect("source A");
        let validator = peers.pop().expect("validator");
        let semantic_origin = PeerId::from(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source_a.clone());
        let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
        let request = v2_auxiliary_prepare(0);
        ingress
            .configure_roster([validator])
            .expect("one validator, one authenticated relay, and anonymous fit exactly");
        ingress.open().expect("open exact source geometry");

        let inbound = |message: BlockMessage, via: PeerId, route: NetworkReplyRoute| {
            InboundBlockMessage::try_from_transport_with_reply_route(
                message,
                semantic_origin.clone(),
                via,
                route,
            )
            .expect("route binds the exact semantic request and source")
        };
        assert!(matches!(
            ingress.try_push(inbound(request.clone(), source_a, route_a)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(inbound(request, source_b.clone(), route_b)),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_transport(
                v2_auxiliary_prepare(1),
                semantic_origin,
                source_b,
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ));

        let delivered = ingress
            .try_recv()
            .expect("semantic owner remains queued once");
        assert_eq!(
            delivered
                .into_message_sender_and_reply_routes()
                .2
                .expect("both exact routes remain attached")
                .len(),
            2
        );
    }

    #[test]
    fn fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
        let mut sources = validator_peers(2);
        let source_b = sources.pop().expect("second authenticated source");
        let source_a = sources.pop().expect("first authenticated source");
        let semantic_origin = PeerId::from(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source_a.clone());
        let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        let request = v2_auxiliary_prepare(0);
        ingress.close();
        ingress
            .configure_roster([source_a.clone(), source_b.clone()])
            .expect("two authenticated lanes plus anonymous reserve fit");
        ingress.open().expect("open configured roster");

        let inbound = |via: PeerId, route: NetworkReplyRoute| {
            InboundBlockMessage::try_from_transport_with_reply_route(
                request.clone(),
                semantic_origin.clone(),
                via,
                route,
            )
            .expect("test route binds the semantic request and authenticated source")
        };
        assert!(matches!(
            ingress.try_push(inbound(source_a.clone(), route_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(inbound(source_a.clone(), route_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));

        {
            let mut state = ingress.state.lock();
            let entry = state
                .lanes
                .get_mut(&super::FairV2IngressSource::Validator(source_a.clone()))
                .and_then(|lane| lane.entries.front_mut())
                .expect("source A owns the queued semantic request");
            let evidence = Arc::make_mut(&mut entry.inbound)
                .ingress_ownership
                .as_mut()
                .expect("fair admission attached exact ownership evidence");
            assert!(evidence.advance_reply_cursors(&route_a, 7, 11));
            assert!(!evidence.advance_reply_cursors(&route_a, 6, 11));
            let unowned_later_delivery = routes
                .redeliver(&route_a)
                .expect("mint a same-target delivery owned by another canonical request");
            assert!(unowned_later_delivery.same_source(&route_a));
            assert!(unowned_later_delivery.same_request_authority(&route_a));
            assert!(!unowned_later_delivery.same_delivery(&route_a));
            assert!(
                !evidence.advance_reply_cursors(&unowned_later_delivery, 8, 12),
                "a later capability not installed in this request cannot advance its cursor"
            );
            let foreign_origin = PeerId::from(KeyPair::random().public_key().clone());
            let retargeted_same_hub = routes.mint_via(foreign_origin, source_a.clone());
            assert!(retargeted_same_hub.same_source(&route_a));
            assert!(!retargeted_same_hub.same_request_authority(&route_a));
            assert!(
                !evidence.advance_reply_cursors(&retargeted_same_hub, 8, 12),
                "a same-hub capability for another relayed origin cannot advance this request"
            );
            let source_a_attempt = evidence
                .attempts
                .iter()
                .find(|attempt| attempt.route.same_source(&route_a))
                .expect("source A retains its original request authority");
            assert_eq!(
                (
                    source_a_attempt.message_cursor,
                    source_a_attempt.chunk_cursor
                ),
                (7, 11),
                "retargeted rejection cannot regress or advance the retained cursor"
            );
            assert!(evidence.validate_exact());
        }

        let later_a = routes
            .redeliver(&route_a)
            .expect("same-tenure later delivery");
        assert!(matches!(
            ingress.try_push(inbound(source_a.clone(), later_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        let reconnect_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        assert!(matches!(
            ingress.try_push(inbound(source_a.clone(), reconnect_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            routes.retire(&reconnect_a),
            "source A retires while its semantic request remains queued"
        );
        let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
        assert!(matches!(
            ingress.try_push(inbound(source_b.clone(), route_b.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        let resumed_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        assert!(matches!(
            ingress.try_push(inbound(source_a.clone(), resumed_a.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));

        let mut delivered = ingress.try_recv().expect("deliver the semantic owner");
        let evidence = delivered
            .ingress_ownership()
            .expect("ownership carrier survives fair dequeue")
            .clone();
        assert!(evidence.validate_exact());
        assert_eq!(evidence.occurrence_count, 6);
        assert_eq!(evidence.action_counts, [1, 1, 1, 2, 1]);
        assert!(Arc::ptr_eq(
            &evidence.first.encoded_bytes,
            &evidence.latest.encoded_bytes
        ));
        assert_eq!(
            evidence.latest_action(),
            super::FairV2IngressOwnershipAction::Reconnect
        );
        let source_a_attempt = evidence
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&resumed_a))
            .expect("source A attempt survives reconnect");
        assert!(source_a_attempt.route.same_delivery(&resumed_a));
        assert_eq!(source_a_attempt.message_cursor, 7);
        assert_eq!(source_a_attempt.chunk_cursor, 11);
        let source_b_attempt = evidence
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_b))
            .expect("new alternate source starts an independent attempt");
        assert_eq!(source_b_attempt.message_cursor, 0);
        assert_eq!(source_b_attempt.chunk_cursor, 0);

        let mut projected_routes = delivered
            .clone()
            .into_message_sender_and_reply_routes()
            .2
            .expect("dequeued request retains its independently carried routes");
        let mut projected_evidence = evidence.clone();
        let (retained, prune_receipt) = projected_routes.retain_active_with_receipt();
        assert_eq!(
            retained, 2,
            "both sources are live at the authoritative snapshot"
        );
        assert!(
            routes.retire(&route_b),
            "source B disconnects after the route snapshot"
        );
        projected_routes = projected_evidence
            .project_retained_reply_routes(prune_receipt)
            .expect(
                "a post-snapshot disconnect cannot make ownership drop a route retained by that snapshot",
            );
        assert_eq!(projected_routes.len(), 2);
        let projected_a = projected_evidence
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&resumed_a))
            .expect("source A remains independently owned");
        assert_eq!(
            (projected_a.message_cursor, projected_a.chunk_cursor),
            (7, 11)
        );
        assert!(
            projected_evidence
                .attempts
                .iter()
                .any(|attempt| attempt.route.same_delivery(&route_b)),
            "the first projection must preserve the exact route retained by its snapshot"
        );

        let (retained, prune_receipt) = projected_routes.retain_active_with_receipt();
        assert_eq!(
            retained, 1,
            "the next bounded snapshot observes source B's retirement"
        );
        projected_routes = projected_evidence
            .project_retained_reply_routes(prune_receipt)
            .expect("the next receipt removes only source B");
        assert!(projected_evidence.validate_exact());
        assert!(projected_evidence.matches_reply_routes(Some(&projected_routes)));
        assert_eq!(
            projected_evidence.attempts.len(),
            2,
            "pruning parks source B's bounded cursor instead of erasing its owner"
        );
        let projected_a = projected_evidence
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&resumed_a))
            .expect("source A retains its live cursor");
        assert!(projected_a.route.same_delivery(&resumed_a));
        assert_eq!(
            (projected_a.message_cursor, projected_a.chunk_cursor),
            (7, 11)
        );
        let projected_b = projected_evidence
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_b))
            .expect("source B retains its parked cursor");
        assert!(projected_b.route.same_delivery(&route_b));
        assert!(!projected_b.route.is_active());
        assert_eq!(
            (projected_b.message_cursor, projected_b.chunk_cursor),
            (0, 0)
        );

        let rejected = |label: &str, mutated: super::FairV2IngressOwnershipEvidence| {
            assert!(!mutated.validate_exact(), "accepted mutated {label}");
        };

        let mut mutated = evidence.clone();
        mutated.latest.wire_key.hash = CryptoHash::new(b"mutated semantic hash");
        rejected("semantic hash", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.semantic_origin = None;
        rejected("semantic origin", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.authenticated_via = None;
        rejected("authenticated delivery peer", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.authenticated_source = super::FairV2IngressSource::Anonymous;
        rejected("authenticated source", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.message_kind = super::FairV2IngressMessageKind::V2Proposal;
        rejected("message kind", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.class = super::FairV2IngressClass::Progress;
        rejected("message class", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.encoded_bytes = Arc::<[u8]>::from(vec![0xFF]);
        rejected("canonical bytes", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.resource_after.global_len =
            mutated.latest.resource_after.global_len.saturating_add(1);
        rejected("global length", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.resource_after.source_bytes =
            mutated.latest.resource_after.source_bytes.saturating_add(1);
        rejected("source bytes", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.resource_after.timeout_vote_byte_reserve = mutated
            .latest
            .resource_after
            .timeout_vote_byte_reserve
            .saturating_add(1);
        rejected("timeout reserve", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.resource_after.message_capacity = mutated
            .latest
            .resource_after
            .message_capacity
            .saturating_add(1);
        rejected("message capacity", mutated);

        let mut mutated = evidence.clone();
        mutated.latest.route_capacity = mutated
            .latest
            .route_capacity
            .and_then(|capacity| capacity.checked_add(1));
        rejected("route capacity", mutated);

        let mut mutated = evidence.clone();
        mutated.action_counts[super::FairV2IngressOwnershipAction::ExactDuplicate.index()] += 1;
        rejected("action count", mutated);

        let mut mutated = evidence;
        mutated.attempts[0].message_cursor = mutated.attempts[0].message_cursor.saturating_add(1);
        rejected("attempt cursor", mutated);

        assert!(delivered.take_ingress_ownership().is_some());
        assert!(delivered.ingress_ownership().is_none());
    }

    include!("tests/mod_authoritative_runtime_gate_05_ownership_maintenance.rs");
    #[test]
    fn fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let authenticated_via = validator_peers(1).pop().expect("validator fixture");
        let origin_a = PeerId::from(KeyPair::random().public_key().clone());
        let origin_b = PeerId::from(KeyPair::random().public_key().clone());
        let request = v2_auxiliary_prepare(0);
        ingress.close();
        ingress
            .configure_roster([authenticated_via.clone()])
            .expect("validator plus anonymous lane fit");
        ingress.open().expect("open configured roster");

        for origin in [origin_a, origin_b] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::from_transport(
                    request.clone(),
                    origin,
                    authenticated_via.clone(),
                )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ));
        }

        let first = ingress
            .try_recv()
            .and_then(|inbound| inbound.ingress_ownership().cloned())
            .expect("first exact ownership carrier");
        let second = ingress
            .try_recv()
            .and_then(|inbound| inbound.ingress_ownership().cloned())
            .expect("second exact ownership carrier");
        assert!(first.validate_exact());
        assert!(second.validate_exact());
        assert!(!first.same_semantic_request(&second));
        assert_ne!(
            first.process_local_projection_hash(),
            second.process_local_projection_hash(),
            "process-local scheduler projections must bind semantic origin"
        );
    }

    include!("tests/mod_authoritative_runtime_gate_06_source_isolation.rs");
    #[test]
    fn fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let auxiliary = v2_auxiliary_prepare(0);
        let progress = v2_commit_certificate_request(0, &validator);
        let second_progress = v2_commit_certificate_request(1, &validator);
        let timeout = v2_timeout_vote();
        let body_response = v2_certified_body_response(0, 0, 64);
        let chunk = v2_message_with_bytes(0, 64);
        let ordinary_bytes = encoded_v2_len(&auxiliary)
            .checked_add(encoded_v2_len(&progress))
            .expect("ordinary fixture bytes fit usize");
        let timeout_bytes = encoded_v2_len(&timeout);
        let completion_bytes = encoded_v2_len(&body_response).max(encoded_v2_len(&chunk));
        let source_bytes = ordinary_bytes
            .checked_add(timeout_bytes)
            .and_then(|bytes| bytes.checked_add(completion_bytes))
            .expect("disjoint test partitions fit usize");
        let ingress = super::FairV2Ingress::new(
            7,
            2 * source_bytes,
            source_bytes,
            timeout_bytes,
            completion_bytes,
        );
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous partitions fit");
        ingress.open().expect("open configured roster");

        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::new(
                progress.clone(),
                Some(validator.clone()),
            )),
            FairV2IngressClass::Progress
        );
        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::new(
                body_response.clone(),
                Some(validator.clone()),
            )),
            FairV2IngressClass::TransportCompletion
        );
        for message in [auxiliary, progress, timeout] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::new(message, Some(validator.clone()))),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ));
        }
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    second_progress,
                    Some(validator.clone()),
                )),
                Err(super::FairV2IngressPushError::Full(_))
            ),
            "ordinary Progress cannot spend the completion corridor"
        );
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                body_response,
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));

        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_transport_completion)
            .expect("body response bypasses saturated ordinary, Progress, and timeout owners");
        assert!(matches!(
            delivered.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_),
                ..
            })
        ));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(chunk, Some(validator))),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ),
            "PayloadChunk shares the released completion corridor"
        );
    }

    #[test]
    fn fair_v2_ingress_completion_owner_is_source_isolated_and_queue_scoped() {
        let validators = validator_peers(3);
        let first = validators[0].clone();
        let second = validators[1].clone();
        let outsider = validators[2].clone();
        let chunk = v2_message_with_bytes(0, 64);
        let response = v2_certified_body_response(0, 0, 64);
        let completion_bytes = encoded_v2_len(&chunk).max(encoded_v2_len(&response));
        let source_bytes = completion_bytes + 1;
        let ingress =
            super::FairV2Ingress::new(12, 3 * source_bytes, source_bytes, 0, completion_bytes);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validators and anonymous source fit");
        ingress.open().expect("open configured roster");

        let oversized = v2_message_with_bytes(7, completion_bytes + 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(oversized, Some(first.clone()))),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(chunk.clone(), Some(first.clone()))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(chunk, Some(first.clone()))),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    response.clone(),
                    Some(first.clone())
                )),
                Err(super::FairV2IngressPushError::Full(_))
            ),
            "one source serializes chunk/response conflicts through one shared owner"
        );
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    response.clone(),
                    Some(second.clone())
                )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ),
            "one validator cannot consume another validator's completion owner"
        );
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(response.clone(), Some(outsider))),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(response.clone(), None)),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));

        let first_completion = ingress
            .try_recv_if(|inbound| inbound.sender() == Some(&first))
            .expect("fair service releases the first validator's completion owner");
        assert!(super::fair_v2_ingress_is_transport_completion(
            &first_completion
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(response, Some(first))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
    }

    #[test]
    fn fair_v2_ingress_rejects_insufficient_roster_byte_partition() {
        let validators = validator_peers(2);
        let ingress = super::FairV2Ingress::new(12, 2 * 1024, 1024, 0, 0);
        let error = ingress
            .configure_roster(validators)
            .expect_err("two validators plus anonymous require three byte partitions");
        assert!(error.is_bytes());
        assert_eq!(error.configured(), 2 * 1024);
        assert_eq!(error.required(), 3 * 1024);
    }

    #[test]
    fn fair_v2_ingress_required_serve_gate_precedes_open() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(7, 2 * 1024, 1024, 0, 0);
        ingress
            .configure_roster([validator])
            .expect("validator and anonymous ownership partitions fit");
        ingress.require_certified_serve_gate();

        let error = ingress
            .open()
            .expect_err("production admission cannot open before its Serve gate binds");
        assert_eq!(
            error.kind,
            super::FairV2IngressCapacityKind::CertifiedServeGate
        );
        assert_eq!(error.configured(), 0);
        assert_eq!(error.required(), 1);
        assert!(
            !ingress.state.lock().open,
            "failed open leaves admission closed"
        );
    }

    #[test]
    fn fair_v2_ingress_reserves_timeout_vote_bytes_behind_auxiliary_pressure() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let auxiliary = v2_auxiliary_prepare(0);
        let timeout_vote = v2_maximum_valid_timeout_vote_wire();
        let auxiliary_len = encoded_v2_len(&auxiliary);
        let timeout_vote_len = encoded_v2_len(&timeout_vote);
        let source_capacity = auxiliary_len + timeout_vote_len;
        let ingress =
            super::FairV2Ingress::new(7, 2 * source_capacity, source_capacity, timeout_vote_len, 0);
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous byte partitions fit");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(auxiliary, Some(validator.clone()),)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                v2_auxiliary_prepare(1),
                Some(validator.clone()),
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                timeout_vote,
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));

        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
            .expect("reserved timeout vote bypasses the byte-saturated auxiliary prefix");
        assert_eq!(delivered.sender(), Some(&validator));
        assert!(super::fair_v2_ingress_is_timeout_vote(&delivered));
    }

    #[test]
    fn fair_v2_ingress_serializes_distinct_timeout_vote_byte_owners() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let first = v2_timeout_vote();
        let second = v2_maximum_valid_timeout_vote_wire();
        let reserve = encoded_v2_len(&first)
            .checked_add(encoded_v2_len(&second))
            .and_then(|bytes| bytes.checked_add(1))
            .expect("fixture byte sum fits usize");
        let source_capacity = reserve.checked_add(1).expect("ordinary-byte partition");
        let ingress =
            super::FairV2Ingress::new(7, 2 * source_capacity, source_capacity, reserve, 0);
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous byte partitions fit");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(first, Some(validator.clone()))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                second.clone(),
                Some(validator.clone()),
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ));

        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
            .expect("fair service releases the first TimeoutVote byte owner");
        assert!(super::fair_v2_ingress_is_timeout_vote(&delivered));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(second, Some(validator))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
    }

    #[test]
    fn fair_v2_ingress_maximum_valid_timeout_vote_fits_production_byte_reserve() {
        let encoded_len = encoded_v2_len(&v2_maximum_valid_timeout_vote_wire());
        assert!(encoded_len <= super::MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES);
        assert!(encoded_len <= super::TIMEOUT_VOTE_RESERVE_BYTES);
    }

    #[test]
    fn fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions() {
        let layout = wire::SumeragiV2GenesisContextParameters::recommended().da_layout;
        assert_eq!(
            usize::try_from(layout.max_chunk_count).expect("recommended count fits usize"),
            iroha_config::parameters::defaults::sumeragi::RECOMMENDED_DA_MAX_CHUNK_COUNT,
            "config completion allowance must track the signed recommended layout"
        );
        let required = super::fair_v2_ingress_required_transport_completion_bytes(layout);
        assert_eq!(
            required, 16_811_581,
            "recommended wire ceiling is a regression boundary"
        );
        let required_proposal =
            super::fair_v2_ingress_required_proposal_bytes(layout, wire::MAX_VALIDATORS_PER_HEIGHT);
        assert_eq!(
            required_proposal, 70_940,
            "maximal proposal wire geometry is a regression boundary"
        );
        let proposal = v2_maximum_structural_proposal_wire(layout, wire::MAX_VALIDATORS_PER_HEIGHT);
        assert_eq!(
            encoded_v2_len(&proposal),
            required_proposal,
            "checked activation geometry must equal canonical bare Norito"
        );
        let BlockMessage::V2(proposal_envelope) = &proposal else {
            unreachable!("maximum proposal fixture is v2");
        };
        let wire::ConsensusMessageV2Payload::Proposal(maximum_proposal) =
            &proposal_envelope.payload
        else {
            unreachable!("maximum proposal fixture carries Proposal");
        };
        let wire::ProposalJustification::Timeout(timeout_justification) =
            &maximum_proposal.justification
        else {
            unreachable!("maximum proposal fixture carries Timeout justification");
        };
        let maximum_timeout_certificate = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(
                timeout_justification.timeout_certificate.clone(),
            ),
        ));
        let required_certified_fence_escape =
            super::fair_v2_ingress_required_certified_fence_escape_bytes(
                wire::MAX_VALIDATORS_PER_HEIGHT,
            );
        assert_eq!(
            encoded_v2_len(&maximum_timeout_certificate),
            required_certified_fence_escape,
            "maximal-roster TC must equal the checked certified-fence ceiling",
        );
        assert!(
            required_certified_fence_escape <= super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            "the production certified partition must contain every legal TC/CommitQC envelope",
        );
        let maximal_roster = validator_peers(
            u8::try_from(wire::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound fits u8"),
        );
        assert!(
            required_proposal
                >= super::fair_v2_ingress_required_commit_certificate_response_bytes(
                    wire::MAX_VALIDATORS_PER_HEIGHT,
                ),
            "maximal-roster proposal must dominate other safety/control envelopes"
        );
        let maximal_peer = maximal_roster.first().expect("non-empty maximal roster");
        let network_message = crate::NetworkMessage::SumeragiBlock(Arc::new(
            super::message::BlockMessageWire::new(proposal),
        ));
        assert_eq!(
            super::fair_v2_ingress_network_message_bytes(required_proposal),
            Some(network_message.encoded_len()),
            "checked nesting must equal the core NetworkMessage encoding"
        );
        let exact_direct_frame = iroha_p2p::network::data_frame_wire_len(
            maximal_peer,
            Some(maximal_peer),
            u8::MAX,
            iroha_p2p::network::message::Priority::High,
            &network_message,
        );
        let exact_broadcast_frame = iroha_p2p::network::data_frame_wire_len(
            maximal_peer,
            None,
            u8::MAX,
            iroha_p2p::network::message::Priority::High,
            &network_message,
        );
        let required_control_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(required_proposal);
        let network_message_bytes = network_message.encoded_len();
        assert_eq!(
            required_control_frame,
            iroha_p2p::network::data_frame_wire_len_from_payload_len_with_peer_key_bytes::<
                crate::NetworkMessage,
            >(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
                Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
                iroha_p2p::network::MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                network_message_bytes,
            ),
            "protocol-maximum identities must use the exact complete direct P2P wire"
        );
        assert!(required_control_frame >= exact_direct_frame);
        assert!(exact_direct_frame > exact_broadcast_frame);

        let minimal_layout = minimal_rs16_layout();
        let minimal_proposal_bytes =
            super::fair_v2_ingress_required_proposal_bytes(minimal_layout, 1);
        assert_eq!(minimal_proposal_bytes, 2_523);
        assert_eq!(
            encoded_v2_len(&v2_maximum_structural_proposal_wire(minimal_layout, 1)),
            minimal_proposal_bytes,
            "minimal-layout/single-validator geometry must not rely on maximal-roster dominance"
        );
        assert!(
            super::fair_v2_ingress_required_commit_certificate_response_bytes(1)
                > minimal_proposal_bytes,
            "protocol-maximum rotated responder must dominate a minimal proposal"
        );
        let minimal_peer = maximal_roster
            .first()
            .expect("maximal roster fixture is non-empty");
        let (_, minimal_peer_key_bytes) = minimal_peer
            .public_key()
            .try_to_bytes()
            .expect("fixture public key is canonical");
        let recovery_chain_id = ChainId::from("fair-v2-ingress-test");
        let (body_request, commit_request, commit_response) =
            v2_maximum_recovery_wires(&recovery_chain_id, minimal_peer, 1);
        assert_eq!(
            super::fair_v2_ingress_required_recovery_request_bytes_for_key(
                &recovery_chain_id,
                1,
                minimal_peer_key_bytes.len(),
            ),
            encoded_v2_len(&body_request).max(encoded_v2_len(&commit_request)),
            "checked request geometry must equal canonical bare Norito"
        );
        assert_eq!(
            super::fair_v2_ingress_required_commit_certificate_response_bytes_for_key(
                1,
                minimal_peer_key_bytes.len(),
            ),
            encoded_v2_len(&commit_response),
            "checked recovery-response geometry must equal canonical bare Norito"
        );

        let source_bytes =
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
        let ordinary_bytes = iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES
            .get()
            .checked_add(super::BODY_ENVELOPE_HEADROOM_BYTES)
            .expect("default ordinary partition fits usize");
        let completion_bytes = source_bytes
            .checked_sub(super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES)
            .and_then(|bytes| bytes.checked_sub(super::TIMEOUT_VOTE_RESERVE_BYTES))
            .and_then(|bytes| bytes.checked_sub(ordinary_bytes))
            .expect("default source partition is disjoint");
        assert!(completion_bytes >= required);

        let global_plaintext = iroha_p2p::frame_plaintext_cap(
            iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
        );
        let ingress = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            22,
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get(),
            source_bytes,
            super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            super::TIMEOUT_VOTE_RESERVE_BYTES,
            completion_bytes,
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get()),
            global_plaintext
                .min(iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get()),
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES
                .get(),
            None,
        );
        ingress
            .configure_roster_for_context(
                validator_peers(4),
                &ChainId::from("fair-v2-ingress-test"),
                layout,
            )
            .expect("recommended four-validator genesis context fits default ingress bytes");
        ingress.open().expect("recommended ingress opens");
    }

    #[test]
    fn fair_v2_ingress_exact_response_bound_accepts_required_and_rejects_required_minus_one() {
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 64 * 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 8 * 1024 * 1024,
            max_chunk_count: 64,
        };
        let response = v2_maximum_certified_body_response(layout);
        let required = super::fair_v2_ingress_required_transport_completion_bytes(layout);
        assert_eq!(encoded_v2_len(&response), required);
        let validator = validator_peers(1).pop().expect("validator fixture");
        let network_response = crate::NetworkMessage::SumeragiBlock(Arc::new(
            super::message::BlockMessageWire::new(response.clone()),
        ));
        assert_eq!(
            super::fair_v2_ingress_network_message_bytes(required),
            Some(network_response.encoded_len()),
            "maximum completion must cross the exact full NetworkMessage framing"
        );
        let actual_direct_response_frame = iroha_p2p::network::data_frame_wire_len(
            &validator,
            Some(&validator),
            u8::MAX,
            iroha_p2p::network::message::Priority::High,
            &network_response,
        );
        assert_eq!(
            iroha_p2p::network::data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
                &validator,
                Some(&validator),
                network_response.encoded_len(),
            ),
            actual_direct_response_frame,
            "allocation-free P2P geometry must remain exact beyond the 2^24 prefix boundary"
        );
        let protocol_maximum_response_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(required);
        assert_eq!(
            protocol_maximum_response_frame,
            iroha_p2p::network::data_frame_wire_len_from_payload_len_with_peer_key_bytes::<
                crate::NetworkMessage,
            >(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
                Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
                iroha_p2p::network::MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                network_response.encoded_len(),
            ),
            "maximum completion must retain exact protocol-maximum direct-relay geometry"
        );
        assert!(protocol_maximum_response_frame >= actual_direct_response_frame);
        let chain_id = ChainId::from("fair-v2-ingress-test");
        let roster_len = 1;
        let certified_bytes =
            super::fair_v2_ingress_required_certified_fence_escape_bytes(roster_len);
        let proposal_bytes = super::fair_v2_ingress_required_proposal_bytes(layout, roster_len);
        let control_message_bytes = proposal_bytes
            .max(super::fair_v2_ingress_required_commit_certificate_response_bytes(roster_len));
        let request_bytes =
            super::fair_v2_ingress_required_recovery_request_bytes(&chain_id, roster_len);
        let lane_progress_frame = super::fair_v2_ingress_required_lane_p2p_frame_bytes(
            super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES,
        );
        let lane_completion_frame = super::fair_v2_ingress_required_lane_p2p_frame_bytes(
            super::MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES,
        );
        let consensus_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(request_bytes).max(lane_progress_frame);
        let control_frame = super::fair_v2_ingress_required_p2p_frame_bytes(control_message_bytes);
        let block_sync_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(required).max(lane_completion_frame);
        assert_eq!(block_sync_frame, protocol_maximum_response_frame);
        let outbound_high_frame =
            iroha_p2p::frame_queue_charge(consensus_frame.max(control_frame).max(block_sync_frame))
                .expect("test transport queue charge fits usize");
        let ordinary_bytes = super::BODY_ENVELOPE_HEADROOM_BYTES
            .max(control_message_bytes)
            .max(request_bytes)
            .max(super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);

        let short_source = ordinary_bytes
            .checked_add(certified_bytes)
            .and_then(|bytes| bytes.checked_add(required - 1))
            .expect("test source bound fits usize");
        let short = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            7,
            2 * short_source,
            short_source,
            certified_bytes,
            0,
            required - 1,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        );
        let error = short
            .configure_roster_for_context(
                [validator.clone()],
                &ChainId::from("fair-v2-ingress-test"),
                layout,
            )
            .expect_err("one byte below the exact completion ceiling fails closed");
        assert_eq!(error.configured(), required - 1);
        assert_eq!(error.required(), required);
        assert_eq!(
            short.open(),
            Err(error),
            "open rechecks the frozen context's exact completion requirement"
        );

        let ordinary_short_source = certified_bytes
            .checked_add(required)
            .and_then(|bytes| bytes.checked_add(ordinary_bytes - 1))
            .expect("test source bound fits usize");
        let ordinary_short =
            super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                7,
                2 * ordinary_short_source,
                ordinary_short_source,
                certified_bytes,
                0,
                required,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                None,
            );
        let ordinary_error = ordinary_short
            .configure_roster_for_context(
                [validator.clone()],
                &ChainId::from("fair-v2-ingress-test"),
                layout,
            )
            .expect_err("completion reserve cannot consume the reviewed ordinary region");
        assert_eq!(ordinary_error.configured(), ordinary_bytes - 1);
        assert_eq!(ordinary_error.required(), ordinary_bytes);
        assert_eq!(ordinary_short.open(), Err(ordinary_error));

        let source_bytes = ordinary_bytes
            .checked_add(certified_bytes)
            .and_then(|bytes| bytes.checked_add(required))
            .expect("test source bound fits usize");
        let invalid_chain_id =
            "x".repeat(iroha_data_model::id::MAX_CHAIN_ID_BYTES.saturating_add(1));
        assert!(
            ChainId::try_from(invalid_chain_id).is_err(),
            "an overlong chain id must be rejected before ingress sizing"
        );
        let maximum_chain_id =
            ChainId::try_from("x".repeat(iroha_data_model::id::MAX_CHAIN_ID_BYTES))
                .expect("maximum-length canonical chain id");
        let maximum_request_bytes =
            super::fair_v2_ingress_required_recovery_request_bytes(&maximum_chain_id, roster_len);
        assert!(
            maximum_request_bytes <= ordinary_bytes,
            "the reviewed ordinary region must cover every canonical chain id"
        );
        let maximum_chain_ingress =
            super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                7,
                2 * source_bytes,
                source_bytes,
                certified_bytes,
                0,
                required,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                None,
            );
        maximum_chain_ingress
            .configure_roster_for_context([validator.clone()], &maximum_chain_id, layout)
            .expect("the maximum canonical chain id fits its ordinary byte owner");

        let ingress_with_caps = |consensus, control, block_sync, outbound_high| {
            super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                7,
                2 * source_bytes,
                source_bytes,
                certified_bytes,
                0,
                required,
                consensus,
                control,
                block_sync,
                outbound_high,
                None,
            )
        };
        let consensus_short = ingress_with_caps(
            consensus_frame - 1,
            control_frame,
            block_sync_frame,
            outbound_high_frame,
        );
        let consensus_error = consensus_short
            .configure_roster_for_context([validator.clone()], &chain_id, layout)
            .expect_err("one byte below the recovery-request frame fails closed");
        assert_eq!(consensus_error.configured(), consensus_frame - 1);
        assert_eq!(consensus_error.required(), consensus_frame);
        assert_eq!(consensus_short.open(), Err(consensus_error));

        let control_short = ingress_with_caps(
            consensus_frame,
            control_frame - 1,
            block_sync_frame,
            outbound_high_frame,
        );
        let control_error = control_short
            .configure_roster_for_context([validator.clone()], &chain_id, layout)
            .expect_err("one byte below the maximal safety/control frame fails closed");
        assert_eq!(control_error.configured(), control_frame - 1);
        assert_eq!(control_error.required(), control_frame);
        assert_eq!(control_short.open(), Err(control_error));

        let block_sync_short = ingress_with_caps(
            consensus_frame,
            control_frame,
            block_sync_frame - 1,
            outbound_high_frame,
        );
        let block_sync_error = block_sync_short
            .configure_roster_for_context([validator.clone()], &chain_id, layout)
            .expect_err("one byte below the payload-completion frame fails closed");
        assert_eq!(block_sync_error.configured(), block_sync_frame - 1);
        assert_eq!(block_sync_error.required(), block_sync_frame);
        assert_eq!(block_sync_short.open(), Err(block_sync_error));

        let outbound_short = ingress_with_caps(
            consensus_frame,
            control_frame,
            block_sync_frame,
            outbound_high_frame - 1,
        );
        let outbound_error = outbound_short
            .configure_roster_for_context([validator.clone()], &chain_id, layout)
            .expect_err("one byte below one encrypted high frame fails closed");
        assert_eq!(outbound_error.configured(), outbound_high_frame - 1);
        assert_eq!(outbound_error.required(), outbound_high_frame);
        assert_eq!(outbound_short.open(), Err(outbound_error));

        let ingress = ingress_with_caps(
            consensus_frame,
            control_frame,
            block_sync_frame,
            outbound_high_frame,
        );
        ingress
            .configure_roster_for_context([validator.clone()], &chain_id, layout)
            .expect("exact completion ceiling leaves a reviewed ordinary partition");
        ingress.open().expect("exactly sized ingress opens");

        let oversized = v2_certified_body_response(9, 0, required + 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(oversized, Some(validator.clone()))),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(response, Some(validator))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
    }

    include!("tests/mod_authoritative_runtime_gate_07_wire_bounds.rs");
    #[test]
    fn fair_v2_ingress_exact_max_chunk_bound_matches_canonical_wire() {
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 4 * 1024 * 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1,
            max_chunk_count: 2,
        };
        let chunk = v2_maximum_payload_chunk(layout);
        assert_eq!(
            encoded_v2_len(&chunk),
            super::fair_v2_ingress_required_transport_completion_bytes(layout)
        );
    }

    #[test]
    fn fair_v2_ingress_completion_bound_overflow_fails_closed() {
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: u32::MAX,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: u64::MAX,
            max_chunk_count: u32::MAX,
        };
        assert_eq!(
            super::fair_v2_ingress_required_transport_completion_bytes(layout),
            usize::MAX
        );
        assert_eq!(
            super::fair_v2_ingress_required_proposal_bytes(layout, usize::MAX),
            usize::MAX
        );

        let ingress = super::FairV2Ingress::new_with_transport_frame_caps(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        );
        let error = ingress
            .configure_roster_for_context(
                validator_peers(1),
                &ChainId::from("overflow-test"),
                layout,
            )
            .expect_err(
                "an unrepresentable outbound queue charge must fail even at usize::MAX capacity",
            );
        assert_eq!(error.required(), usize::MAX);
        assert_eq!(error.configured(), usize::MAX);
        assert_eq!(
            error.kind,
            super::FairV2IngressCapacityKind::OutboundHighFrameBytes
        );
    }

    include!("tests/mod_authoritative_runtime_gate_08_capacity_and_control.rs");
    #[test]
    fn fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
        let validators = validator_peers(2);
        let first_ready_source = validators[0].clone();
        let target_source = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their protected owners, and anonymous fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(
                first_ready_source.clone(),
                v2_auxiliary_prepare(0),
            )
        );
        assert!(matches!(
            ingress.try_push(v2_certified_body_request_inbound(&target_source)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(v2_certified_body_request_inbound(&first_ready_source)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));

        let target = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the exact request passes the blocked first-ready source");
        assert_eq!(target.sender(), Some(&target_source));

        let later_request = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the later Serve request becomes eligible after the target drains");
        assert_eq!(later_request.sender(), Some(&first_ready_source));

        let predecessor = ingress
            .try_recv_if(|_| true)
            .expect("the blocked preexisting entry remains queued");
        assert_eq!(predecessor.sender(), Some(&first_ready_source));
        assert_eq!(vote_height(&predecessor), Some(1));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_certified_request_cutoff_blocks_later_churn() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(27);
        let validators = validator_peers(5);
        let target_source = validators[0].clone();
        let control_source = validators[1].clone();
        let completion_source = validators[2].clone();
        let priority_source = validators[3].clone();
        let causal_source = validators[4].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("five validators, their protected owners, and anonymous fit");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(v2_certified_body_request_inbound(&target_source)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(handle.try_incoming_block_message_from(
            control_source.clone(),
            v2_vote(wire::GlobalPhase::Commit),
        ));
        assert!(handle.try_incoming_block_message_from(
            completion_source.clone(),
            v2_certified_body_response(7, 0, 64),
        ));
        assert!(
            handle.try_incoming_block_message_from(priority_source.clone(), v2_timeout_vote(),)
        );
        assert!(
            handle
                .try_incoming_block_message_from(causal_source.clone(), v2_auxiliary_prepare(11),)
        );

        assert!(
            ingress
                .try_recv_if(|inbound| !fair_v2_ingress_is_certified_body_request(inbound))
                .is_none(),
            "later control, completion, priority, and causal work cannot pass the target"
        );

        let target = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the cutoff target remains selectable");
        assert_eq!(target.sender(), Some(&target_source));

        let control = ingress
            .try_recv_if(|_| true)
            .expect("later control proceeds after the target drains");
        assert_eq!(control.sender(), Some(&control_source));
        assert_eq!(
            vote_phase(&control),
            Some(wire::GlobalPhase::Commit),
            "the first released occurrence is consensus control"
        );

        let completion = ingress
            .try_recv_if(|_| true)
            .expect("later completion proceeds after the target drains");
        assert_eq!(completion.sender(), Some(&completion_source));
        assert!(matches!(
            completion.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_),
                ..
            })
        ));

        let priority = ingress
            .try_recv_if(|_| true)
            .expect("later priority work proceeds after the target drains");
        assert_eq!(priority.sender(), Some(&priority_source));
        assert!(matches!(
            priority.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        ));

        let causal = ingress
            .try_recv_if(|_| true)
            .expect("later causal work proceeds after the target drains");
        assert_eq!(causal.sender(), Some(&causal_source));
        assert_eq!(vote_phase(&causal), Some(wire::GlobalPhase::Prepare));
        assert_eq!(vote_height(&causal), Some(12));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(
            8,
            64 * 1024 * 1024,
            32 * 1024 * 1024,
            super::TIMEOUT_VOTE_RESERVE_BYTES,
            iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get()
                + super::BODY_ENVELOPE_HEADROOM_BYTES,
        );
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous protected owners fit");
        ingress.open().expect("open configured roster");
        let message = v2_certified_body_response(7, 0, 64);

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let first_ordinal = {
            let state = ingress.state.lock();
            assert_eq!(state.last_admission_ordinal, 1);
            state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .next()
                .expect("the request owns one queued occurrence")
                .admission_ordinal
        };

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(message, Some(validator.clone()))),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        {
            let state = ingress.state.lock();
            assert_eq!(state.last_admission_ordinal, first_ordinal);
            assert_eq!(state.len, 1);
            assert_eq!(
                state
                    .lanes
                    .values()
                    .flat_map(|lane| lane.entries.iter())
                    .next()
                    .expect("coalescing retains the original occurrence")
                    .admission_ordinal,
                first_ordinal
            );
        }

        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("rollover reinstalls the validator and anonymous lanes");
        {
            let state = ingress.state.lock();
            assert_eq!(state.len, 0, "rollover clears prior queued ownership");
            assert_eq!(
                state.last_admission_ordinal, first_ordinal,
                "rollover retains the process-local ordinal high-watermark"
            );
        }
        ingress.open().expect("open the rolled-over roster");
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                v2_auxiliary_prepare(8),
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let rollover_ordinal = {
            let state = ingress.state.lock();
            let ordinal = state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .next()
                .expect("the post-rollover occurrence is queued")
                .admission_ordinal;
            assert_eq!(
                ordinal,
                first_ordinal
                    .checked_add(1)
                    .expect("the first test ordinal has a successor"),
                "the first post-rollover occurrence cannot reuse an old ordinal"
            );
            ordinal
        };

        ingress.state.lock().last_admission_ordinal = u64::MAX;
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                v2_auxiliary_prepare(9),
                Some(validator),
            )),
            Err(super::FairV2IngressPushError::FailStop(_))
        ));
        let state = ingress.state.lock();
        assert!(!state.open, "ordinal exhaustion fails admission closed");
        assert_eq!(
            state.len, 1,
            "the retained post-rollover occurrence is not disturbed"
        );
        assert_eq!(
            state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .next()
                .expect("the retained post-rollover occurrence remains queued")
                .admission_ordinal,
            rollover_ordinal
        );
    }

    #[test]
    fn fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(
            8,
            64 * 1024 * 1024,
            32 * 1024 * 1024,
            super::TIMEOUT_VOTE_RESERVE_BYTES,
            iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get()
                + super::BODY_ENVELOPE_HEADROOM_BYTES,
        );
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous protected owners fit");
        ingress.open().expect("open configured roster");
        let message = v2_certified_body_response(7, 0, 64);

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));

        let mut first = ingress
            .try_recv()
            .expect("checked dequeue owns the coalesced request");
        let first_owner = first
            .take_ingress_ownership()
            .expect("checked dequeue retains exact ownership");
        assert_eq!(first_owner.physical_admission_ordinal(), Some(1));
        assert_eq!(first_owner.runtime_physical_cut(), Some(2));
        let mut illegally_refreshed = first_owner.clone();
        assert!(
            !illegally_refreshed.freeze_runtime_physical_cut(3),
            "an admitted occurrence cannot refresh its frozen predecessor cut"
        );
        assert_eq!(illegally_refreshed.runtime_physical_cut(), Some(2));

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(message, Some(validator))),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut retry = ingress
            .try_recv()
            .expect("post-drain transport retry owns a fresh physical occurrence");
        let retry_owner = retry
            .take_ingress_ownership()
            .expect("retry retains exact physical ownership");
        assert_eq!(retry_owner.physical_admission_ordinal(), Some(2));
        assert_eq!(retry_owner.runtime_physical_cut(), Some(3));
        assert_eq!(first_owner.runtime_physical_cut(), Some(2));
    }

    include!("tests/mod_authoritative_runtime_gate_09_snapshot_and_source_lanes.rs");
    #[test]
    fn v2_ingress_rejects_capacity_without_per_validator_progress_reservations() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(21);
        ingress.close();
        let error = ingress
            .configure_roster(validator_peers(4))
            .expect_err(
                "four validators require ordinary, progress, certified, TimeoutVote, and transport-completion slots",
            );
        assert_eq!(error.configured(), 21);
        assert_eq!(error.required(), 22);
        assert_eq!(ingress.open(), Err(error));
    }
}

impl SumeragiWorker {
    fn run(self) {
        v2_runner::run(self);
    }
}
