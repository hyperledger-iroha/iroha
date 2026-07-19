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
            BODY_ENVELOPE_HEADROOM_BYTES, TIMEOUT_VOTE_RESERVE_BYTES, npos::EPOCH_LENGTH_BLOCKS,
        },
    },
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, PublicKey};
use iroha_data_model::{
    ChainId,
    block::consensus_v2::ConsensusMode,
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
use mv::storage::StorageReadOnly;
use norito::codec::Encode as _;
use parking_lot::Mutex;

use crate::{
    merge_sidecar::CertifiedMergeSidecarMessage,
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};

static CONFIGURED_SUMERAGI_STACK_SIZE_BYTES: AtomicUsize = AtomicUsize::new(0);
const WORKER_WAKE_CHANNEL_CAP: usize = 1;
// The valid v2 timeout-vote envelope is bounded by a 128-entry signer vector
// and two individually bounded BLS signatures. Keep this conservative wire
// ceiling aligned with the formal ingress refinement and the maximal fixture
// below; the production byte reserve is intentionally much larger.
const MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES: usize = 4 * 1024;
// Every admitted lane-local message must fit the independently reviewed source
// bundle from which it is reconstructed. This gives the shared ingress one
// finite byte witness for proposals, payload completions, view changes, QCs,
// and the atomic proposal+PrepareQC+CommitQC recovery certificate.
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

/// Resolve the signed on-chain activation lag for VRF penalties.
#[cfg(any(test, feature = "bench", feature = "iroha-core-tests"))]
pub(crate) fn resolve_npos_activation_lag_blocks_from_world(
    world: &impl WorldReadOnly,
) -> Option<u64> {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.activation_lag_blocks())
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
    signed_genesis_voting_peers, staged_genesis_nexus_amx_context_hash,
};
pub(crate) mod v2_effects;
pub(crate) mod v2_lane_work;
#[cfg(any(test, feature = "bench", feature = "iroha-core-tests"))]
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
        /// Authenticated transport sender.
        sender: PeerId,
        /// Certified sidecar protocol message.
        message: CertifiedMergeSidecarMessage,
    },
    /// Native AMX control message with its authenticated transport sender.
    NativeAmx {
        /// Authenticated transport sender.
        sender: PeerId,
        /// Native AMX protocol message.
        message: crate::native_amx::NativeAmxMessage,
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
        }
    }

    /// Consume the envelope and return the normalized message and semantic origin.
    pub(crate) fn into_message_and_sender(self) -> (BlockMessage, Option<PeerId>) {
        (self.message, self.sender)
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

    /// Borrow the authenticated transport hop used for resource isolation.
    fn via(&self) -> Option<&PeerId> {
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
    Untrusted,
}

struct FairV2IngressState {
    roster: BTreeSet<PeerId>,
    lanes: BTreeMap<FairV2IngressSource, FairV2IngressLane>,
    ready: VecDeque<FairV2IngressSource>,
    len: usize,
    bytes: usize,
    nonempty_since: Option<Instant>,
    last_service_attempt_at: Option<Instant>,
    required_ordinary_bytes: usize,
    required_transport_completion_bytes: usize,
    required_consensus_frame_bytes: usize,
    required_control_frame_bytes: usize,
    required_block_sync_frame_bytes: usize,
    required_outbound_high_frame_bytes: usize,
    open: bool,
}

#[derive(Default)]
struct FairV2IngressLane {
    entries: VecDeque<FairV2IngressEntry>,
    pending_wire: BTreeSet<FairV2IngressWireKey>,
    progress_len: usize,
    timeout_vote_len: usize,
    transport_completion_len: usize,
    bytes: usize,
    timeout_vote_bytes: usize,
    transport_completion_bytes: usize,
}

struct FairV2IngressEntry {
    inbound: InboundBlockMessage,
    enqueued_at: Instant,
    class: FairV2IngressClass,
    wire_key: Option<FairV2IngressWireKey>,
    encoded_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FairV2IngressWireKey {
    origin: Option<PeerId>,
    hash: CryptoHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressClass {
    Auxiliary,
    Progress,
    TransportCompletion,
}

impl FairV2IngressClass {
    fn classify(inbound: &InboundBlockMessage) -> Self {
        let BlockMessage::V2(message) = inbound.message() else {
            return match inbound.message() {
                BlockMessage::LaneExecutablePayload(_)
                | BlockMessage::LaneExecutablePayloadHandoff(_) => Self::TransportCompletion,
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
            | ConsensusMessageV2Payload::CommitCertificateResponse(_) => Self::Progress,
            ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_) => Self::TransportCompletion,
            ConsensusMessageV2Payload::Proposal(_)
            | ConsensusMessageV2Payload::Vote(_)
            | ConsensusMessageV2Payload::PayloadManifest(_) => Self::Auxiliary,
        }
    }
}

fn fair_v2_ingress_is_timeout_vote(inbound: &InboundBlockMessage) -> bool {
    matches!(
        inbound.message(),
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
    Bytes,
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

fn fair_v2_ingress_required_capacity(roster_len: usize) -> Option<usize> {
    if roster_len == 0 {
        return Some(1);
    }
    roster_len
        .checked_mul(4)
        .and_then(|required| required.checked_add(2))
}

const fn fair_v2_ingress_lane_protected_slots(
    is_validator: bool,
    reserve_untrusted_completion: bool,
    depth: usize,
    has_non_timeout_progress: bool,
    has_timeout_vote: bool,
    has_transport_completion: bool,
) -> usize {
    if !is_validator {
        if !reserve_untrusted_completion {
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
    let missing_non_timeout_progress = if has_non_timeout_progress { 0 } else { 1 };
    let missing_timeout_vote = if has_timeout_vote { 0 } else { 1 };
    let missing_transport_completion = if has_transport_completion { 0 } else { 1 };
    let missing_classes =
        missing_non_timeout_progress + missing_timeout_vote + missing_transport_completion;
    let missing_generic_slots = 4_usize.saturating_sub(depth);
    if missing_generic_slots > missing_classes {
        missing_generic_slots
    } else {
        missing_classes
    }
}

fn fair_v2_ingress_required_byte_capacity(
    roster_len: usize,
    source_byte_capacity: usize,
) -> Option<usize> {
    roster_len
        .checked_add(1)
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
        .checked_add(5)?
        .checked_add(102)?
        .checked_add(fair_v2_ingress_framed_bytes(172)?)?
        .checked_add(fair_v2_ingress_framed_bytes(signer_vector_bytes)?)?
        .checked_add(fair_v2_ingress_framed_bytes(signature_vector_bytes)?)
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

        // QuorumCertificate = Round + phase + Subject + ExecutionCommitment
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
    let align = core::mem::align_of::<norito::core::Archived<BlockMessage>>();
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

#[derive(Debug)]
enum FairV2IngressPushError {
    Closed(InboundBlockMessage),
    Full(InboundBlockMessage),
    Rejected(InboundBlockMessage),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressPushDisposition {
    Enqueued,
    Coalesced,
}

/// Fixed-capacity, roster-aware v2 ingress with per-hop admission and service fairness.
///
/// Every authenticated validator hop owns one protected source slot, one non-timeout
/// progress slot, one distinct TimeoutVote slot, and one transport-completion
/// slot. The shared untrusted lane owns one generic slot plus a distinct
/// transport-completion slot whenever the frozen roster is non-empty.
/// Anonymous and non-roster traffic shares one untrusted lane and cannot spend
/// a validator's completion reservation. A roster-origin completion forwarded
/// by a trusted non-validator relay stays in that relay's untrusted lane and
/// spends only the untrusted lane's completion reservation. Exact wire retransmissions coalesce
/// only while the same semantic origin still owns an identical queued envelope;
/// after service, a later retransmission is admitted normally. Distinct
/// semantic origins relayed by one hop share that hop's finite owners. A
/// distinct response through the same validator retries after fair service
/// releases that validator hop's sole completion owner.
///
/// Non-empty lanes are serviced in round-robin order, so a source may use
/// otherwise idle capacity but cannot starve an honest validator's progress.
/// Canonical envelope hashes are computed before taking the shared queue lock,
/// so duplicate detection never compares whole bodies while holding that lock.
/// Canonical wire bytes are charged to fixed aggregate and per-source budgets.
/// Within each validator partition, ordinary traffic, TimeoutVote, and the
/// payload transport completion own disjoint byte regions. The untrusted
/// partition likewise separates ordinary and transport-completion bytes.
/// Lane-local control
/// and atomic certificate recovery share the progress reservation; exact
/// executable-payload and proposer-handoff bytes share the completion
/// reservation. Roster
/// installation succeeds only when every validator and the shared untrusted
/// lane own an isolated byte partition. `CommitCertificateResponse` remains
/// reducer-producing Progress and cannot use the transport-completion slot or
/// bytes.
pub(crate) struct FairV2Ingress {
    capacity: usize,
    byte_capacity: usize,
    source_byte_capacity: usize,
    timeout_vote_byte_reserve: usize,
    transport_completion_byte_reserve: usize,
    consensus_frame_byte_capacity: usize,
    control_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    outbound_high_frame_byte_capacity: usize,
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
        let mut lanes = BTreeMap::new();
        lanes.insert(FairV2IngressSource::Untrusted, FairV2IngressLane::default());
        Self {
            capacity,
            byte_capacity,
            source_byte_capacity,
            timeout_vote_byte_reserve,
            transport_completion_byte_reserve,
            consensus_frame_byte_capacity,
            control_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            outbound_high_frame_byte_capacity,
            state: Mutex::new(FairV2IngressState {
                roster: BTreeSet::new(),
                lanes,
                ready: VecDeque::new(),
                len: 0,
                bytes: 0,
                nonempty_since: None,
                last_service_attempt_at: None,
                required_ordinary_bytes: 0,
                required_transport_completion_bytes: 0,
                required_consensus_frame_bytes: 0,
                required_control_frame_bytes: 0,
                required_block_sync_frame_bytes: 0,
                required_outbound_high_frame_bytes: 0,
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

            for (source, lane) in &state.lanes {
                debug_assert!(lane.bytes <= self.source_byte_capacity);
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
                        .timeout_vote_bytes
                        .checked_add(lane.transport_completion_bytes)
                        .expect("per-source byte ownership remains bounded");
                    debug_assert!(lane.bytes.checked_sub(reserved_bytes).is_some_and(
                        |ordinary_bytes| {
                            ordinary_bytes
                                <= self
                                    .source_byte_capacity
                                    .saturating_sub(self.timeout_vote_byte_reserve)
                                    .saturating_sub(self.transport_completion_byte_reserve)
                        }
                    ));
                } else {
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
                        .checked_add(self.timeout_vote_byte_reserve)
                        .and_then(|reserved| {
                            reserved.checked_add(self.transport_completion_byte_reserve)
                        })
                        .is_some_and(|reserved| reserved <= self.source_byte_capacity)
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
                let protected = state
                    .lanes
                    .iter()
                    .map(|(source, lane)| {
                        let is_validator = matches!(source, FairV2IngressSource::Validator(_));
                        let has_non_timeout_progress = lane.progress_len > lane.timeout_vote_len;
                        let has_timeout_vote = lane.timeout_vote_len != 0;
                        let has_transport_completion = lane.transport_completion_len != 0;
                        fair_v2_ingress_lane_protected_slots(
                            is_validator,
                            !state.roster.is_empty(),
                            lane.entries.len(),
                            has_non_timeout_progress,
                            has_timeout_vote,
                            has_transport_completion,
                        )
                    })
                    .sum::<usize>();
                debug_assert!(
                    state
                        .len
                        .checked_add(protected)
                        .is_some_and(|owned| owned <= self.capacity)
                );
            }
        }
    }

    /// Close admission and atomically install the next height's frozen roster.
    ///
    /// Queued messages belong to the preceding immutable height and are
    /// discarded while the public ingress gate is closed. The caller may open
    /// the queue only after context and safety-WAL recovery complete.
    #[cfg(test)]
    pub(crate) fn configure_roster(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
    ) -> Result<(), FairV2IngressCapacityError> {
        self.configure_roster_with_byte_requirements(roster, 0, 0, 0, 0, 0, 0)
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
        let required_control_message_bytes =
            required_proposal_bytes.max(required_commit_certificate_response_bytes);
        let required_transport_completion_bytes =
            fair_v2_ingress_required_transport_completion_bytes(layout)
                .max(MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES);
        let required_recovery_request_bytes =
            fair_v2_ingress_required_recovery_request_bytes(chain_id, roster.len());
        let required_lane_progress_frame_bytes =
            fair_v2_ingress_required_lane_p2p_frame_bytes(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);
        let required_lane_completion_frame_bytes =
            fair_v2_ingress_required_lane_p2p_frame_bytes(MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES);
        let required_consensus_frame_bytes =
            fair_v2_ingress_required_p2p_frame_bytes(required_recovery_request_bytes)
                .max(required_lane_progress_frame_bytes);
        let required_control_frame_bytes =
            fair_v2_ingress_required_p2p_frame_bytes(required_control_message_bytes);
        let required_block_sync_frame_bytes = fair_v2_ingress_required_p2p_frame_bytes(
            fair_v2_ingress_required_transport_completion_bytes(layout),
        )
        .max(required_lane_completion_frame_bytes);
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
        self.configure_roster_with_byte_requirements(
            roster,
            BODY_ENVELOPE_HEADROOM_BYTES
                .max(required_control_message_bytes)
                .max(required_recovery_request_bytes)
                .max(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES),
            required_transport_completion_bytes,
            required_consensus_frame_bytes,
            required_control_frame_bytes,
            required_block_sync_frame_bytes,
            required_outbound_high_frame_bytes,
        )
    }

    fn configure_roster_with_byte_requirements(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
        required_ordinary_bytes: usize,
        required_transport_completion_bytes: usize,
        required_consensus_frame_bytes: usize,
        required_control_frame_bytes: usize,
        required_block_sync_frame_bytes: usize,
        required_outbound_high_frame_bytes: usize,
    ) -> Result<(), FairV2IngressCapacityError> {
        let roster = roster.into_iter().collect::<BTreeSet<_>>();
        let required = fair_v2_ingress_required_capacity(roster.len());
        let mut lanes = BTreeMap::new();
        for peer in &roster {
            lanes.insert(
                FairV2IngressSource::Validator(peer.clone()),
                FairV2IngressLane::default(),
            );
        }
        lanes.insert(FairV2IngressSource::Untrusted, FairV2IngressLane::default());
        let mut state = self.state.lock();
        state.open = false;
        state.roster = roster;
        state.lanes = lanes;
        state.ready.clear();
        state.len = 0;
        state.bytes = 0;
        state.nonempty_since = None;
        state.last_service_attempt_at = None;
        state.required_ordinary_bytes = required_ordinary_bytes;
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
        if self.timeout_vote_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.timeout_vote_byte_reserve,
                kind: FairV2IngressCapacityKind::TimeoutVoteBytes,
            });
        }
        let Some(required_reserved_bytes) = self
            .timeout_vote_byte_reserve
            .checked_add(self.transport_completion_byte_reserve)
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
        let Some(required_bytes) =
            fair_v2_ingress_required_byte_capacity(state.roster.len(), self.source_byte_capacity)
        else {
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

    /// Open admission for the already-configured immutable height.
    pub(crate) fn open(&self) -> Result<(), FairV2IngressCapacityError> {
        let mut state = self.state.lock();
        let Some(required) = fair_v2_ingress_required_capacity(state.roster.len()) else {
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
        if self.timeout_vote_byte_reserve > self.source_byte_capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.source_byte_capacity,
                required: self.timeout_vote_byte_reserve,
                kind: FairV2IngressCapacityKind::TimeoutVoteBytes,
            });
        }
        let Some(required_reserved_bytes) = self
            .timeout_vote_byte_reserve
            .checked_add(self.transport_completion_byte_reserve)
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
        let Some(required_bytes) =
            fair_v2_ingress_required_byte_capacity(state.roster.len(), self.source_byte_capacity)
        else {
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

    fn try_push(
        &self,
        inbound: InboundBlockMessage,
    ) -> Result<FairV2IngressPushDisposition, FairV2IngressPushError> {
        self.try_push_at(inbound, Instant::now())
    }

    fn try_push_at(
        &self,
        inbound: InboundBlockMessage,
        enqueued_at: Instant,
    ) -> Result<FairV2IngressPushDisposition, FairV2IngressPushError> {
        let class = FairV2IngressClass::classify(&inbound);
        let is_timeout_vote = fair_v2_ingress_is_timeout_vote(&inbound);
        let is_transport_completion = class == FairV2IngressClass::TransportCompletion;
        let (wire_hash, encoded_len) = match inbound.message() {
            BlockMessage::V2(message) => {
                let encoded = message.encode();
                let encoded_len = encoded.len();
                (Some(CryptoHash::new(encoded)), encoded_len)
            }
            message if message.is_lane_local() => {
                let encoded = message.encode();
                let encoded_len = encoded.len();
                let lane_limit = if class == FairV2IngressClass::TransportCompletion {
                    MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES
                } else {
                    MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES
                };
                if encoded_len > lane_limit {
                    return Err(FairV2IngressPushError::Rejected(inbound));
                }
                (Some(CryptoHash::new(encoded)), encoded_len)
            }
            _ => return Err(FairV2IngressPushError::Rejected(inbound)),
        };
        // Delivery deduplication remains scoped to the semantic origin: two
        // requesters behind one trusted relay can require distinct responses.
        // Count, byte, and fair-service ownership below is instead charged to
        // the authenticated hop so origin churn cannot multiply resources.
        let wire_key = wire_hash.map(|hash| FairV2IngressWireKey {
            origin: inbound.sender.clone(),
            hash,
        });
        let mut state = self.state.lock();
        if !state.open {
            return Err(FairV2IngressPushError::Closed(inbound));
        }
        let source = inbound
            .via()
            .filter(|peer| state.roster.contains(*peer))
            .cloned()
            .map_or(
                FairV2IngressSource::Untrusted,
                FairV2IngressSource::Validator,
            );
        let lane = state
            .lanes
            .get(&source)
            .expect("configured fair ingress always contains the classified source lane");
        if wire_key
            .as_ref()
            .is_some_and(|key| lane.pending_wire.contains(key))
        {
            return Ok(FairV2IngressPushDisposition::Coalesced);
        }
        let is_validator_source = matches!(source, FairV2IngressSource::Validator(_));
        let is_validator_origin = inbound
            .sender()
            .is_some_and(|peer| state.roster.contains(peer));
        // Transport completions are protocol-valid only for a frozen-roster
        // semantic origin. Their finite queue and byte owners belong to the
        // authenticated hop's lane: a trusted non-validator relay therefore
        // spends the untrusted lane's reserve and cannot borrow a validator's.
        if is_transport_completion && !is_validator_origin {
            return Err(FairV2IngressPushError::Rejected(inbound));
        }
        // A validator lane has one critical TimeoutVote byte owner. Exact
        // retransmissions were coalesced above; a distinct later-view vote is
        // retried by retained control after fair service releases the owner.
        // This keeps the runtime byte abstraction equal to the formal gate
        // instead of allowing several votes to consume one logical reserve.
        if is_validator_source && is_timeout_vote && lane.timeout_vote_len != 0 {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        if is_validator_source && is_timeout_vote && encoded_len > self.timeout_vote_byte_reserve {
            return Err(FairV2IngressPushError::Rejected(inbound));
        }
        // A validator also has one source-isolated payload-completion owner.
        // Exact retransmissions coalesced above; distinct, conflicting, or
        // replayed responses retry after fair service releases the owner.
        if is_transport_completion && lane.transport_completion_len != 0 {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        if is_transport_completion && encoded_len > self.transport_completion_byte_reserve {
            return Err(FairV2IngressPushError::Rejected(inbound));
        }
        let (owned_class_bytes, source_class_byte_limit) = if is_validator_source && is_timeout_vote
        {
            (lane.timeout_vote_bytes, self.timeout_vote_byte_reserve)
        } else if is_transport_completion {
            (
                lane.transport_completion_bytes,
                self.transport_completion_byte_reserve,
            )
        } else if is_validator_source {
            let reserved_bytes = lane
                .timeout_vote_bytes
                .checked_add(lane.transport_completion_bytes)
                .expect("configured per-source byte limit prevents overflow");
            (
                lane.bytes
                    .checked_sub(reserved_bytes)
                    .expect("reserved byte owners are included in the source total"),
                self.source_byte_capacity
                    .saturating_sub(self.timeout_vote_byte_reserve)
                    .saturating_sub(self.transport_completion_byte_reserve),
            )
        } else {
            (
                lane.bytes
                    .checked_sub(lane.transport_completion_bytes)
                    .expect("untrusted completion bytes are included in the source total"),
                self.source_byte_capacity
                    .saturating_sub(self.transport_completion_byte_reserve),
            )
        };
        if encoded_len > source_class_byte_limit || encoded_len > self.byte_capacity {
            return Err(FairV2IngressPushError::Rejected(inbound));
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
                let is_validator = matches!(lane_source, FairV2IngressSource::Validator(_));
                let projected_timeout_vote_len =
                    lane.timeout_vote_len + usize::from(is_target && is_timeout_vote);
                let projected_transport_completion_len = lane.transport_completion_len
                    + usize::from(is_target && is_transport_completion);
                let projected_non_timeout_progress_len =
                    lane.progress_len.saturating_sub(lane.timeout_vote_len)
                        + usize::from(
                            is_target && class == FairV2IngressClass::Progress && !is_timeout_vote,
                        );
                fair_v2_ingress_lane_protected_slots(
                    is_validator,
                    !state.roster.is_empty(),
                    projected_len,
                    projected_non_timeout_progress_len != 0,
                    projected_timeout_vote_len != 0,
                    projected_transport_completion_len != 0,
                )
            })
            .sum::<usize>();
        let usable_capacity = self
            .capacity
            .saturating_sub(protected_slots_after_admission);
        if state.len >= usable_capacity {
            return Err(FairV2IngressPushError::Full(inbound));
        }
        let queue_was_empty = state.len == 0;
        let lane = state
            .lanes
            .get_mut(&source)
            .expect("configured fair ingress always contains the classified source lane");
        let was_empty = lane.entries.is_empty();
        if class == FairV2IngressClass::Progress {
            lane.progress_len += 1;
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
            inbound,
            enqueued_at,
            class,
            wire_key,
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
    /// The predicate executes while the ingress state is locked. For every
    /// ready source, the method selects its oldest currently admissible entry.
    /// Earlier blocked entries remain in place, and the source still consumes
    /// only one round-robin turn. This lets a proposal, certificate, body
    /// response, or payload chunk bypass an auxiliary request waiting for I/O
    /// capacity without dropping or duplicating that request. Once the blocked
    /// entry becomes admissible, the head-first search selects it before later
    /// entries. When every entry is rejected, one complete source rotation
    /// restores the original source order and total length.
    pub(crate) fn try_recv_if(
        &self,
        predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Option<InboundBlockMessage> {
        self.try_recv_if_at(Instant::now(), predicate)
    }

    fn try_recv_if_at(
        &self,
        service_attempt_at: Instant,
        mut predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Option<InboundBlockMessage> {
        let mut state = self.state.lock();
        if state.len != 0 {
            // A rejected scan is still proof that the outer runner scheduler
            // reached this queue. Downstream admission owns any remaining
            // delay; queue age alone does not establish scheduler starvation.
            state.last_service_attempt_at = Some(service_attempt_at);
        }
        let ready_sources = state.ready.len();
        for _ in 0..ready_sources {
            let source = state
                .ready
                .pop_front()
                .expect("snapshotted ready source must remain queued");
            let admitted_index = state.lanes.get(&source).and_then(|lane| {
                lane.entries
                    .iter()
                    .position(|entry| predicate(&entry.inbound))
            });
            let Some(admitted_index) = admitted_index else {
                state.ready.push_back(source);
                continue;
            };
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
            state.len -= 1;
            state.bytes = state
                .bytes
                .checked_sub(entry.encoded_len)
                .expect("aggregate byte ownership includes every queued entry");
            if state.len == 0 {
                state.nonempty_since = None;
                state.last_service_attempt_at = None;
            }
            if remains_ready {
                state.ready.push_back(source);
            }
            self.debug_assert_consistent(&state);
            return Some(entry.inbound);
        }
        None
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

/// Bounded ingress handle for the serialized Sumeragi v2 runner.
///
/// Global v1 frames are decode-only and are rejected before any queue handoff.
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

    /// Try to transfer one exact normalized block envelope to the serialized owner.
    pub fn try_incoming_block_message_owned(
        &self,
        inbound: InboundBlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        let Some(_permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(inbound);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            iroha_logger::debug!(
                "deferring Sumeragi ingress until context and safety WAL replay complete"
            );
            return SumeragiIngressDisposition::Retry(inbound);
        }

        if matches!(inbound.message(), BlockMessage::V2(_)) || inbound.message().is_lane_local() {
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
                Err(FairV2IngressPushError::Rejected(inbound)) => {
                    iroha_logger::warn!(?queue, "permanently rejected Sumeragi ingress envelope");
                    SumeragiIngressDisposition::Rejected(inbound)
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

    /// Enqueue a canonical v2 or retained lane-local message without blocking.
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

    /// Reject a decode-only lane-drain vote while the first-release collector is retired.
    pub fn try_incoming_lane_drain_vote(
        &self,
        sender: PeerId,
        vote: crate::lane_consensus::LaneDrainVoteV1,
    ) -> bool {
        let _ = (sender, vote);
        iroha_logger::debug!(
            "rejecting decode-only lane-drain vote: the first-release collector is retired"
        );
        false
    }

    /// Try to enqueue authenticated certified merge-sidecar traffic.
    pub fn try_incoming_certified_merge_sidecar(
        &self,
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::CertifiedMergeSidecar {
            sender,
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
        self.try_incoming_lane_relay_owned(LaneRelayMessage::NativeAmx { sender, message })
            .accepted_or_coalesced()
    }

    /// Return whether a fatal consensus failure requires process restart.
    #[must_use]
    pub fn restart_required(&self) -> bool {
        self.output_guard.restart_required()
    }
}

#[cfg(test)]
fn test_sumeragi_handle(
    block_capacity: usize,
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
    let block = Arc::new(FairV2Ingress::new(
        block_capacity,
        TEST_AGGREGATE_BYTE_CAPACITY,
        TEST_SOURCE_BYTE_CAPACITY,
        TIMEOUT_VOTE_RESERVE_BYTES,
        TEST_TRANSPORT_COMPLETION_BYTE_RESERVE,
    ));
    block
        .configure_roster(std::iter::empty())
        .expect("test untrusted lane fits configured capacity");
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
            .checked_sub(TIMEOUT_VOTE_RESERVE_BYTES)
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
        let block = Arc::new(FairV2Ingress::new_with_transport_frame_caps(
            block_channel_cap,
            block_byte_cap,
            block_source_byte_cap,
            TIMEOUT_VOTE_RESERVE_BYTES,
            transport_completion_byte_reserve,
            consensus_frame_byte_capacity,
            control_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            outbound_frame_queue_max_high_bytes,
        ));
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
            network,
            genesis_network,
            lane_relay_rx,
            ingress_ready,
            output_guard: Arc::clone(&output_guard),
            block_rx: block,
            wake_rx,
            shutdown_signal,
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
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    fn fail_spawn(
        _builder: std::thread::Builder,
        _work: SumeragiThreadWork,
    ) -> std::io::Result<SumeragiThreadCompletion> {
        Err(std::io::Error::other("injected synchronous spawn failure"))
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
}

struct SumeragiWorker {
    config: SumeragiConfig,
    common_config: CommonConfig,
    events_sender: EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    network: IrohaNetwork,
    genesis_network: GenesisWithPubKey,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
    block_rx: Arc<FairV2Ingress>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
}

#[cfg(test)]
mod authoritative_runtime_gate_tests {
    use std::{
        collections::BTreeSet,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId,
        block::{
            consensus::{
                CertPhase, LaneBlockCertificateV1, LaneBlockDescriptorV1, LaneBlockProposalV1,
                LaneBlockQcV1,
            },
            consensus_v2 as wire,
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1, MergeCommitteeSignature},
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
    };
    use norito::codec::Encode as _;

    use super::{BlockMessage, FairV2IngressClass, InboundBlockMessage, test_sumeragi_handle};

    fn v2_message_with_bytes(index: u32, byte_len: usize) -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-ingress-test")),
                index,
                bytes: vec![0xA5; byte_len],
                sender: 0,
                signature: vec![0x5A],
            }),
        ))
    }

    fn v2_message_with_index(index: u32) -> BlockMessage {
        v2_message_with_bytes(index, 1)
    }

    fn v2_message() -> BlockMessage {
        v2_auxiliary_prepare(0)
    }

    fn lane_block_certificate(seed: u8) -> BlockMessage {
        let validator = PeerId::new(
            KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
                .expect("derive lane-certificate validator")
                .public_key()
                .clone(),
        );
        let validator_set = vec![validator];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(9),
            lane_incarnation: Hash::new(b"fair-ingress-lane-incarnation"),
            proposal_height: 1,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"fair-ingress-lane-subject"),
            payload_ownership_hash: Hash::new(b"fair-ingress-lane-ownership"),
            rbc_instance_hash: Hash::new(b"fair-ingress-lane-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"fair-ingress-lane-tx")],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:fair-ingress-lane".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let qc = |phase| LaneBlockQcV1 {
            body: proposal.vote_body(phase),
            validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
            validator_set_hash: proposal.descriptor.validator_set_hash,
            validator_set: proposal.descriptor.validator_set.clone(),
            signers_bitmap: vec![1],
            bls_aggregate_signature: vec![seed; 96],
            payload_availability_qc: None,
        };
        let prepare_qc = qc(CertPhase::Prepare);
        let commit_qc = qc(CertPhase::Commit);
        BlockMessage::LaneBlockCertificate(Box::new(LaneBlockCertificateV1 {
            proposal,
            prepare_qc,
            commit_qc,
        }))
    }

    fn v2_commit_certificate_request(index: u64, requester: &PeerId) -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                wire::CommitCertificateRequest {
                    protocol_version: wire::PROTOCOL_VERSION,
                    chain_id: ChainId::from("fair-v2-ingress-test"),
                    context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                        b"fair-v2-ingress-context",
                    ))),
                    height: index.saturating_add(1),
                    requester: requester.clone(),
                    signature: vec![u8::try_from(index).unwrap_or(u8::MAX)],
                },
            ),
        ))
    }

    fn v2_certified_body_request(requester: &PeerId) -> BlockMessage {
        let BlockMessage::V2(message) = v2_vote(wire::GlobalPhase::Prepare) else {
            unreachable!("v2 vote fixture always returns a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = message.payload else {
            unreachable!("v2 vote fixture always carries a vote");
        };
        let certificate = wire::QuorumCertificate {
            round: vote.round,
            phase: wire::GlobalPhase::Prepare,
            subject: vote.subject,
            execution_commitment: vote.execution_commitment,
            signers: vec![0],
            aggregate_signature: vec![0x5A],
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
                round: vote.round,
                subject: vote.subject,
                certificate,
                requester: requester.clone(),
                signature: vec![0x5A],
            }),
        ))
    }

    fn v2_certified_body_response(
        request_ordinal: u64,
        responder: wire::ValidatorIndex,
        body_len: usize,
    ) -> BlockMessage {
        let body = vec![u8::try_from(request_ordinal).unwrap_or(0xA5); body_len];
        let payload_hash = Hash::new(&body);
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-response-context",
            ))),
            height: 1,
            view: request_ordinal,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(&request_ordinal.to_le_bytes())),
            payload_hash,
        };
        let manifest = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: u64::try_from(body_len).expect("test body length fits u64"),
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: u32::try_from(body_len.max(1)).unwrap_or(u32::MAX),
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: u64::try_from(body_len.max(1))
                    .expect("test body bound fits u64"),
                max_chunk_count: 1,
            },
            chunk_hashes: vec![payload_hash],
            chunk_root: Hash::new(payload_hash.as_ref()),
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    &request_ordinal.to_le_bytes(),
                )),
                manifest,
                body,
                responder,
                signature: vec![0x5A],
            }),
        ))
    }

    fn v2_maximum_certified_body_response(layout: wire::DataAvailabilityLayout) -> BlockMessage {
        let body_len =
            usize::try_from(layout.max_payload_size_bytes).expect("test payload bound fits usize");
        let chunk_count =
            usize::try_from(layout.max_chunk_count).expect("test chunk count fits usize");
        let body = vec![0xA5; body_len];
        let chunk_hash = Hash::new(vec![
            0x5A;
            usize::try_from(layout.chunk_size_bytes)
                .expect("test chunk bound fits usize")
        ]);
        let chunk_hashes = vec![chunk_hash; chunk_count];
        let leaves = chunk_hashes
            .iter()
            .map(|hash| *hash.as_ref())
            .collect::<Vec<[u8; Hash::LENGTH]>>();
        let chunk_root =
            iroha_crypto::MerkleTree::<[u8; Hash::LENGTH]>::from_hashed_leaves_sha256(leaves)
                .root()
                .map(Hash::from)
                .expect("non-empty maximal manifest has a Merkle root");
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-response-context",
            ))),
            height: u64::MAX,
            view: u64::MAX,
        };
        let manifest = wire::PayloadManifest {
            round,
            subject: wire::BlockSubject {
                parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-max-response-parent",
                ))),
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-max-response-block",
                )),
                payload_hash: Hash::new(&body),
            },
            payload_size_bytes: layout.max_payload_size_bytes,
            layout,
            chunk_hashes,
            chunk_root,
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-max-response-request",
                )),
                manifest,
                body,
                responder: u32::MAX,
                signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ))
    }

    fn v2_maximum_payload_chunk(layout: wire::DataAvailabilityLayout) -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-max-chunk-manifest",
                )),
                index: u32::MAX,
                bytes: vec![
                    0xA5;
                    usize::try_from(layout.chunk_size_bytes)
                        .expect("test chunk bound fits usize")
                ],
                sender: u32::MAX,
                signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ))
    }

    fn v2_commit_certificate_response(request_ordinal: u64, responder: &PeerId) -> BlockMessage {
        let BlockMessage::V2(message) = v2_vote(wire::GlobalPhase::Commit) else {
            unreachable!("v2 vote fixture always returns a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = message.payload else {
            unreachable!("v2 vote fixture always carries a vote");
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                wire::CommitCertificateResponse {
                    request_hash: HashOf::from_untyped_unchecked(Hash::new(
                        &request_ordinal.to_le_bytes(),
                    )),
                    certificate: wire::QuorumCertificate {
                        round: vote.round,
                        phase: wire::GlobalPhase::Commit,
                        subject: vote.subject,
                        execution_commitment: vote.execution_commitment,
                        signers: vec![0],
                        aggregate_signature: vec![0x5A],
                    },
                    responder: responder.clone(),
                    signature: vec![0x5A],
                },
            ),
        ))
    }

    fn v2_vote(phase: wire::GlobalPhase) -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round: wire::ConsensusRound {
                    context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                        b"fair-v2-ingress-vote-context",
                    ))),
                    height: 1,
                    view: 0,
                },
                phase,
                subject: wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"fair-v2-ingress-vote-block",
                    )),
                    payload_hash: Hash::new(b"fair-v2-ingress-vote-payload"),
                },
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"fair-v2-ingress-parent-state"),
                    Hash::new(b"fair-v2-ingress-post-state"),
                    Hash::new(b"fair-v2-ingress-writes"),
                    Hash::new(b"fair-v2-ingress-executed-wire"),
                ),
                signer: 0,
                signature: vec![0x5A],
            }),
        ))
    }

    fn v2_auxiliary_prepare(index: u64) -> BlockMessage {
        let BlockMessage::V2(mut message) = v2_vote(wire::GlobalPhase::Prepare) else {
            unreachable!("v2 vote fixture always returns a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = &mut message.payload else {
            unreachable!("v2 vote fixture always carries a vote");
        };
        vote.round.height = index.saturating_add(1);
        vote.signature = vec![u8::try_from(index).unwrap_or(u8::MAX)];
        BlockMessage::V2(message)
    }

    fn v2_timeout_vote() -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round: wire::ConsensusRound {
                    context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                        b"fair-v2-ingress-timeout-context",
                    ))),
                    height: 1,
                    view: 0,
                },
                highest_prepare_qc: None,
                signer: 0,
                signature: vec![0x5A],
            }),
        ))
    }

    fn v2_maximum_valid_timeout_vote_wire() -> BlockMessage {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-timeout-context",
            ))),
            height: u64::MAX,
            view: u64::MAX,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-timeout-parent",
            ))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-timeout-block",
            )),
            payload_hash: Hash::new(b"fair-v2-ingress-max-timeout-payload"),
        };
        let ordinary_writes_root = Hash::new(b"fair-v2-ingress-max-writes");
        let topup_anchor_root = Hash::new(b"fair-v2-ingress-max-topup-root");
        let topup_anchor_count = wire::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK;
        let post_state_root = wire::ExecutionCommitment::topup_post_state_root(
            topup_anchor_count,
            ordinary_writes_root,
            topup_anchor_root,
        );
        let highest_prepare_qc = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: wire::ExecutionCommitment::new(
                Hash::new(b"fair-v2-ingress-max-parent-state"),
                post_state_root,
                ordinary_writes_root,
                Some(topup_anchor_root),
                topup_anchor_count,
                Hash::new(b"fair-v2-ingress-max-executed-wire"),
            )
            .expect("maximum top-up projection is canonical"),
            signers: (0..wire::MAX_VALIDATORS_PER_HEIGHT)
                .map(|index| u32::try_from(index).expect("validator bound fits u32"))
                .collect(),
            aggregate_signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round,
                highest_prepare_qc: Some(highest_prepare_qc),
                signer: u32::try_from(wire::MAX_VALIDATORS_PER_HEIGHT - 1)
                    .expect("validator bound fits u32"),
                signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ))
    }

    fn v2_maximum_structural_proposal_wire(
        layout: wire::DataAvailabilityLayout,
        roster_len: usize,
    ) -> BlockMessage {
        assert!(roster_len <= wire::MAX_VALIDATORS_PER_HEIGHT);
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-proposal-context",
        )));
        let subject = wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-proposal-parent",
            ))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-proposal-block",
            )),
            payload_hash: Hash::new(b"fair-v2-ingress-max-proposal-payload"),
        };
        let ordinary_writes_root = Hash::new(b"fair-v2-ingress-max-proposal-writes");
        let topup_anchor_root = Hash::new(b"fair-v2-ingress-max-proposal-topup-root");
        let topup_anchor_count = wire::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK;
        let post_state_root = wire::ExecutionCommitment::topup_post_state_root(
            topup_anchor_count,
            ordinary_writes_root,
            topup_anchor_root,
        );
        let execution_commitment = wire::ExecutionCommitment::new(
            Hash::new(b"fair-v2-ingress-max-proposal-parent-state"),
            post_state_root,
            ordinary_writes_root,
            Some(topup_anchor_root),
            topup_anchor_count,
            Hash::new(b"fair-v2-ingress-max-proposal-executed-wire"),
        )
        .expect("maximum top-up projection is canonical");
        let signers = (0..roster_len)
            .map(|index| u32::try_from(index).expect("validator bound fits u32"))
            .collect::<Vec<_>>();
        let prepare_qc = |view| wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id,
                height: u64::MAX,
                view,
            },
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers: signers.clone(),
            aggregate_signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        };
        let groups = (0..roster_len)
            .map(|index| wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare_qc(
                    u64::try_from(index).expect("validator bound fits view"),
                )),
                signers: vec![u32::try_from(index).expect("validator bound fits index")],
                aggregate_signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            })
            .collect::<Vec<_>>();
        let highest_prepare_qc = groups
            .last()
            .and_then(|group| group.highest_prepare_qc.clone());
        let timeout_view = u64::try_from(roster_len).expect("validator bound fits view");
        let proposal_round = wire::ConsensusRound {
            context_id,
            height: u64::MAX,
            view: timeout_view
                .checked_add(1)
                .expect("bounded view has successor"),
        };
        let chunk_count =
            usize::try_from(layout.max_chunk_count).expect("test chunk ceiling fits usize");
        let chunk_hashes = vec![Hash::new(b"fair-v2-ingress-max-proposal-chunk"); chunk_count];
        let manifest = wire::PayloadManifest {
            round: proposal_round,
            subject,
            payload_size_bytes: layout.max_payload_size_bytes,
            layout,
            chunk_hashes,
            chunk_root: Hash::new(b"fair-v2-ingress-max-proposal-root"),
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
                round: proposal_round,
                proposer: u32::try_from(roster_len.saturating_sub(1))
                    .expect("validator bound fits proposer index"),
                subject,
                manifest,
                justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                    timeout_certificate: wire::TimeoutCertificate {
                        round: wire::ConsensusRound {
                            context_id,
                            height: u64::MAX,
                            view: timeout_view,
                        },
                        groups,
                    },
                    highest_prepare_qc,
                }),
                signature: vec![0xC3; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ))
    }

    fn v2_maximum_recovery_wires(
        chain_id: &ChainId,
        requester: &PeerId,
        roster_len: usize,
    ) -> (BlockMessage, BlockMessage, BlockMessage) {
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let BlockMessage::V2(proposal_message) =
            v2_maximum_structural_proposal_wire(layout, roster_len)
        else {
            unreachable!("maximum proposal fixture is v2");
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal_message.payload else {
            unreachable!("maximum proposal fixture carries a proposal");
        };
        let wire::ProposalJustification::Timeout(justification) = proposal.justification else {
            unreachable!("maximum proposal fixture carries timeout justification");
        };
        let certificate = justification
            .highest_prepare_qc
            .expect("maximum proposal fixture carries its highest PrepareQC");
        let round = certificate.round;
        let subject = certificate.subject;
        let certified_body_request = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
                round,
                subject,
                certificate: certificate.clone(),
                requester: requester.clone(),
                signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ));
        let commit_certificate_request = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                wire::CommitCertificateRequest {
                    protocol_version: wire::PROTOCOL_VERSION,
                    chain_id: chain_id.clone(),
                    context_id: round.context_id,
                    height: u64::MAX,
                    requester: requester.clone(),
                    signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
                },
            ),
        ));
        let mut commit_certificate = certificate;
        commit_certificate.phase = wire::GlobalPhase::Commit;
        let commit_certificate_response = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                wire::CommitCertificateResponse {
                    request_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"fair-v2-ingress-max-recovery-request",
                    )),
                    certificate: commit_certificate,
                    responder: requester.clone(),
                    signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
                },
            ),
        ));
        (
            certified_body_request,
            commit_certificate_request,
            commit_certificate_response,
        )
    }

    fn vote_phase(inbound: &InboundBlockMessage) -> Option<wire::GlobalPhase> {
        let BlockMessage::V2(message) = inbound.message() else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = &message.payload else {
            return None;
        };
        Some(vote.phase)
    }

    fn vote_height(inbound: &InboundBlockMessage) -> Option<u64> {
        let BlockMessage::V2(message) = inbound.message() else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = &message.payload else {
            return None;
        };
        Some(vote.round.height)
    }

    fn payload_chunk_index(inbound: &InboundBlockMessage) -> Option<u32> {
        let BlockMessage::V2(message) = inbound.message() else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload else {
            return None;
        };
        Some(chunk.index)
    }

    fn encoded_v2_len(message: &BlockMessage) -> usize {
        let BlockMessage::V2(message) = message else {
            panic!("test fixture must be a v2 envelope");
        };
        message.encode().len()
    }

    #[test]
    fn ingress_stays_closed_until_replay_owner_acknowledges_ready() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(false, Ordering::Release);

        assert!(!handle.incoming_block_message(v2_message()));
        assert!(receiver.try_recv().is_none());

        handle.ingress_ready.store(true, Ordering::Release);
        assert!(handle.incoming_block_message(v2_message()));
        assert!(receiver.try_recv().is_some());
    }

    #[test]
    fn retired_global_v1_messages_never_enter_live_queues() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);

        assert!(!handle.incoming_block_message(BlockMessage::invalid_wire_sentinel()));
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn first_release_vrf_frames_are_decode_only_and_never_enter_live_queues() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);
        let commit = BlockMessage::VrfCommit(super::consensus::VrfCommit {
            epoch: 4,
            commitment: [0xA5; 32],
            signer: 0,
            bls_sig: vec![0x5A],
        });
        let reveal = BlockMessage::VrfReveal(super::consensus::VrfReveal {
            epoch: 4,
            reveal: [0xA6; 32],
            signer: 0,
            bls_sig: vec![0x5B],
        });

        assert!(!handle.incoming_block_message(commit));
        assert!(!handle.incoming_block_message(reveal));
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn first_release_lane_drain_votes_never_enter_the_live_relay_queue() {
        let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);
        let signer = PeerId::new(KeyPair::random().public_key().clone());
        let validator_set = vec![signer.clone()];
        let vote = crate::lane_consensus::LaneDrainVoteV1 {
            body: LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    chain_id_digest: Hash::new(b"retired-drain-ingress-chain"),
                    lane_id: LaneId::new(7),
                    dataspace_id: DataSpaceId::new(9),
                    lane_incarnation: Hash::new(b"retired-drain-ingress-incarnation"),
                    close_global_height: 3,
                    initial_merged_lane_height: 0,
                    initial_merged_descriptor_hash: None,
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count: 1,
                    min_quorum: 1,
                },
                final_lane_block_height: 0,
                final_lane_block_descriptor_hash: None,
            },
            signer: signer.clone(),
            proof_of_possession: vec![0xA5],
            bls_signature: vec![0x5A],
        };

        assert!(!handle.try_incoming_lane_drain_vote(signer, vote));
        assert!(relay_receiver.try_recv().is_err());
    }

    #[test]
    fn v2_ingress_is_bounded_and_never_blocks_a_network_caller() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);

        assert!(handle.incoming_block_message(v2_message()));
        assert!(
            !handle.incoming_block_message(v2_auxiliary_prepare(1)),
            "a distinct message at saturated capacity must reject promptly and rely on retransmission"
        );
        let _ = receiver.try_recv().expect("drain the bounded v2 queue");
        assert!(handle.incoming_block_message(v2_message()));
    }

    #[test]
    fn saturated_v2_ingress_returns_the_exact_owned_message_for_retry() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        let sender = validator_peers(1).pop().expect("sender fixture");

        assert!(handle.try_incoming_block_message(v2_message()));
        let retry =
            handle.try_incoming_block_message_from_owned(sender.clone(), v2_auxiliary_prepare(1));
        let super::SumeragiIngressDisposition::Retry(inbound) = retry else {
            panic!("saturated ingress must return caller ownership");
        };
        assert_eq!(inbound.sender(), Some(&sender));
        assert_eq!(vote_height(&inbound), Some(2));

        let _ = receiver
            .try_recv()
            .expect("release bounded ingress capacity");
        assert!(matches!(
            handle.try_incoming_block_message_owned(inbound),
            super::SumeragiIngressDisposition::Accepted
        ));
    }

    #[test]
    fn direct_and_synthetic_envelopes_keep_identity_roles_consistent() {
        let sender = validator_peers(1).pop().expect("sender fixture");
        let direct = InboundBlockMessage::new(v2_message(), Some(sender.clone()));
        assert_eq!(direct.sender(), Some(&sender));
        assert_eq!(direct.via(), Some(&sender));

        let synthetic = InboundBlockMessage::new(v2_message(), None);
        assert!(synthetic.sender().is_none());
        assert!(synthetic.via().is_none());
    }

    #[test]
    fn atomic_lane_certificate_uses_the_shared_progress_owner() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
        let certificate = lane_block_certificate(71);
        let expected = certificate.encode();

        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::new(certificate.clone(), None,)),
            FairV2IngressClass::Progress
        );
        assert!(matches!(
            handle.try_incoming_block_message_owned(InboundBlockMessage::new(certificate, None)),
            super::SumeragiIngressDisposition::Accepted
        ));
        let retained = ingress
            .try_recv()
            .expect("shared fair ingress retains the lane certificate");
        assert_eq!(retained.message().encode(), expected);
    }

    #[test]
    fn oversized_atomic_lane_certificate_is_returned_exactly() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
        let mut certificate = lane_block_certificate(72);
        let BlockMessage::LaneBlockCertificate(envelope) = &mut certificate else {
            unreachable!("fixture is an atomic lane certificate")
        };
        envelope.commit_qc.bls_aggregate_signature =
            vec![0xA5; super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES];
        let expected = certificate.encode();
        assert!(expected.len() > super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);

        let disposition =
            handle.try_incoming_block_message_owned(InboundBlockMessage::new(certificate, None));
        let super::SumeragiIngressDisposition::Rejected(retained) = disposition else {
            panic!("oversized lane certificate must be rejected with exact ownership")
        };
        assert_eq!(retained.message().encode(), expected);
        assert!(ingress.try_recv().is_none());
    }

    #[test]
    fn saturated_lane_ingress_returns_the_exact_owned_message_for_retry() {
        let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
        let first = MergeCommitteeSignature {
            epoch_id: 7,
            view: 1,
            signer: 0,
            message_digest: Hash::new(b"first retained lane item"),
            bls_sig: vec![0xA5],
        };
        let second = MergeCommitteeSignature {
            epoch_id: 7,
            view: 2,
            signer: 0,
            message_digest: Hash::new(b"second retained lane item"),
            bls_sig: vec![0x5A],
        };

        assert!(matches!(
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(first)),
            super::SumeragiIngressDisposition::Accepted
        ));
        let retry =
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(second));
        let super::SumeragiIngressDisposition::Retry(message) = retry else {
            panic!("saturated lane ingress must return caller ownership");
        };
        let super::LaneRelayMessage::MergeSignature(retained) = &message else {
            panic!("retry must preserve the exact lane message variant");
        };
        assert_eq!(retained.view, 2);
        assert_eq!(retained.bls_sig, vec![0x5A]);

        let _ = relay_receiver
            .try_recv()
            .expect("release bounded lane ingress capacity");
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(message),
            super::SumeragiIngressDisposition::Accepted
        ));
    }

    #[test]
    fn restart_required_ingress_rejects_before_queue_mutation() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.output_guard.activate_restart_required();

        assert!(handle.restart_required());
        assert!(!handle.incoming_block_message(v2_message()));
        assert!(
            receiver.try_recv().is_none(),
            "restart-required admission must not mutate the bounded ingress queue"
        );
    }

    fn validator_peers(count: u8) -> Vec<PeerId> {
        (0..count)
            .map(|seed| {
                PeerId::new(
                    KeyPair::try_from_seed(
                        vec![seed.saturating_add(1); 32],
                        iroha_crypto::Algorithm::Ed25519,
                    )
                    .expect("derive deterministic ingress peer")
                    .public_key()
                    .clone(),
                )
            })
            .collect()
    }

    #[test]
    fn byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(19);
        let validators = validator_peers(4);
        let attacker = validators[0].clone();
        let outsider = validator_peers(5).pop().expect("outsider fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        for index in 0..2 {
            assert!(
                handle.try_incoming_block_message_from(
                    attacker.clone(),
                    v2_auxiliary_prepare(index),
                )
            );
        }
        assert!(
            !handle.try_incoming_block_message_from(attacker.clone(), v2_auxiliary_prepare(2),),
            "attacker cannot consume ordinary, progress, or TimeoutVote slots reserved for empty validator lanes"
        );
        for honest in validators.iter().skip(1) {
            assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));
        }
        assert!(handle.try_incoming_block_message_from(outsider.clone(), v2_message()));
        assert_eq!(ingress.len(), 6);

        let first_cycle = (0..5)
            .map(|_| {
                ingress
                    .try_recv()
                    .expect("one ready source per fair service turn")
                    .into_message_and_sender()
                    .1
            })
            .collect::<Vec<_>>();
        assert_eq!(
            first_cycle,
            vec![
                Some(attacker),
                Some(validators[1].clone()),
                Some(validators[2].clone()),
                Some(validators[3].clone()),
                Some(outsider),
            ]
        );
        assert_eq!(ingress.len(), 1, "only the attacker's second item remains");
    }

    #[test]
    fn relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin() {
        const RELAYED_ORIGINS: usize = 32;
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(19);
        let validators = validator_peers(4);
        let via = validators[0].clone();
        let lane_origin = validators[1].clone();
        let origins = validator_peers(64)
            .into_iter()
            .skip(validators.len())
            .take(RELAYED_ORIGINS)
            .collect::<Vec<_>>();
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validator owners and the untrusted owner fit");
        ingress.open().expect("open configured roster");

        let mut accepted = 0_usize;
        for (index, origin) in origins.iter().enumerate() {
            let inbound = InboundBlockMessage::from_transport(
                v2_auxiliary_prepare(u64::try_from(index).expect("fixture index fits u64")),
                origin.clone(),
                via.clone(),
            );
            match handle.try_incoming_block_message_owned(inbound) {
                super::SumeragiIngressDisposition::Accepted => accepted += 1,
                super::SumeragiIngressDisposition::Retry(retained) => {
                    assert_eq!(retained.sender(), Some(origin));
                    assert_eq!(retained.via(), Some(&via));
                }
                disposition => panic!("unexpected relayed-origin disposition: {disposition:?}"),
            }
        }
        assert_eq!(
            accepted, 2,
            "semantic-origin churn must remain inside one validator lane instead of multiplying its reserved slots"
        );
        {
            let state = ingress.state.lock();
            let nonempty = state
                .lanes
                .iter()
                .filter(|(_, lane)| !lane.entries.is_empty())
                .map(|(source, _)| source.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                nonempty,
                vec![super::FairV2IngressSource::Validator(via.clone())]
            );
            assert_eq!(
                state.ready,
                std::collections::VecDeque::from([nonempty[0].clone()])
            );
        }

        assert!(
            handle.try_incoming_block_message_from(validators[2].clone(), v2_message()),
            "one relayed via cannot consume a responsive validator's reserved owner"
        );
        let first = ingress
            .try_recv()
            .expect("oldest relayed origin owns the via's first fair turn");
        assert_eq!(first.sender(), Some(&origins[0]));
        assert_eq!(first.via(), Some(&via));
        let responsive = ingress
            .try_recv()
            .expect("responsive validator follows after one via turn");
        assert_eq!(responsive.sender(), Some(&validators[2]));
        let second = ingress
            .try_recv()
            .expect("the via retains its second admitted origin");
        assert_eq!(second.sender(), Some(&origins[1]));
        assert!(ingress.try_recv().is_none());

        assert!(matches!(
            handle.try_incoming_block_message_owned(InboundBlockMessage::from_transport(
                lane_block_certificate(73),
                lane_origin.clone(),
                via.clone(),
            )),
            super::SumeragiIngressDisposition::Accepted
        ));
        let inbound = ingress
            .try_recv()
            .expect("relayed lane certificate reaches serialized validation");
        assert_eq!(inbound.sender(), Some(&lane_origin));
        assert_eq!(inbound.via(), Some(&via));
        let (message, sender) = inbound.into_message_and_sender();
        assert_eq!(sender, Some(lane_origin));
        assert!(matches!(message, BlockMessage::LaneBlockCertificate(_)));
    }

    #[test]
    fn roster_origin_relay_completion_has_untrusted_count_and_byte_owner() {
        const FORGED_OCCURRENCES: usize = 32;
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(18);
        let validators = validator_peers(4);
        let untrusted_via = validator_peers(5).pop().expect("untrusted via fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validator owners and the untrusted owner fit");
        ingress.open().expect("open configured roster");

        let mut accepted = 0_usize;
        for index in 0..FORGED_OCCURRENCES {
            let origin = &validators[index % validators.len()];
            let inbound = InboundBlockMessage::from_transport(
                v2_auxiliary_prepare(u64::try_from(index).expect("fixture index fits u64")),
                origin.clone(),
                untrusted_via.clone(),
            );
            match handle.try_incoming_block_message_owned(inbound) {
                super::SumeragiIngressDisposition::Accepted => accepted += 1,
                super::SumeragiIngressDisposition::Retry(retained) => {
                    assert_eq!(retained.sender(), Some(origin));
                    assert_eq!(retained.via(), Some(&untrusted_via));
                }
                disposition => panic!("unexpected forged-origin disposition: {disposition:?}"),
            }
        }
        assert_eq!(
            accepted, 1,
            "semantic roster identities must not consume the untrusted hop's reserved completion owner"
        );
        {
            let state = ingress.state.lock();
            let nonempty = state
                .lanes
                .iter()
                .filter(|(_, lane)| !lane.entries.is_empty())
                .map(|(source, _)| source.clone())
                .collect::<Vec<_>>();
            assert_eq!(nonempty, vec![super::FairV2IngressSource::Untrusted]);
            assert_eq!(
                state.ready,
                std::collections::VecDeque::from([super::FairV2IngressSource::Untrusted])
            );
            assert!(validators.iter().all(|validator| {
                state
                    .lanes
                    .get(&super::FairV2IngressSource::Validator(validator.clone()))
                    .is_some_and(|lane| lane.entries.is_empty())
            }));
        }

        let relayed_completion = InboundBlockMessage::from_transport(
            v2_message_with_index(0),
            validators[0].clone(),
            untrusted_via.clone(),
        );
        assert!(matches!(
            handle.try_incoming_block_message_owned(relayed_completion),
            super::SumeragiIngressDisposition::Accepted
        ));
        {
            let state = ingress.state.lock();
            assert_eq!(
                state
                    .lanes
                    .get(&super::FairV2IngressSource::Untrusted)
                    .expect("untrusted lane exists")
                    .transport_completion_len,
                1
            );
            assert_eq!(
                state
                    .lanes
                    .get(&super::FairV2IngressSource::Untrusted)
                    .expect("untrusted lane exists")
                    .entries
                    .len(),
                2,
                "ordinary relay pressure and its reserved completion coexist"
            );
            assert!(validators.iter().all(|validator| {
                state
                    .lanes
                    .get(&super::FairV2IngressSource::Validator(validator.clone()))
                    .is_some_and(|lane| lane.transport_completion_len == 0)
            }));
        }
        assert!(matches!(
            handle.try_incoming_block_message_owned(InboundBlockMessage::from_transport(
                v2_auxiliary_prepare(99),
                validators[1].clone(),
                untrusted_via.clone(),
            )),
            super::SumeragiIngressDisposition::Retry(_)
        ));
        let completion = ingress
            .try_recv_if(super::fair_v2_ingress_is_transport_completion)
            .expect("trusted-relay completion bypasses ordinary relay pressure");
        assert_eq!(completion.sender(), Some(&validators[0]));
        assert_eq!(completion.via(), Some(&untrusted_via));
        let ordinary = ingress
            .try_recv()
            .expect("the ordinary relay item remains after completion service");
        assert_eq!(ordinary.sender(), Some(&validators[0]));
        assert_eq!(ordinary.via(), Some(&untrusted_via));

        let outsider = validator_peers(6)
            .pop()
            .expect("non-roster semantic origin fixture");
        let outsider_completion =
            InboundBlockMessage::from_transport(v2_message_with_index(1), outsider, untrusted_via);
        assert!(matches!(
            handle.try_incoming_block_message_owned(outsider_completion),
            super::SumeragiIngressDisposition::Rejected(_)
        ));
        assert!(ingress.try_recv().is_none());
    }

    #[test]
    fn fair_v2_ingress_retains_ready_head_until_downstream_admission() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
        let validators = validator_peers(2);
        let attacker = validators[0].clone();
        let honest = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message_from(attacker.clone(), v2_message()));
        assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));

        let mut downstream_slots = 1_usize;
        let first = ingress
            .try_recv_if(|_| downstream_slots != 0)
            .expect("attacker consumes the initially available downstream slot");
        downstream_slots -= 1;
        assert_eq!(first.sender(), Some(&attacker));
        assert_eq!(ingress.len(), 1);

        assert!(ingress.try_recv_if(|_| downstream_slots != 0).is_none());
        assert_eq!(
            ingress.len(),
            1,
            "failed downstream admission must not remove the honest head"
        );

        downstream_slots += 1;
        let retained = ingress
            .try_recv_if(|_| downstream_slots != 0)
            .expect("honest head remains available after downstream service");
        assert_eq!(retained.sender(), Some(&honest));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_rotates_blocked_head_to_admissible_source() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
        let validators = validator_peers(2);
        let blocked = validators[0].clone();
        let admissible = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message_from(blocked.clone(), v2_message()));
        assert!(handle.try_incoming_block_message_from(admissible.clone(), v2_message()));

        let selected = ingress
            .try_recv_if(|inbound| inbound.sender() == Some(&admissible))
            .expect("later admissible source bypasses a blocked ready head");
        assert_eq!(selected.sender(), Some(&admissible));
        assert_eq!(ingress.len(), 1);

        let retained = ingress
            .try_recv_if(|_| true)
            .expect("blocked source remains queued after the bypass");
        assert_eq!(retained.sender(), Some(&blocked));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_bypasses_a_blocked_entry_within_the_same_source() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(0),)
        );
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(1),)
        );
        assert!(handle.try_incoming_block_message_from(validator, v2_auxiliary_prepare(2),));

        let selected = ingress
            .try_recv_if(|inbound| vote_height(inbound) == Some(3))
            .expect("admissible later item bypasses a blocked same-source head");
        assert_eq!(vote_height(&selected), Some(3));
        assert_eq!(ingress.len(), 2);

        let first_retained = ingress
            .try_recv_if(|_| true)
            .expect("oldest blocked entry remains owned for a later fair turn");
        assert_eq!(vote_height(&first_retained), Some(1));
        let second_retained = ingress
            .try_recv_if(|_| true)
            .expect("later blocked entry retains its relative order");
        assert_eq!(vote_height(&second_retained), Some(2));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_coalesces_only_a_pending_exact_source_retransmission() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(6);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");
        let request = v2_auxiliary_prepare(0);

        assert!(handle.try_incoming_block_message_from(validator.clone(), request.clone()));
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), request.clone()),
            "a retransmitter keeps ownership through the queued exact occurrence"
        );
        assert_eq!(ingress.len(), 1, "the exact pending wire value coalesces");
        assert!(
            !handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(1),),
            "a different auxiliary request cannot consume the progress reservation"
        );

        let delivered = ingress.try_recv().expect("deliver the queued occurrence");
        assert_eq!(delivered.sender(), Some(&validator));
        assert_eq!(ingress.len(), 0);
        assert!(handle.try_incoming_block_message_from(validator, request));
        assert_eq!(
            ingress.len(),
            1,
            "coalescing is queue-scoped and ends when the consumer takes ownership"
        );
    }

    #[test]
    fn fair_v2_ingress_wire_index_keeps_untrusted_origins_distinct() {
        let ingress = super::FairV2Ingress::new(3, 3 * 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("untrusted lane byte quota fits");
        ingress.open().expect("open configured roster");
        let outsiders = validator_peers(2);
        let message = v2_message();

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(outsiders[0].clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message,
                Some(outsiders[1].clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            ingress.len(),
            2,
            "the shared untrusted lane preserves distinct semantic request origins"
        );
    }

    #[test]
    fn fair_v2_ingress_byte_quota_isolates_validator_sources() {
        let validators = validator_peers(2);
        let first = v2_message_with_bytes(0, 64);
        let second = v2_message_with_bytes(1, 64);
        let encoded_len = encoded_v2_len(&first);
        assert_eq!(encoded_v2_len(&second), encoded_len);
        let source_capacity = encoded_len.checked_add(1).expect("ordinary byte partition");
        let ingress =
            super::FairV2Ingress::new(10, source_capacity * 3, source_capacity, 0, encoded_len);
        ingress
            .configure_roster(validators.clone())
            .expect("two validator and one untrusted byte partition fit exactly");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(first, Some(validators[0].clone()),)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                second,
                Some(validators[0].clone()),
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                v2_message_with_bytes(2, 64),
                Some(validators[1].clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            ingress.len(),
            2,
            "one validator's byte pressure cannot consume another validator's quota"
        );
    }

    #[test]
    fn fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let auxiliary = v2_auxiliary_prepare(0);
        let commit_response = v2_commit_certificate_response(0, &validator);
        let second_commit_response = v2_commit_certificate_response(1, &validator);
        let timeout = v2_timeout_vote();
        let body_response = v2_certified_body_response(0, 0, 64);
        let chunk = v2_message_with_bytes(0, 64);
        let ordinary_bytes = encoded_v2_len(&auxiliary)
            .checked_add(encoded_v2_len(&commit_response))
            .expect("ordinary fixture bytes fit usize");
        let timeout_bytes = encoded_v2_len(&timeout);
        let completion_bytes = encoded_v2_len(&body_response).max(encoded_v2_len(&chunk));
        let source_bytes = ordinary_bytes
            .checked_add(timeout_bytes)
            .and_then(|bytes| bytes.checked_add(completion_bytes))
            .expect("disjoint test partitions fit usize");
        let ingress = super::FairV2Ingress::new(
            6,
            2 * source_bytes,
            source_bytes,
            timeout_bytes,
            completion_bytes,
        );
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and untrusted partitions fit");
        ingress.open().expect("open configured roster");

        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::new(
                commit_response.clone(),
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
        for message in [auxiliary, commit_response, timeout] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::new(message, Some(validator.clone()))),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ));
        }
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    second_commit_response,
                    Some(validator.clone()),
                )),
                Err(super::FairV2IngressPushError::Full(_))
            ),
            "CommitCertificateResponse cannot spend the completion corridor"
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
            super::FairV2Ingress::new(10, 3 * source_bytes, source_bytes, 0, completion_bytes);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validators and untrusted source fit");
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
        let ingress = super::FairV2Ingress::new(10, 2 * 1024, 1024, 0, 0);
        let error = ingress
            .configure_roster(validators)
            .expect_err("two validators plus untrusted require three byte partitions");
        assert!(error.is_bytes());
        assert_eq!(error.configured(), 2 * 1024);
        assert_eq!(error.required(), 3 * 1024);
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
            super::FairV2Ingress::new(6, 2 * source_capacity, source_capacity, timeout_vote_len, 0);
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and untrusted byte partitions fit");
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
            super::FairV2Ingress::new(6, 2 * source_capacity, source_capacity, reserve, 0);
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and untrusted byte partitions fit");
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
            required_proposal, 232_541,
            "maximal proposal wire geometry is a regression boundary"
        );
        let proposal = v2_maximum_structural_proposal_wire(layout, wire::MAX_VALIDATORS_PER_HEIGHT);
        assert_eq!(
            encoded_v2_len(&proposal),
            required_proposal,
            "checked activation geometry must equal canonical bare Norito"
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
        let network_message = crate::NetworkMessage::SumeragiBlock(Box::new(
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
                network_message_bytes,
            ),
            "protocol-maximum identities must use the exact complete direct P2P wire"
        );
        assert!(required_control_frame >= exact_direct_frame);
        assert!(exact_direct_frame > exact_broadcast_frame);

        let minimal_layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let minimal_proposal_bytes =
            super::fair_v2_ingress_required_proposal_bytes(minimal_layout, 1);
        assert_eq!(minimal_proposal_bytes, 2_302);
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
            .checked_sub(super::TIMEOUT_VOTE_RESERVE_BYTES)
            .and_then(|bytes| bytes.checked_sub(ordinary_bytes))
            .expect("default source partition is disjoint");
        assert!(completion_bytes >= required);

        let global_plaintext = iroha_p2p::frame_plaintext_cap(
            iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
        );
        let ingress = super::FairV2Ingress::new_with_transport_frame_caps(
            18,
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get(),
            source_bytes,
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
        let network_response = crate::NetworkMessage::SumeragiBlock(Box::new(
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
                network_response.encoded_len(),
            ),
            "maximum completion must retain exact protocol-maximum direct-relay geometry"
        );
        assert!(protocol_maximum_response_frame >= actual_direct_response_frame);
        let chain_id = ChainId::from("fair-v2-ingress-test");
        let roster_len = 1;
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
            .checked_add(required - 1)
            .expect("test source bound fits usize");
        let short = super::FairV2Ingress::new(6, 2 * short_source, short_source, 0, required - 1);
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

        let ordinary_short_source = required
            .checked_add(ordinary_bytes - 1)
            .expect("test source bound fits usize");
        let ordinary_short = super::FairV2Ingress::new(
            6,
            2 * ordinary_short_source,
            ordinary_short_source,
            0,
            required,
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
            .checked_add(required)
            .expect("test source bound fits usize");
        let oversized_chain_id = ChainId::from(
            "x".repeat(
                ordinary_bytes
                    .checked_mul(2)
                    .expect("test chain-id length fits usize"),
            ),
        );
        let oversized_request_bytes =
            super::fair_v2_ingress_required_recovery_request_bytes(&oversized_chain_id, roster_len);
        assert!(oversized_request_bytes > ordinary_bytes);
        let oversized_chain_ingress =
            super::FairV2Ingress::new(6, 2 * source_bytes, source_bytes, 0, required);
        let oversized_chain_error = oversized_chain_ingress
            .configure_roster_for_context([validator.clone()], &oversized_chain_id, layout)
            .expect_err("an authenticated recovery request must fit its ordinary byte owner");
        assert_eq!(oversized_chain_error.configured(), ordinary_bytes);
        assert_eq!(oversized_chain_error.required(), oversized_request_bytes);

        let ingress_with_caps = |consensus, control, block_sync, outbound_high| {
            super::FairV2Ingress::new_with_transport_frame_caps(
                6,
                2 * source_bytes,
                source_bytes,
                0,
                required,
                consensus,
                control,
                block_sync,
                outbound_high,
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
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: u32::MAX,
            data_shards: 0,
            parity_shards: 0,
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

    #[test]
    fn fair_v2_ingress_capacity_arithmetic_overflow_fails_closed() {
        let largest_exact_roster = (usize::MAX - 2) / 4;
        assert!(super::fair_v2_ingress_required_capacity(largest_exact_roster).is_some());
        assert_eq!(
            super::fair_v2_ingress_required_capacity(largest_exact_roster + 1),
            None,
            "an unrepresentable validator-plus-relay ownership total must remain distinguishable from an exact usize::MAX capacity"
        );
        assert_eq!(
            super::fair_v2_ingress_required_byte_capacity(0, usize::MAX),
            Some(usize::MAX),
            "one exact usize::MAX source partition is representable"
        );
        assert_eq!(
            super::fair_v2_ingress_required_byte_capacity(1, usize::MAX),
            None,
            "two usize::MAX source partitions are not representable"
        );

        let exact_max = super::FairV2Ingress::new(1, usize::MAX, usize::MAX, 0, 0);
        exact_max
            .configure_roster([])
            .expect("an exact untrusted-only usize::MAX byte partition is valid");
        exact_max
            .open()
            .expect("an exact representable maximum must not be rejected as overflow");

        let validator = validator_peers(1).pop().expect("validator fixture");
        let aggregate_overflow = super::FairV2Ingress::new(6, usize::MAX, usize::MAX, 0, 0);
        let error = aggregate_overflow
            .configure_roster([validator.clone()])
            .expect_err("two source partitions must not overflow into an apparent exact fit");
        assert_eq!(error.configured(), usize::MAX);
        assert_eq!(error.required(), usize::MAX);
        assert_eq!(error.kind, super::FairV2IngressCapacityKind::Bytes);
        assert_eq!(aggregate_overflow.open(), Err(error));

        let reserve_overflow = super::FairV2Ingress::new(6, usize::MAX, usize::MAX, usize::MAX, 1);
        let error = reserve_overflow
            .configure_roster([validator])
            .expect_err("disjoint byte reserves must not overflow into an apparent exact fit");
        assert_eq!(error.configured(), usize::MAX);
        assert_eq!(error.required(), usize::MAX);
        assert_eq!(
            error.kind,
            super::FairV2IngressCapacityKind::TransportCompletionBytes
        );
        assert_eq!(reserve_overflow.open(), Err(error));
    }

    #[test]
    fn fair_v2_ingress_rejects_timeout_vote_larger_than_its_byte_reserve() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let timeout_vote = v2_timeout_vote();
        let timeout_vote_len = encoded_v2_len(&timeout_vote);
        let reserve = timeout_vote_len.checked_sub(1).expect("non-empty envelope");
        let source_capacity = timeout_vote_len * 2;
        let ingress =
            super::FairV2Ingress::new(6, 2 * source_capacity, source_capacity, reserve, 0);
        ingress
            .configure_roster([validator.clone()])
            .expect("the deliberately short reserve still fits its source partition");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(timeout_vote, Some(validator))),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
    }

    #[test]
    fn fair_v2_ingress_rejects_timeout_vote_reserve_larger_than_source_partition() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(6, 2 * 1024, 1024, 1025, 0);
        let error = ingress
            .configure_roster([validator])
            .expect_err("timeout-vote reserve must fit each validator source partition");
        assert!(error.is_bytes());
        assert_eq!(error.configured(), 1024);
        assert_eq!(error.required(), 1025);
    }

    #[test]
    fn fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        for index in 0..3 {
            assert!(
                handle.try_incoming_block_message_from(
                    validator.clone(),
                    v2_auxiliary_prepare(index),
                )
            );
        }
        assert!(
            !handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(3),),
            "auxiliary pressure leaves the validator's transport-completion slot unconsumed"
        );
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_message_with_index(99),)
        );
        assert_eq!(ingress.len(), 4);

        let completion = ingress
            .try_recv_if(|inbound| payload_chunk_index(inbound) == Some(99))
            .expect("same-source transport completion bypasses the saturated auxiliary prefix");
        assert_eq!(completion.sender(), Some(&validator));
        assert_eq!(payload_chunk_index(&completion), Some(99));
        assert!(handle.try_incoming_block_message_from(validator, v2_message_with_index(100),));
        assert_eq!(
            ingress.len(),
            4,
            "service restores the exact per-validator transport-completion reservation"
        );
    }

    #[test]
    fn fair_v2_ingress_prepare_vote_cannot_consume_commit_progress_reservation() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        let prepare =
            InboundBlockMessage::new(v2_vote(wire::GlobalPhase::Prepare), Some(validator.clone()));
        let commit =
            InboundBlockMessage::new(v2_vote(wire::GlobalPhase::Commit), Some(validator.clone()));
        assert_eq!(
            FairV2IngressClass::classify(&prepare),
            FairV2IngressClass::Auxiliary
        );
        assert_eq!(
            FairV2IngressClass::classify(&commit),
            FairV2IngressClass::Progress
        );
        let timeout = InboundBlockMessage::new(v2_timeout_vote(), Some(validator.clone()));
        assert_eq!(
            FairV2IngressClass::classify(&timeout),
            FairV2IngressClass::Progress,
            "TimeoutVote must use the per-validator protected timeout corridor"
        );
        let body_request = InboundBlockMessage::new(
            v2_certified_body_request(&validator),
            Some(validator.clone()),
        );
        assert_eq!(
            FairV2IngressClass::classify(&body_request),
            FairV2IngressClass::Progress,
            "certified body recovery must share the protected progress slot"
        );
        let commit_request = InboundBlockMessage::new(
            v2_commit_certificate_request(0, &validator),
            Some(validator.clone()),
        );
        assert_eq!(
            FairV2IngressClass::classify(&commit_request),
            FairV2IngressClass::Progress,
            "Commit-certificate recovery must share the protected progress slot"
        );

        assert!(handle.try_incoming_block_message_from(
            validator.clone(),
            v2_vote(wire::GlobalPhase::Prepare),
        ));
        for index in 0..2 {
            assert!(
                handle.try_incoming_block_message_from(
                    validator.clone(),
                    v2_auxiliary_prepare(index),
                )
            );
        }
        assert!(
            !handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(2),),
            "Prepare and auxiliary work must leave one same-source Commit slot"
        );
        assert!(handle.try_incoming_block_message_from(
            validator.clone(),
            v2_vote(wire::GlobalPhase::Commit),
        ));

        let delivered = ingress
            .try_recv_if(|inbound| vote_phase(inbound) == Some(wire::GlobalPhase::Commit))
            .expect("Commit vote bypasses the saturated auxiliary prefix");
        assert_eq!(delivered.sender(), Some(&validator));
        assert_eq!(vote_phase(&delivered), Some(wire::GlobalPhase::Commit));
    }

    #[test]
    fn fair_v2_ingress_minimum_capacity_admits_timeout_votes() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(6);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("one validator, its progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(0),)
        );
        assert!(handle.try_incoming_block_message_from(
            validator.clone(),
            v2_commit_certificate_request(0, &validator),
        ));

        let timeout = InboundBlockMessage::new(v2_timeout_vote(), Some(validator.clone()));
        assert_eq!(
            FairV2IngressClass::classify(&timeout),
            FairV2IngressClass::Progress
        );
        assert!(handle.try_incoming_block_message_from(validator.clone(), v2_timeout_vote()));
        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
            .expect("minimum capacity must reserve TimeoutVote behind Prepare and recovery");
        assert_eq!(delivered.sender(), Some(&validator));
        assert!(matches!(
            delivered.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        ));
    }

    #[test]
    fn fair_v2_ingress_reservation_potential_does_not_increase_on_service() {
        assert_eq!(super::fair_v2_ingress_required_capacity(0), Some(1));
        assert_eq!(super::fair_v2_ingress_required_capacity(1), Some(6));
        assert_eq!(super::fair_v2_ingress_required_capacity(4), Some(18));

        for (is_validator, reserve_untrusted_completion) in
            [(false, false), (false, true), (true, true)]
        {
            for depth in 1_usize..=8 {
                for timeout_count in 0..=usize::from(is_validator) {
                    let completion_limit = 1_usize.min(depth.saturating_sub(timeout_count));
                    for completion_count in 0..=completion_limit {
                        let remaining = depth - timeout_count - completion_count;
                        for progress_count in 0..=remaining {
                            let auxiliary_count =
                                depth - timeout_count - completion_count - progress_count;
                            for removed in [
                                "Auxiliary",
                                "Progress",
                                "TimeoutVote",
                                "TransportCompletion",
                            ] {
                                if (removed == "Auxiliary" && auxiliary_count == 0)
                                    || (removed == "Progress" && progress_count == 0)
                                    || (removed == "TimeoutVote" && timeout_count == 0)
                                    || (removed == "TransportCompletion" && completion_count == 0)
                                {
                                    continue;
                                }
                                let next_progress_count =
                                    progress_count - usize::from(removed == "Progress");
                                let next_timeout_count =
                                    timeout_count - usize::from(removed == "TimeoutVote");
                                let next_completion_count = completion_count
                                    - usize::from(removed == "TransportCompletion");
                                let before = depth
                                    + super::fair_v2_ingress_lane_protected_slots(
                                        is_validator,
                                        reserve_untrusted_completion,
                                        depth,
                                        progress_count != 0,
                                        timeout_count != 0,
                                        completion_count != 0,
                                    );
                                let after = depth - 1
                                    + super::fair_v2_ingress_lane_protected_slots(
                                        is_validator,
                                        reserve_untrusted_completion,
                                        depth - 1,
                                        next_progress_count != 0,
                                        next_timeout_count != 0,
                                        next_completion_count != 0,
                                    );
                                assert!(
                                    after <= before,
                                    "service increased potential: validator={is_validator}, depth={depth}, progress={progress_count}, timeout={timeout_count}, completion={completion_count}, removed={removed}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fair_v2_ingress_saturated_peer_cannot_block_an_empty_validator_timeout() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
        let validators = validator_peers(2);
        let saturated = validators[0].clone();
        let honest = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(saturated.clone(), v2_auxiliary_prepare(0),)
        );
        assert!(
            handle.try_incoming_block_message_from(saturated.clone(), v2_message_with_index(1),)
        );
        assert!(
            !handle.try_incoming_block_message_from(saturated.clone(), v2_auxiliary_prepare(2),),
            "borrowed capacity must preserve both slots needed by an empty validator lane"
        );
        assert!(
            handle.try_incoming_block_message_from(honest.clone(), v2_timeout_vote()),
            "the saturated peer must not consume the honest validator's timeout slot"
        );

        let delivered = ingress
            .try_recv_if(|inbound| inbound.sender() == Some(&honest))
            .expect("honest timeout remains serviceable despite peer saturation");
        assert!(matches!(
            delivered.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        ));
    }

    #[test]
    fn fair_v2_ingress_non_head_service_consumes_one_source_turn() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
        let validators = validator_peers(2);
        let first_source = validators[0].clone();
        let second_source = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(first_source.clone(), v2_auxiliary_prepare(0),)
        );
        assert!(handle.try_incoming_block_message_from(
            first_source.clone(),
            v2_vote(wire::GlobalPhase::Commit),
        ));
        assert!(
            handle.try_incoming_block_message_from(second_source.clone(), v2_auxiliary_prepare(1),)
        );

        let bypass = ingress
            .try_recv_if(|inbound| vote_phase(inbound) == Some(wire::GlobalPhase::Commit))
            .expect("the first source's later admissible entry is selected");
        assert_eq!(bypass.sender(), Some(&first_source));
        assert_eq!(vote_phase(&bypass), Some(wire::GlobalPhase::Commit));

        let next = ingress
            .try_recv_if(|_| true)
            .expect("the other ready source owns the next turn");
        assert_eq!(next.sender(), Some(&second_source));
        assert_eq!(vote_height(&next), Some(2));

        let retained = ingress
            .try_recv_if(|_| true)
            .expect("the bypassed entry remains in its original source lane");
        assert_eq!(retained.sender(), Some(&first_source));
        assert_eq!(vote_height(&retained), Some(1));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_snapshot_tracks_live_depth_and_oldest_age() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
        let validators = validator_peers(2);
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("two validators, their progress and TimeoutVote slots, and untrusted fit");
        ingress.open().expect("open configured roster");

        let captured_at = Instant::now();
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_message(), Some(validators[0].clone())),
                captured_at - Duration::from_secs(5),
            )
            .expect("enqueue oldest validator message");
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_message(), Some(validators[1].clone())),
                captured_at - Duration::from_secs(2),
            )
            .expect("enqueue newer validator message");
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_message_with_index(1), Some(validators[0].clone())),
                captured_at - Duration::from_secs(7),
            )
            .expect("enqueue timestamp-inverted message behind the source head");

        assert_eq!(
            ingress.snapshot_at(captured_at),
            super::FairV2IngressSnapshot {
                depth: 3,
                capacity: 10,
                oldest_age: Some(Duration::from_secs(7)),
                service_idle_age: Some(Duration::from_secs(5)),
            }
        );
        let _ = ingress
            .try_recv_if_at(captured_at, |_| true)
            .expect("drain first fair source head");
        assert_eq!(
            ingress.snapshot_at(captured_at),
            super::FairV2IngressSnapshot {
                depth: 2,
                capacity: 10,
                oldest_age: Some(Duration::from_secs(7)),
                service_idle_age: Some(Duration::ZERO),
            }
        );
        let _ = ingress
            .try_recv_if_at(captured_at, |_| true)
            .expect("drain second fair source head");
        assert_eq!(
            ingress.snapshot_at(captured_at),
            super::FairV2IngressSnapshot {
                depth: 1,
                capacity: 10,
                oldest_age: Some(Duration::from_secs(7)),
                service_idle_age: Some(Duration::ZERO),
            }
        );
        let _ = ingress
            .try_recv_if_at(captured_at, |_| true)
            .expect("drain remaining fair source");
        assert_eq!(
            ingress.snapshot_at(captured_at),
            super::FairV2IngressSnapshot {
                depth: 0,
                capacity: 10,
                oldest_age: None,
                service_idle_age: None,
            }
        );
    }

    #[test]
    fn fair_v2_ingress_service_idle_age_tracks_scans_not_oldest_item_age() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(7);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        let captured_at = Instant::now();
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_auxiliary_prepare(0), Some(validator.clone())),
                captured_at - Duration::from_secs(5),
            )
            .expect("enqueue old blocked entry");
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_auxiliary_prepare(1), Some(validator)),
                captured_at - Duration::from_secs(1),
            )
            .expect("enqueue later admissible entry");

        assert!(
            ingress.try_recv_if_at(captured_at, |_| false).is_none(),
            "a rejected scan must retain both entries"
        );
        assert_eq!(
            ingress.snapshot_at(captured_at + Duration::from_secs(2)),
            super::FairV2IngressSnapshot {
                depth: 2,
                capacity: 7,
                oldest_age: Some(Duration::from_secs(7)),
                service_idle_age: Some(Duration::from_secs(2)),
            }
        );

        let selected = ingress
            .try_recv_if_at(captured_at + Duration::from_secs(2), |inbound| {
                vote_height(inbound) == Some(2)
            })
            .expect("later admissible entry bypasses the old blocked entry");
        assert_eq!(vote_height(&selected), Some(2));
        assert_eq!(
            ingress.snapshot_at(captured_at + Duration::from_secs(3)),
            super::FairV2IngressSnapshot {
                depth: 1,
                capacity: 7,
                oldest_age: Some(Duration::from_secs(8)),
                service_idle_age: Some(Duration::from_secs(1)),
            },
            "successful fair service refreshes the scan clock without hiding old ownership"
        );
    }

    #[test]
    fn fair_v2_ingress_empty_to_nonempty_resets_service_idle_baseline() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(2);
        ingress.close();
        ingress
            .configure_roster(std::iter::empty())
            .expect("untrusted ingress lane fits");
        ingress.open().expect("open configured roster");

        let captured_at = Instant::now();
        assert!(ingress.try_recv_if_at(captured_at, |_| true).is_none());
        ingress
            .try_push_at(
                InboundBlockMessage::new(BlockMessage::invalid_wire_sentinel(), None),
                captured_at + Duration::from_secs(5),
            )
            .expect("enqueue after an empty-queue scan");
        assert_eq!(
            ingress
                .snapshot_at(captured_at + Duration::from_secs(6))
                .service_idle_age,
            Some(Duration::from_secs(1)),
            "an empty-queue scan must not make later ownership look serviced"
        );

        let _ = ingress
            .try_recv_if_at(captured_at + Duration::from_secs(7), |_| true)
            .expect("drain the first ownership interval");
        assert_eq!(
            ingress
                .snapshot_at(captured_at + Duration::from_secs(8))
                .service_idle_age,
            None
        );

        ingress
            .try_push_at(
                InboundBlockMessage::new(BlockMessage::invalid_wire_sentinel(), None),
                captured_at + Duration::from_secs(10),
            )
            .expect("enqueue a fresh ownership interval");
        assert_eq!(
            ingress
                .snapshot_at(captured_at + Duration::from_secs(11))
                .service_idle_age,
            Some(Duration::from_secs(1)),
            "the prior ownership interval's service time must not mask a fresh stall"
        );
    }

    #[test]
    fn anonymous_and_non_roster_v2_sources_share_one_bounded_lane() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(18);
        let validators = validator_peers(4);
        let outsiders = validator_peers(9).split_off(4);
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("minimum fair-lane capacity");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message(v2_message()));
        for (index, outsider) in outsiders.iter().enumerate() {
            assert!(
                !handle.try_incoming_block_message_from(
                    outsider.clone(),
                    v2_auxiliary_prepare(u64::try_from(index + 1).expect("small index")),
                ),
                "transport identities cannot expand the one untrusted owner"
            );
        }
        for validator in validators {
            assert!(handle.try_incoming_block_message_from(validator, v2_message()));
        }
        assert_eq!(ingress.len(), 5);

        let anonymous = ingress
            .try_recv_if(|inbound| inbound.sender().is_none())
            .expect("the shared untrusted owner remains fairly serviceable");
        assert!(anonymous.sender().is_none());
        assert!(
            handle.try_incoming_block_message_from(outsiders[0].clone(), v2_auxiliary_prepare(1),)
        );
        assert_eq!(ingress.len(), 5);
    }

    #[test]
    fn v2_ingress_rejects_capacity_without_per_validator_progress_reservations() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(17);
        ingress.close();
        let error = ingress
            .configure_roster(validator_peers(4))
            .expect_err(
                "four validators require ordinary, progress, TimeoutVote, and transport-completion slots",
            );
        assert_eq!(error.configured(), 17);
        assert_eq!(error.required(), 18);
        assert_eq!(ingress.open(), Err(error));
    }
}

impl SumeragiWorker {
    fn run(self) {
        v2_runner::run(self);
    }
}
