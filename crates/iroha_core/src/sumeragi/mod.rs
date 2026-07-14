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
    time::Duration,
};

use eyre::Result;
use iroha_config::parameters::{
    actual::{Common as CommonConfig, Sumeragi as SumeragiConfig},
    defaults::{concurrency as concurrency_defaults, sumeragi::npos::EPOCH_LENGTH_BLOCKS},
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, PublicKey};
use iroha_data_model::{
    ChainId, block::consensus_v2::ConsensusMode, consensus::VrfEpochRecord,
    merge::MergeCommitteeSignature, nexus::LaneRelayEnvelope, peer::PeerId,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, try_spawn_os_thread_as_future};
use iroha_genesis::GenesisBlock;
use mv::storage::StorageReadOnly;
use parking_lot::Mutex;

use crate::{
    merge_sidecar::CertifiedMergeSidecarMessage,
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};

static CONFIGURED_SUMERAGI_STACK_SIZE_BYTES: AtomicUsize = AtomicUsize::new(0);
const WORKER_WAKE_CHANNEL_CAP: usize = 1;

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
    lanes: BTreeMap<FairV2IngressSource, VecDeque<InboundBlockMessage>>,
    ready: VecDeque<FairV2IngressSource>,
    len: usize,
    open: bool,
}

/// Invalid fixed capacity for the active roster's fair v2 ingress lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FairV2IngressCapacityError {
    configured: usize,
    required: usize,
}

impl FairV2IngressCapacityError {
    /// Configured total message capacity.
    pub(crate) const fn configured(self) -> usize {
        self.configured
    }

    /// Minimum capacity needed for one protected slot per active lane.
    pub(crate) const fn required(self) -> usize {
        self.required
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressPushError {
    Closed,
    Full,
}

/// Fixed-capacity, roster-aware v2 ingress with per-source admission and service fairness.
///
/// Every active validator owns one protected slot. Anonymous and non-roster
/// traffic shares one untrusted lane, so creating transport identities cannot
/// consume another validator's reservation. Non-empty lanes are serviced in
/// round-robin order; a source may use otherwise idle capacity, but cannot
/// starve an honest validator's retransmission.
pub(crate) struct FairV2Ingress {
    capacity: usize,
    state: Mutex<FairV2IngressState>,
}

impl FairV2Ingress {
    fn new(capacity: usize) -> Self {
        let mut lanes = BTreeMap::new();
        lanes.insert(FairV2IngressSource::Untrusted, VecDeque::new());
        Self {
            capacity,
            state: Mutex::new(FairV2IngressState {
                roster: BTreeSet::new(),
                lanes,
                ready: VecDeque::new(),
                len: 0,
                open: false,
            }),
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
        let required = roster.len().checked_add(1).unwrap_or(usize::MAX);
        let mut lanes = BTreeMap::new();
        for peer in &roster {
            lanes.insert(
                FairV2IngressSource::Validator(peer.clone()),
                VecDeque::new(),
            );
        }
        lanes.insert(FairV2IngressSource::Untrusted, VecDeque::new());
        let mut state = self.state.lock();
        state.open = false;
        state.roster = roster;
        state.lanes = lanes;
        state.ready.clear();
        state.len = 0;
        if required > self.capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required,
            });
        }
        Ok(())
    }

    /// Open admission for the already-configured immutable height.
    pub(crate) fn open(&self) -> Result<(), FairV2IngressCapacityError> {
        let mut state = self.state.lock();
        let required = state.lanes.len();
        if required > self.capacity {
            return Err(FairV2IngressCapacityError {
                configured: self.capacity,
                required,
            });
        }
        state.open = true;
        Ok(())
    }

    /// Close admission before rollover or abnormal runner exit.
    pub(crate) fn close(&self) {
        self.state.lock().open = false;
    }

    fn try_push(&self, inbound: InboundBlockMessage) -> Result<(), FairV2IngressPushError> {
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
        let empty_other_lanes = state
            .lanes
            .iter()
            .filter(|(lane, messages)| **lane != source && messages.is_empty())
            .count();
        let usable_capacity = self.capacity.saturating_sub(empty_other_lanes);
        if state.len >= usable_capacity {
            return Err(FairV2IngressPushError::Full);
        }
        let lane = state
            .lanes
            .get_mut(&source)
            .expect("configured fair ingress always contains the classified source lane");
        let was_empty = lane.is_empty();
        lane.push_back(inbound);
        state.len += 1;
        if was_empty {
            state.ready.push_back(source);
        }
        Ok(())
    }

    /// Pop one conditionally admitted message while rotating its source.
    ///
    /// The predicate executes while the ingress state is locked. The method
    /// examines at most one head from every ready source, rotating a rejected
    /// source without removing its message. This lets an admissible Progress
    /// message bypass a Normal head whose downstream prefix is full, while the
    /// blocked message remains in its original lane for a later fair round.
    /// When every head is rejected, one complete rotation restores the
    /// original source order and total length.
    pub(crate) fn try_recv_if(
        &self,
        mut predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Option<InboundBlockMessage> {
        let mut state = self.state.lock();
        let ready_sources = state.ready.len();
        for _ in 0..ready_sources {
            let source = state
                .ready
                .pop_front()
                .expect("snapshotted ready source must remain queued");
            let admitted = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.front())
                .is_some_and(&mut predicate);
            if !admitted {
                state.ready.push_back(source);
                continue;
            }
            let lane = state
                .lanes
                .get_mut(&source)
                .expect("ready fair-ingress source must own a configured lane");
            let inbound = lane
                .pop_front()
                .expect("ready fair-ingress source must have a queued message");
            let remains_ready = !lane.is_empty();
            state.len -= 1;
            if remains_ready {
                state.ready.push_back(source);
            }
            return Some(inbound);
        }
        None
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
                Ok(()) => {
                    status::record_worker_queue_enqueue(queue);
                    self.wake();
                    true
                }
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
    let block = Arc::new(FairV2Ingress::new(block_capacity));
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
        let lane_relay_channel_cap = config.queues.ready_bodies.get();
        let (block_payload_tx, block_payload_rx) = mpsc::sync_channel(block_payload_channel_cap);
        let block = Arc::new(FairV2Ingress::new(block_channel_cap));
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
    use std::sync::atomic::Ordering;

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::consensus_v2 as wire,
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
    };

    use super::{BlockMessage, test_sumeragi_handle};

    fn v2_message() -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-ingress-test")),
                index: 0,
                bytes: vec![0xA5],
                sender: 0,
                signature: vec![0x5A],
            }),
        ))
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
            !handle.incoming_block_message(v2_message()),
            "a saturated v2 queue must reject promptly and rely on retransmission"
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let validators = validator_peers(4);
        let attacker = validators[0].clone();
        let outsider = validator_peers(5).pop().expect("outsider fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validators plus untrusted lane fit");
        ingress.open().expect("open configured roster");

        for _ in 0..4 {
            assert!(handle.try_incoming_block_message_from(attacker.clone(), v2_message()));
        }
        assert!(
            !handle.try_incoming_block_message_from(attacker.clone(), v2_message()),
            "attacker cannot consume slots reserved for empty validator and untrusted lanes"
        );
        for honest in validators.iter().skip(1) {
            assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));
        }
        assert!(handle.try_incoming_block_message_from(outsider.clone(), v2_message()));
        assert_eq!(ingress.len(), 8);

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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(3);
        let validators = validator_peers(2);
        let attacker = validators[0].clone();
        let honest = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators plus untrusted lane fit");
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
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(3);
        let validators = validator_peers(2);
        let blocked = validators[0].clone();
        let admissible = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators plus untrusted lane fit");
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
    fn anonymous_and_non_roster_v2_sources_share_one_bounded_lane() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(5);
        let validators = validator_peers(4);
        let outsider = validator_peers(5).pop().expect("outsider fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("minimum fair-lane capacity");
        ingress.open().expect("open configured roster");

        assert!(handle.try_incoming_block_message(v2_message()));
        assert!(
            !handle.try_incoming_block_message_from(outsider, v2_message()),
            "all untrusted identities share the same protected lane"
        );
        for validator in validators {
            assert!(handle.try_incoming_block_message_from(validator, v2_message()));
        }
        assert_eq!(ingress.len(), 5);
    }

    #[test]
    fn v2_ingress_rejects_capacity_smaller_than_roster_plus_untrusted_lane() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(4);
        ingress.close();
        let error = ingress
            .configure_roster(validator_peers(4))
            .expect_err("four validators require a fifth untrusted lane slot");
        assert_eq!(error.configured(), 4);
        assert_eq!(error.required(), 5);
        assert_eq!(ingress.open(), Err(error));
    }
}

impl SumeragiWorker {
    fn run(self) {
        v2_runner::run(self);
    }
}
