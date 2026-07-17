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
        sumeragi::{TIMEOUT_VOTE_RESERVE_BYTES, npos::EPOCH_LENGTH_BLOCKS},
    },
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, PublicKey};
use iroha_data_model::{
    ChainId, block::consensus_v2::ConsensusMode, consensus::VrfEpochRecord,
    merge::MergeCommitteeSignature, nexus::LaneRelayEnvelope, peer::PeerId,
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
const _: () = assert!(TIMEOUT_VOTE_RESERVE_BYTES >= MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES);

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
enum LaneRelayMessage {
    Envelope(LaneRelayEnvelope),
    MergeSignature(MergeCommitteeSignature),
    CertifiedMergeSidecar {
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    },
    NativeAmx {
        sender: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    },
}

/// One normalized live-consensus message plus its authenticated transport peer.
#[derive(Clone, Debug)]
pub(crate) struct InboundBlockMessage {
    message: BlockMessage,
    sender: Option<PeerId>,
}

impl InboundBlockMessage {
    pub(crate) fn new(message: BlockMessage, sender: Option<PeerId>) -> Self {
        Self {
            message: message.normalize(),
            sender,
        }
    }

    /// Consume the envelope and return the normalized message and transport peer.
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

    /// Borrow the authenticated transport peer, when one was supplied.
    pub(crate) fn sender(&self) -> Option<&PeerId> {
        self.sender.as_ref()
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
    open: bool,
}

#[derive(Default)]
struct FairV2IngressLane {
    entries: VecDeque<FairV2IngressEntry>,
    pending_wire: BTreeSet<FairV2IngressWireKey>,
    progress_len: usize,
    timeout_vote_len: usize,
    bytes: usize,
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
    sender: Option<PeerId>,
    hash: CryptoHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressClass {
    Auxiliary,
    Progress,
}

impl FairV2IngressClass {
    fn classify(inbound: &InboundBlockMessage) -> Self {
        let BlockMessage::V2(message) = inbound.message() else {
            return Self::Auxiliary;
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
            | ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | ConsensusMessageV2Payload::CommitCertificateResponse(_) => Self::Progress,
            ConsensusMessageV2Payload::Proposal(_)
            | ConsensusMessageV2Payload::Vote(_)
            | ConsensusMessageV2Payload::TimeoutVote(_)
            | ConsensusMessageV2Payload::PayloadManifest(_)
            | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | ConsensusMessageV2Payload::CommitCertificateRequest(_) => Self::Auxiliary,
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
            FairV2IngressCapacityKind::Bytes | FairV2IngressCapacityKind::TimeoutVoteBytes
        )
    }
}

fn fair_v2_ingress_required_capacity(roster_len: usize) -> usize {
    roster_len
        .checked_mul(2)
        .and_then(|required| required.checked_add(1))
        .unwrap_or(usize::MAX)
}

const fn fair_v2_ingress_lane_protected_slots(
    is_validator: bool,
    depth: usize,
    has_progress: bool,
) -> usize {
    let first_or_progress = if depth == 0 || (is_validator && !has_progress) {
        1
    } else {
        0
    };
    let continuation = if is_validator && (depth == 0 || (depth == 1 && has_progress)) {
        1
    } else {
        0
    };
    first_or_progress + continuation
}

fn fair_v2_ingress_required_byte_capacity(roster_len: usize, source_byte_capacity: usize) -> usize {
    roster_len
        .checked_add(1)
        .and_then(|source_count| source_count.checked_mul(source_byte_capacity))
        .unwrap_or(usize::MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressPushError {
    Closed,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressPushDisposition {
    Enqueued,
    Coalesced,
}

/// Fixed-capacity, roster-aware v2 ingress with per-source admission and service fairness.
///
/// Every active validator owns one protected source slot and, while its lane
/// contains only auxiliary work, one additional progress slot. Anonymous and
/// non-roster traffic shares one untrusted lane, so creating transport
/// identities cannot consume a validator's reservation. Exact wire
/// retransmissions coalesce only while the same source still owns an identical
/// queued envelope; after service, a later retransmission is admitted normally.
/// Non-empty lanes are serviced in round-robin order, so a source may use
/// otherwise idle capacity but cannot starve an honest validator's progress.
/// Canonical envelope hashes are computed before taking the shared queue lock,
/// so duplicate detection never compares whole bodies while holding that lock.
/// Canonical wire bytes are also charged to fixed aggregate and per-source
/// budgets. Roster installation succeeds only when every validator and the
/// shared untrusted lane own an isolated byte partition. Non-timeout traffic
/// in a validator lane cannot consume its fixed timeout-vote byte reserve, so
/// auxiliary byte pressure cannot contradict the formal view-change admission
/// guarantee. Each validator lane owns at most one distinct queued TimeoutVote
/// at a time; exact retransmissions coalesce and a newer vote retries after fair
/// service releases that critical byte owner.
pub(crate) struct FairV2Ingress {
    capacity: usize,
    byte_capacity: usize,
    source_byte_capacity: usize,
    timeout_vote_byte_reserve: usize,
    state: Mutex<FairV2IngressState>,
}

impl FairV2Ingress {
    fn new(
        capacity: usize,
        byte_capacity: usize,
        source_byte_capacity: usize,
        timeout_vote_byte_reserve: usize,
    ) -> Self {
        let mut lanes = BTreeMap::new();
        lanes.insert(FairV2IngressSource::Untrusted, FairV2IngressLane::default());
        Self {
            capacity,
            byte_capacity,
            source_byte_capacity,
            timeout_vote_byte_reserve,
            state: Mutex::new(FairV2IngressState {
                roster: BTreeSet::new(),
                lanes,
                ready: VecDeque::new(),
                len: 0,
                bytes: 0,
                nonempty_since: None,
                last_service_attempt_at: None,
                open: false,
            }),
        }
    }

    fn debug_assert_consistent(&self, state: &FairV2IngressState) {
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
                if matches!(source, FairV2IngressSource::Validator(_)) {
                    let timeout_vote_bytes = lane
                        .entries
                        .iter()
                        .filter(|entry| fair_v2_ingress_is_timeout_vote(&entry.inbound))
                        .map(|entry| {
                            debug_assert!(entry.encoded_len <= self.timeout_vote_byte_reserve);
                            entry.encoded_len
                        })
                        .sum::<usize>();
                    debug_assert!(lane.timeout_vote_len <= 1);
                    debug_assert!(timeout_vote_bytes <= self.timeout_vote_byte_reserve);
                    debug_assert!(lane.bytes.checked_sub(timeout_vote_bytes).is_some_and(
                        |non_timeout_bytes| {
                            non_timeout_bytes
                                <= self
                                    .source_byte_capacity
                                    .saturating_sub(self.timeout_vote_byte_reserve)
                        }
                    ));
                }
            }

            if state.open {
                debug_assert!(self.timeout_vote_byte_reserve <= self.source_byte_capacity);
                let protected = state
                    .lanes
                    .iter()
                    .map(|(source, lane)| {
                        let is_validator = matches!(source, FairV2IngressSource::Validator(_));
                        let has_progress = lane.progress_len != 0;
                        fair_v2_ingress_lane_protected_slots(
                            is_validator,
                            lane.entries.len(),
                            has_progress,
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
    pub(crate) fn configure_roster(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
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
        let required_bytes =
            fair_v2_ingress_required_byte_capacity(state.roster.len(), self.source_byte_capacity);
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
        let required = fair_v2_ingress_required_capacity(state.roster.len());
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
        let required_bytes =
            fair_v2_ingress_required_byte_capacity(state.roster.len(), self.source_byte_capacity);
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
        let (wire_hash, encoded_len) = match inbound.message() {
            BlockMessage::V2(message) => {
                let encoded = message.encode();
                let encoded_len = encoded.len();
                (Some(CryptoHash::new(encoded)), encoded_len)
            }
            _ => (None, 0),
        };
        let wire_key = wire_hash.map(|hash| FairV2IngressWireKey {
            sender: inbound.sender.clone(),
            hash,
        });
        let mut state = self.state.lock();
        if !state.open {
            return Err(FairV2IngressPushError::Closed);
        }
        let source = inbound
            .sender
            .as_ref()
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
        // A validator lane has one critical TimeoutVote byte owner. Exact
        // retransmissions were coalesced above; a distinct later-view vote is
        // retried by retained control after fair service releases the owner.
        // This keeps the runtime byte abstraction equal to the formal gate
        // instead of allowing several votes to consume one logical reserve.
        if is_validator_source && is_timeout_vote && lane.timeout_vote_len != 0 {
            return Err(FairV2IngressPushError::Full);
        }
        if is_validator_source && is_timeout_vote && encoded_len > self.timeout_vote_byte_reserve {
            return Err(FairV2IngressPushError::Full);
        }
        let source_byte_limit = if is_validator_source && !is_timeout_vote {
            self.source_byte_capacity
                .saturating_sub(self.timeout_vote_byte_reserve)
        } else {
            self.source_byte_capacity
        };
        if encoded_len > source_byte_limit.saturating_sub(lane.bytes)
            || encoded_len > self.byte_capacity.saturating_sub(state.bytes)
        {
            return Err(FairV2IngressPushError::Full);
        }

        // Project the reservation potential after this admission. Every empty
        // source needs a first-message slot. A validator without queued
        // progress also needs a progress slot. Finally, a validator whose sole
        // entry is progress keeps one continuation slot so servicing that
        // entry cannot destroy the two reservations of the resulting empty
        // lane. The incoming item is part of the projection.
        let protected_slots_after_admission = state
            .lanes
            .iter()
            .map(|(lane_source, lane)| {
                let is_target = *lane_source == source;
                let projected_len = lane.entries.len() + usize::from(is_target);
                let is_validator = matches!(lane_source, FairV2IngressSource::Validator(_));
                let has_progress =
                    lane.progress_len != 0 || (is_target && class == FairV2IngressClass::Progress);
                fair_v2_ingress_lane_protected_slots(is_validator, projected_len, has_progress)
            })
            .sum::<usize>();
        let usable_capacity = self
            .capacity
            .saturating_sub(protected_slots_after_admission);
        if state.len >= usable_capacity {
            return Err(FairV2IngressPushError::Full);
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
/// All accepted queues are bounded and non-blocking; retransmission belongs to
/// the v2 reducer or the independent lane-local adapter.
#[derive(Clone)]
pub struct SumeragiHandle {
    block: Arc<FairV2Ingress>,
    lane_payload: mpsc::SyncSender<InboundBlockMessage>,
    lane_votes: mpsc::SyncSender<InboundBlockMessage>,
    lane_relay: mpsc::SyncSender<LaneRelayMessage>,
    wake: mpsc::SyncSender<()>,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
}

impl SumeragiHandle {
    fn new(
        block: Arc<FairV2Ingress>,
        lane_payload: mpsc::SyncSender<InboundBlockMessage>,
        lane_votes: mpsc::SyncSender<InboundBlockMessage>,
        lane_relay: mpsc::SyncSender<LaneRelayMessage>,
        wake: mpsc::SyncSender<()>,
        ingress_ready: Arc<AtomicBool>,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            block,
            lane_payload,
            lane_votes,
            lane_relay,
            wake,
            ingress_ready,
            output_guard,
        }
    }

    fn ingress_is_ready(&self) -> bool {
        self.ingress_ready.load(Ordering::Acquire) && !self.output_guard.restart_required()
    }

    fn wake(&self) {
        let _ = self.wake.try_send(());
    }

    fn try_enqueue_block(&self, sender: Option<PeerId>, message: BlockMessage) -> bool {
        let Some(_permit) = self.output_guard.acquire() else {
            return false;
        };
        if !self.ingress_is_ready() {
            iroha_logger::debug!(
                "rejecting Sumeragi ingress until context and safety WAL replay complete"
            );
            return false;
        }

        if matches!(&message, BlockMessage::V2(_)) {
            let queue = status::WorkerQueueKind::Blocks;
            return match self
                .block
                .try_push(InboundBlockMessage::new(message, sender))
            {
                Ok(FairV2IngressPushDisposition::Enqueued) => {
                    status::record_worker_queue_enqueue(queue);
                    self.wake();
                    true
                }
                Ok(FairV2IngressPushDisposition::Coalesced) => true,
                Err(FairV2IngressPushError::Full) => {
                    status::record_worker_queue_drop(queue);
                    iroha_logger::debug!(
                        ?queue,
                        "bounded per-source Sumeragi v2 ingress queue is full"
                    );
                    false
                }
                Err(FairV2IngressPushError::Closed) => {
                    status::record_worker_queue_drop(queue);
                    iroha_logger::debug!(
                        ?queue,
                        "Sumeragi v2 ingress queue closed during height rollover"
                    );
                    false
                }
            };
        }

        let (tx, queue) = match message {
            BlockMessage::LaneBlockVote(_) => (&self.lane_votes, status::WorkerQueueKind::Votes),
            BlockMessage::LaneBlockProposal(_) | BlockMessage::LaneBlockQc(_) => {
                (&self.lane_payload, status::WorkerQueueKind::BlockPayload)
            }
            _ => {
                iroha_logger::debug!(
                    "rejecting decode-only Sumeragi v1 frame on the v2 live ingress"
                );
                return false;
            }
        };
        match tx.try_send(InboundBlockMessage::new(message, sender)) {
            Ok(()) => {
                status::record_worker_queue_enqueue(queue);
                self.wake();
                true
            }
            Err(mpsc::TrySendError::Full(_)) => {
                status::record_worker_queue_drop(queue);
                iroha_logger::debug!(?queue, "bounded Sumeragi v2 ingress queue is full");
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                status::record_worker_queue_drop(queue);
                iroha_logger::warn!(?queue, "Sumeragi v2 ingress queue is disconnected");
                false
            }
        }
    }

    /// Enqueue a canonical v2 or retained lane-local message without blocking.
    pub fn incoming_block_message(&self, message: BlockMessage) -> bool {
        self.try_enqueue_block(None, message.normalize())
    }

    /// Enqueue a canonical message from an authenticated transport peer.
    pub fn incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) {
        let _ = self.try_enqueue_block(Some(sender), message.normalize());
    }

    /// Try to enqueue a canonical message without blocking.
    pub fn try_incoming_block_message(&self, message: BlockMessage) -> bool {
        self.try_enqueue_block(None, message.normalize())
    }

    /// Try to enqueue a canonical message from an authenticated transport peer.
    pub fn try_incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) -> bool {
        self.try_enqueue_block(Some(sender), message.normalize())
    }

    /// Reject retired v1 control-flow frames.
    pub fn incoming_consensus_control_flow_message(&self, _message: ControlFlow) {
        iroha_logger::debug!("rejecting decode-only Sumeragi v1 control-flow frame");
    }

    /// Reject retired v1 control-flow frames.
    pub fn try_incoming_consensus_control_flow_message(&self, _message: ControlFlow) -> bool {
        false
    }

    fn try_enqueue_lane_relay(&self, message: LaneRelayMessage) -> bool {
        let Some(_permit) = self.output_guard.acquire() else {
            return false;
        };
        if !self.ingress_is_ready() {
            return false;
        }
        match self.lane_relay.try_send(message) {
            Ok(()) => {
                status::record_worker_queue_enqueue(status::WorkerQueueKind::LaneRelay);
                self.wake();
                true
            }
            Err(mpsc::TrySendError::Full(_)) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                iroha_logger::debug!("bounded lane-local ingress queue is full");
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                status::record_worker_queue_drop(status::WorkerQueueKind::LaneRelay);
                iroha_logger::warn!("lane-local ingress queue is disconnected");
                false
            }
        }
    }

    /// Enqueue an inbound lane relay envelope.
    pub fn incoming_lane_relay(&self, envelope: LaneRelayEnvelope) {
        let _ = self.try_incoming_lane_relay(envelope);
    }

    /// Try to enqueue an inbound lane relay envelope.
    pub fn try_incoming_lane_relay(&self, envelope: LaneRelayEnvelope) -> bool {
        self.try_enqueue_lane_relay(LaneRelayMessage::Envelope(envelope))
    }

    /// Enqueue an inbound merge-committee signature.
    pub fn incoming_merge_signature(&self, signature: MergeCommitteeSignature) {
        let _ = self.try_incoming_merge_signature(signature);
    }

    /// Try to enqueue an inbound merge-committee signature.
    pub fn try_incoming_merge_signature(&self, signature: MergeCommitteeSignature) -> bool {
        self.try_enqueue_lane_relay(LaneRelayMessage::MergeSignature(signature))
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
        self.try_enqueue_lane_relay(LaneRelayMessage::CertifiedMergeSidecar { sender, message })
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
        self.try_enqueue_lane_relay(LaneRelayMessage::NativeAmx { sender, message })
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
    let block = Arc::new(FairV2Ingress::new(
        block_capacity,
        TEST_AGGREGATE_BYTE_CAPACITY,
        TEST_SOURCE_BYTE_CAPACITY,
        TIMEOUT_VOTE_RESERVE_BYTES,
    ));
    block
        .configure_roster(std::iter::empty())
        .expect("test untrusted lane fits configured capacity");
    block.open().expect("open configured test ingress");
    let (lane_payload_tx, _lane_payload_rx) = mpsc::sync_channel(block_capacity);
    let (lane_vote_tx, _lane_vote_rx) = mpsc::sync_channel(block_capacity);
    let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(block_capacity);
    let (wake_tx, _wake_rx) = mpsc::sync_channel(1);
    let handle = SumeragiHandle::new(
        Arc::clone(&block),
        lane_payload_tx,
        lane_vote_tx,
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

        let vote_channel_cap = config.queues.commands.get();
        let block_payload_channel_cap = config.queues.chunks.get();
        let block_channel_cap = config.queues.bodies.get();
        let block_byte_cap = config.queues.body_bytes.get();
        let block_source_byte_cap = config.queues.body_source_bytes.get();
        let lane_relay_channel_cap = config.queues.ready_bodies.get();
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(block_payload_channel_cap);
        let block = Arc::new(FairV2Ingress::new(
            block_channel_cap,
            block_byte_cap,
            block_source_byte_cap,
            TIMEOUT_VOTE_RESERVE_BYTES,
        ));
        let (vote_tx, vote_rx) = mpsc::sync_channel(vote_channel_cap);
        let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(lane_relay_channel_cap);
        let (wake_tx, wake_rx) = mpsc::sync_channel(WORKER_WAKE_CHANNEL_CAP);
        let queue_wake = Arc::clone(&queue);
        let queue_wake_tx = wake_tx.clone();
        let ingress_ready = Arc::new(AtomicBool::new(false));

        let handle = SumeragiHandle::new(
            Arc::clone(&block),
            block_payload_tx,
            vote_tx,
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
            vote_rx,
            block_payload_rx,
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
    vote_rx: mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: mpsc::Receiver<InboundBlockMessage>,
    block_rx: Arc<FairV2Ingress>,
    wake_rx: mpsc::Receiver<()>,
    shutdown_signal: ShutdownSignal,
}

#[cfg(test)]
mod authoritative_runtime_gate_tests {
    use std::{
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId,
        block::consensus_v2 as wire,
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
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
        v2_message_with_index(0)
    }

    fn v2_auxiliary_request(index: u64, requester: &PeerId) -> BlockMessage {
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

    fn vote_phase(inbound: &InboundBlockMessage) -> Option<wire::GlobalPhase> {
        let BlockMessage::V2(message) = inbound.message() else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::Vote(vote) = &message.payload else {
            return None;
        };
        Some(vote.phase)
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
            !handle.incoming_block_message(v2_message_with_index(1)),
            "a distinct message at saturated capacity must reject promptly and rely on retransmission"
        );
        let _ = receiver.try_recv().expect("drain the bounded v2 queue");
        assert!(handle.incoming_block_message(v2_message()));
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(9);
        let validators = validator_peers(4);
        let attacker = validators[0].clone();
        let outsider = validator_peers(5).pop().expect("outsider fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validators, their progress slots, and the untrusted lane fit");
        ingress.open().expect("open configured roster");

        for index in 0..5 {
            assert!(
                handle.try_incoming_block_message_from(
                    attacker.clone(),
                    v2_message_with_index(index),
                )
            );
        }
        assert!(
            !handle.try_incoming_block_message_from(attacker.clone(), v2_message_with_index(5),),
            "attacker cannot consume slots reserved for empty validator and untrusted lanes"
        );
        for honest in validators.iter().skip(1) {
            assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));
        }
        assert!(handle.try_incoming_block_message_from(outsider.clone(), v2_message()));
        assert_eq!(ingress.len(), 9);

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
    }

    #[test]
    fn fair_v2_ingress_retains_ready_head_until_downstream_admission() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(2);
        let attacker = validators[0].clone();
        let honest = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress slots, and the untrusted lane fit");
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(2);
        let blocked = validators[0].clone();
        let admissible = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress slots, and the untrusted lane fit");
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(4);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_message_with_index(0),)
        );
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_message_with_index(1),)
        );
        assert!(handle.try_incoming_block_message_from(validator, v2_message_with_index(2),));

        let selected = ingress
            .try_recv_if(|inbound| payload_chunk_index(inbound) == Some(2))
            .expect("admissible body progress bypasses a blocked auxiliary head");
        assert_eq!(payload_chunk_index(&selected), Some(2));
        assert_eq!(ingress.len(), 2);

        let first_retained = ingress
            .try_recv_if(|_| true)
            .expect("oldest blocked entry remains owned for a later fair turn");
        assert_eq!(payload_chunk_index(&first_retained), Some(0));
        let second_retained = ingress
            .try_recv_if(|_| true)
            .expect("later blocked entry retains its relative order");
        assert_eq!(payload_chunk_index(&second_retained), Some(1));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_coalesces_only_a_pending_exact_source_retransmission() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(3);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");
        let request = v2_auxiliary_request(0, &validator);

        assert!(handle.try_incoming_block_message_from(validator.clone(), request.clone()));
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), request.clone()),
            "a retransmitter keeps ownership through the queued exact occurrence"
        );
        assert_eq!(ingress.len(), 1, "the exact pending wire value coalesces");
        assert!(
            !handle.try_incoming_block_message_from(
                validator.clone(),
                v2_auxiliary_request(1, &validator),
            ),
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
    fn fair_v2_ingress_wire_index_keeps_untrusted_senders_distinct() {
        let ingress = super::FairV2Ingress::new(3, 3 * 1024 * 1024, 1024 * 1024, 0);
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
            "the shared untrusted lane coalesces only an exact transport source"
        );
    }

    #[test]
    fn fair_v2_ingress_byte_quota_isolates_validator_sources() {
        let validators = validator_peers(2);
        let first = v2_message_with_bytes(0, 64);
        let second = v2_message_with_bytes(1, 64);
        let encoded_len = encoded_v2_len(&first);
        assert_eq!(encoded_v2_len(&second), encoded_len);
        let ingress = super::FairV2Ingress::new(7, encoded_len * 3, encoded_len, 0);
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
            Err(super::FairV2IngressPushError::Full)
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
    fn fair_v2_ingress_rejects_insufficient_roster_byte_partition() {
        let validators = validator_peers(2);
        let ingress = super::FairV2Ingress::new(5, 2 * 1024, 1024, 0);
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
        let auxiliary = v2_auxiliary_request(0, &validator);
        let timeout_vote = v2_maximum_valid_timeout_vote_wire();
        let auxiliary_len = encoded_v2_len(&auxiliary);
        let timeout_vote_len = encoded_v2_len(&timeout_vote);
        let source_capacity = auxiliary_len + timeout_vote_len;
        let ingress =
            super::FairV2Ingress::new(5, 2 * source_capacity, source_capacity, timeout_vote_len);
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
                v2_auxiliary_request(1, &validator),
                Some(validator.clone()),
            )),
            Err(super::FairV2IngressPushError::Full)
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
        let ingress = super::FairV2Ingress::new(5, 2 * reserve, reserve, reserve);
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
            Err(super::FairV2IngressPushError::Full)
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
    fn fair_v2_ingress_rejects_timeout_vote_larger_than_its_byte_reserve() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let timeout_vote = v2_timeout_vote();
        let timeout_vote_len = encoded_v2_len(&timeout_vote);
        let reserve = timeout_vote_len.checked_sub(1).expect("non-empty envelope");
        let source_capacity = timeout_vote_len * 2;
        let ingress = super::FairV2Ingress::new(3, 2 * source_capacity, source_capacity, reserve);
        ingress
            .configure_roster([validator.clone()])
            .expect("the deliberately short reserve still fits its source partition");
        ingress.open().expect("open configured roster");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(timeout_vote, Some(validator))),
            Err(super::FairV2IngressPushError::Full)
        ));
    }

    #[test]
    fn fair_v2_ingress_rejects_timeout_vote_reserve_larger_than_source_partition() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(3, 2 * 1024, 1024, 1025);
        let error = ingress
            .configure_roster([validator])
            .expect_err("timeout-vote reserve must fit each validator source partition");
        assert!(error.is_bytes());
        assert_eq!(error.configured(), 1024);
        assert_eq!(error.required(), 1025);
    }

    #[test]
    fn fair_v2_ingress_reserves_same_source_progress_behind_auxiliary_pressure() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        for index in 0..3 {
            assert!(handle.try_incoming_block_message_from(
                validator.clone(),
                v2_auxiliary_request(index, &validator),
            ));
        }
        assert!(
            !handle.try_incoming_block_message_from(
                validator.clone(),
                v2_auxiliary_request(3, &validator),
            ),
            "auxiliary pressure leaves the validator's progress slot unconsumed"
        );
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_message_with_index(99),)
        );
        assert_eq!(ingress.len(), 4);

        let progress = ingress
            .try_recv_if(|inbound| payload_chunk_index(inbound) == Some(99))
            .expect("same-source progress bypasses the saturated auxiliary prefix");
        assert_eq!(progress.sender(), Some(&validator));
        assert_eq!(payload_chunk_index(&progress), Some(99));
        assert!(handle.try_incoming_block_message_from(validator, v2_message_with_index(100),));
        assert_eq!(
            ingress.len(),
            4,
            "service restores the exact per-validator progress reservation"
        );
    }

    #[test]
    fn fair_v2_ingress_prepare_vote_cannot_consume_commit_progress_reservation() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
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

        assert!(handle.try_incoming_block_message_from(
            validator.clone(),
            v2_vote(wire::GlobalPhase::Prepare),
        ));
        for index in 0..2 {
            assert!(handle.try_incoming_block_message_from(
                validator.clone(),
                v2_auxiliary_request(index, &validator),
            ));
        }
        assert!(
            !handle.try_incoming_block_message_from(
                validator.clone(),
                v2_auxiliary_request(2, &validator),
            ),
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(3);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("one validator, its progress slot, and the untrusted lane fit");
        ingress.open().expect("open configured roster");

        let timeout = InboundBlockMessage::new(v2_timeout_vote(), Some(validator.clone()));
        assert_eq!(
            FairV2IngressClass::classify(&timeout),
            FairV2IngressClass::Auxiliary
        );
        assert!(handle.try_incoming_block_message_from(validator.clone(), v2_timeout_vote()));
        let delivered = ingress
            .try_recv()
            .expect("minimum valid capacity must admit an honest timeout vote");
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
        assert_eq!(super::fair_v2_ingress_required_capacity(0), 1);
        assert_eq!(super::fair_v2_ingress_required_capacity(1), 3);
        assert_eq!(super::fair_v2_ingress_required_capacity(4), 9);

        for is_validator in [false, true] {
            for depth in 1..=8 {
                for progress_count in 0..=depth {
                    for removed_progress in [false, true] {
                        if removed_progress && progress_count == 0 {
                            continue;
                        }
                        if !removed_progress && progress_count == depth {
                            continue;
                        }
                        let next_progress_count = progress_count - usize::from(removed_progress);
                        let before = depth
                            + super::fair_v2_ingress_lane_protected_slots(
                                is_validator,
                                depth,
                                progress_count != 0,
                            );
                        let after = depth - 1
                            + super::fair_v2_ingress_lane_protected_slots(
                                is_validator,
                                depth - 1,
                                next_progress_count != 0,
                            );
                        assert!(
                            after <= before,
                            "service increased potential: validator={is_validator}, depth={depth}, progress={progress_count}, removed_progress={removed_progress}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fair_v2_ingress_saturated_peer_cannot_block_an_empty_validator_timeout() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(2);
        let saturated = validators[0].clone();
        let honest = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress slots, and the untrusted lane fit");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message_from(
            saturated.clone(),
            v2_auxiliary_request(0, &saturated),
        ));
        assert!(
            handle.try_incoming_block_message_from(saturated.clone(), v2_message_with_index(1),)
        );
        assert!(
            !handle.try_incoming_block_message_from(
                saturated.clone(),
                v2_auxiliary_request(2, &saturated),
            ),
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(2);
        let first_source = validators[0].clone();
        let second_source = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators, their progress slots, and the untrusted lane fit");
        ingress.open().expect("open configured roster");

        assert!(
            handle.try_incoming_block_message_from(first_source.clone(), v2_message_with_index(0),)
        );
        assert!(
            handle.try_incoming_block_message_from(first_source.clone(), v2_message_with_index(2),)
        );
        assert!(
            handle
                .try_incoming_block_message_from(second_source.clone(), v2_message_with_index(1),)
        );

        let bypass = ingress
            .try_recv_if(|inbound| payload_chunk_index(inbound) != Some(0))
            .expect("the first source's later admissible entry is selected");
        assert_eq!(bypass.sender(), Some(&first_source));
        assert_eq!(payload_chunk_index(&bypass), Some(2));

        let next = ingress
            .try_recv_if(|_| true)
            .expect("the other ready source owns the next turn");
        assert_eq!(next.sender(), Some(&second_source));
        assert_eq!(payload_chunk_index(&next), Some(1));

        let retained = ingress
            .try_recv_if(|_| true)
            .expect("the bypassed entry remains in its original source lane");
        assert_eq!(retained.sender(), Some(&first_source));
        assert_eq!(payload_chunk_index(&retained), Some(0));
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn fair_v2_ingress_snapshot_tracks_live_depth_and_oldest_age() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(2);
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("two validators, their progress slots, and the untrusted lane fit");
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
                capacity: 5,
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
                capacity: 5,
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
                capacity: 5,
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
                capacity: 5,
                oldest_age: None,
                service_idle_age: None,
            }
        );
    }

    #[test]
    fn fair_v2_ingress_service_idle_age_tracks_scans_not_oldest_item_age() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(4);
        let validator = validator_peers(1).pop().expect("validator fixture");
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("validator plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        let captured_at = Instant::now();
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_message_with_index(0), Some(validator.clone())),
                captured_at - Duration::from_secs(5),
            )
            .expect("enqueue old blocked entry");
        ingress
            .try_push_at(
                InboundBlockMessage::new(v2_message_with_index(1), Some(validator)),
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
                capacity: 4,
                oldest_age: Some(Duration::from_secs(7)),
                service_idle_age: Some(Duration::from_secs(2)),
            }
        );

        let selected = ingress
            .try_recv_if_at(captured_at + Duration::from_secs(2), |inbound| {
                payload_chunk_index(inbound) == Some(1)
            })
            .expect("later admissible entry bypasses the old blocked entry");
        assert_eq!(payload_chunk_index(&selected), Some(1));
        assert_eq!(
            ingress.snapshot_at(captured_at + Duration::from_secs(3)),
            super::FairV2IngressSnapshot {
                depth: 1,
                capacity: 4,
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(9);
        let validators = validator_peers(4);
        let outsiders = validator_peers(9).split_off(4);
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("minimum fair-lane capacity");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message(v2_message()));
        for (index, outsider) in outsiders.iter().enumerate().take(4) {
            assert!(handle.try_incoming_block_message_from(
                outsider.clone(),
                v2_message_with_index(u32::try_from(index + 1).expect("small index")),
            ));
        }
        assert!(
            !handle
                .try_incoming_block_message_from(outsiders[4].clone(), v2_message_with_index(5),)
        );
        for validator in validators {
            assert!(handle.try_incoming_block_message_from(validator, v2_message()));
        }
        assert_eq!(ingress.len(), 9);
    }

    #[test]
    fn v2_ingress_rejects_capacity_without_per_validator_progress_reservations() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        ingress.close();
        let error = ingress
            .configure_roster(validator_peers(4))
            .expect_err("four validators require four progress slots and one untrusted slot");
        assert_eq!(error.configured(), 8);
        assert_eq!(error.required(), 9);
        assert_eq!(ingress.open(), Err(error));
    }
}

impl SumeragiWorker {
    fn run(self) {
        v2_runner::run(self);
    }
}
