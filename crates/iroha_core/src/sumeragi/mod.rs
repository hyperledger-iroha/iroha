//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
use crate::{
    merge_sidecar::{CertifiedMergeSidecarMessage, MAX_CERTIFIED_MERGE_CHUNK_BYTES},
    state::{State, StateView, WorldReadOnly},
};
use eyre::Result;
use iroha_config::parameters::{
    actual::{Common as CommonConfig, Sumeragi as SumeragiConfig},
    defaults::{
        concurrency as concurrency_defaults,
        sumeragi::{
            BODY_ENVELOPE_HEADROOM_BYTES, CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            TIMEOUT_VOTE_RESERVE_BYTES,
        },
    },
};
use iroha_crypto::{Algorithm, Hash as CryptoHash, HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    block::{
        consensus::Evidence,
        consensus_v2::{
            BlockSubject, ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode,
            ConsensusRound,
        },
    },
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
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;
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
/// Build a deterministic exact network identity for protocol fixtures.
pub(crate) fn synthetic_network_id(seed: &str) -> NetworkId {
    NetworkId::from_genesis_hash(
        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(CryptoHash::new(
            seed.as_bytes(),
        )),
    )
}
#[cfg(test)]
mod thread_builder_tests {
    use super::{
        Algorithm, CONFIGURED_SUMERAGI_STACK_SIZE_BYTES, concurrency_defaults,
        is_bls_normal_public_key, normalized_sumeragi_stack_size_bytes,
        set_sumeragi_stack_size_bytes, sumeragi_stack_size_bytes, sumeragi_thread_builder,
    };
    use iroha_crypto::KeyPair;
    use std::sync::{Mutex, atomic::Ordering, mpsc};
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
/// Build the initial validator topology as the authenticated subset of trusted peers.
///
/// Every returned validator is a trusted peer with a BLS-normal key and an
/// explicit, valid proof of possession. An empty PoP map therefore yields an
/// empty validator roster, while PoPs for keys outside the trusted-peer set are
/// ignored. The result is deduplicated and canonically ordered by [`PeerId`].
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
    let mut validators = BTreeSet::new();
    let mut missing = 0usize;
    for peer_id in &baseline {
        let pk = peer_id.public_key();
        let Some(pop) = tp.pops.get(pk) else {
            missing = missing.saturating_add(1);
            continue;
        };
        if let Err(error) = iroha_crypto::bls_normal_pop_verify(pk, pop) {
            iroha_logger::warn!(?pk, ?error, "invalid PoP; excluding peer from consensus");
            continue;
        }
        validators.insert(peer_id.clone());
    }
    if missing > 0 {
        iroha_logger::info!(
            missing,
            baseline = baseline.len(),
            pops = tp.pops.len(),
            validators = validators.len(),
            "excluding trusted peers without validator PoPs from consensus roster"
        );
    }
    iroha_logger::info!(
        validators = validators.len(),
        configured_peers = tp.others.len().saturating_add(1),
        pops = tp.pops.len(),
        "resolved validator roster from trusted peers"
    );
    validators.into_iter().collect()
}

#[cfg(test)]
mod validator_pop_filter_tests {
    use super::filter_validators_from_trusted;
    use iroha_config::parameters::actual::TrustedPeers;
    use iroha_crypto::{Algorithm, KeyPair, PublicKey, bls_normal_pop_prove};
    use iroha_data_model::peer::{Peer, PeerId};
    use std::collections::BTreeMap;

    fn bls_key(seed: &[u8]) -> KeyPair {
        KeyPair::try_from_seed(seed.to_vec(), Algorithm::BlsNormal)
            .expect("derive BLS validator fixture")
    }

    fn peer(key: &KeyPair, port: u16) -> Peer {
        Peer::new(
            format!("127.0.0.1:{port}")
                .parse()
                .expect("fixture peer address"),
            key.public_key().clone(),
        )
    }

    fn trusted_peers(
        myself: &KeyPair,
        others: &[&KeyPair],
        pops: BTreeMap<PublicKey, Vec<u8>>,
    ) -> TrustedPeers {
        TrustedPeers {
            myself: peer(myself, 21_000),
            others: others
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    peer(
                        key,
                        21_001_u16
                            .checked_add(u16::try_from(index).expect("fixture peer index"))
                            .expect("fixture peer port"),
                    )
                })
                .collect(),
            pops,
        }
    }

    #[test]
    fn validator_filter_requires_explicit_pops_even_when_map_is_empty() {
        let local = bls_key(b"validator-filter-empty-local");
        let other = bls_key(b"validator-filter-empty-other");
        let trusted = trusted_peers(&local, &[&other], BTreeMap::new());

        assert!(filter_validators_from_trusted(&trusted).is_empty());
    }

    #[test]
    fn validator_filter_returns_only_trusted_bls_peers_with_valid_pops() {
        let local = bls_key(b"validator-filter-valid-local");
        let eligible = bls_key(b"validator-filter-valid-other");
        let missing = bls_key(b"validator-filter-missing-pop");
        let invalid = bls_key(b"validator-filter-invalid-pop");
        let observer = KeyPair::try_from_seed(
            b"validator-filter-ed25519-observer".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive non-validator fixture");
        let pop_only = bls_key(b"validator-filter-pop-only");
        let pops = BTreeMap::from([
            (
                local.public_key().clone(),
                bls_normal_pop_prove(local.private_key()).expect("local validator PoP"),
            ),
            (
                eligible.public_key().clone(),
                bls_normal_pop_prove(eligible.private_key()).expect("other validator PoP"),
            ),
            (invalid.public_key().clone(), Vec::new()),
            (
                pop_only.public_key().clone(),
                bls_normal_pop_prove(pop_only.private_key()).expect("untrusted key PoP"),
            ),
        ]);
        let trusted = trusted_peers(&local, &[&eligible, &missing, &invalid, &observer], pops);
        let mut expected = vec![
            PeerId::new(local.public_key().clone()),
            PeerId::new(eligible.public_key().clone()),
        ];
        expected.sort();

        assert_eq!(filter_validators_from_trusted(&trusted), expected);
    }

    #[test]
    fn validator_filter_never_synthesizes_pop_only_keys() {
        let local = bls_key(b"validator-filter-uncredentialed-local");
        let pop_only = bls_key(b"validator-filter-untrusted-pop");
        let trusted = trusted_peers(
            &local,
            &[],
            BTreeMap::from([(
                pop_only.public_key().clone(),
                bls_normal_pop_prove(pop_only.private_key()).expect("untrusted key PoP"),
            )]),
        );

        assert!(filter_validators_from_trusted(&trusted).is_empty());
    }
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
/// Resolve the signed on-chain delay before consensus-evidence penalties apply.
pub(crate) fn resolve_npos_slashing_delay_blocks_from_world(
    world: &impl WorldReadOnly,
) -> Option<u64> {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.slashing_delay_blocks())
}
/// Resolve the epoch index for a height under an authenticated frozen mode.
///
/// Permissioned consensus has one unbounded epoch and does not require NPoS
/// parameters. NPoS must derive its schedule from committed parameters; their
/// absence or invalidity is a consensus error rather than a default schedule.
pub(crate) fn epoch_for_height_from_world(
    world: &impl WorldReadOnly,
    height: u64,
    frozen_mode: ConsensusMode,
) -> Result<u64, v2_npos::V2NposError> {
    match frozen_mode {
        ConsensusMode::Permissioned => Ok(0),
        ConsensusMode::Npos => {
            let epoch_length = v2_npos::committed_epoch_length_blocks(world)?;
            Ok(height.saturating_sub(1) / epoch_length)
        }
    }
}
#[cfg(test)]
mod epoch_schedule_tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::parameter::{Parameter, system::SumeragiNposParameters};
    use std::num::NonZeroU64;

    #[test]
    fn npos_epoch_schedule_uses_committed_epoch_length() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut parameters = SumeragiNposParameters::default();
        parameters.epoch_length_blocks = NonZeroU64::new(7).expect("non-zero epoch length");
        parameters.evidence_horizon_blocks = 14;
        parameters.slashing_delay_blocks = 7;
        parameters
            .validate()
            .expect("test NPoS parameters must be internally consistent");
        {
            let mut block = state.world.parameters.block();
            block.set_parameter(Parameter::Custom(parameters.into_custom_parameter()));
            block.commit();
        }
        let world = state.world_view();
        assert_eq!(
            epoch_for_height_from_world(&world, 0, ConsensusMode::Npos).expect("valid schedule"),
            0
        );
        assert_eq!(
            epoch_for_height_from_world(&world, 1, ConsensusMode::Npos).expect("valid schedule"),
            0
        );
        assert_eq!(
            epoch_for_height_from_world(&world, 7, ConsensusMode::Npos).expect("valid schedule"),
            0
        );
        assert_eq!(
            epoch_for_height_from_world(&world, 8, ConsensusMode::Npos).expect("valid schedule"),
            1
        );
        assert_eq!(
            epoch_for_height_from_world(&world, 15, ConsensusMode::Npos).expect("valid schedule"),
            2
        );
    }

    #[test]
    fn permissioned_epoch_is_zero_without_npos_parameters_at_all_boundaries() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let world = state.world_view();
        for height in [0, 1, 3_600, 3_601, u64::MAX] {
            assert_eq!(
                epoch_for_height_from_world(&world, height, ConsensusMode::Permissioned)
                    .expect("permissioned mode does not require an NPoS schedule"),
                0
            );
        }
    }

    #[test]
    fn npos_epoch_rejects_missing_committed_parameters() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let world = state.world_view();
        assert!(matches!(
            epoch_for_height_from_world(&world, 1, ConsensusMode::Npos),
            Err(v2_npos::V2NposError::MissingCommittedParameters)
        ));
    }
}
/// QC-based consensus message types and helpers (single-chain).
pub mod consensus;
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
// Certified-Serve durability belongs to the production lifecycle coordinator and ledger.
pub(crate) mod v2_certified_serve_payload_store;
pub(crate) mod v2_chunks;
pub(crate) mod v2_context;
pub(crate) mod v2_context_store;
pub(crate) mod v2_core;
pub use v2_context::{
    GenesisV2Bootstrap, V2GenesisBootstrapError, freeze_staged_genesis_v2,
    signed_genesis_validator_pops, signed_genesis_voting_peers,
    staged_genesis_execution_policy_hash, staged_genesis_nexus_amx_context_hash,
    validate_signed_genesis_v2_authority,
};
pub use v2_core::{
    CheckedProductionTransition, ProductionTwoStageRelayRetryTraceProjection,
    check_production_two_stage_relay_retry_transition,
    production_two_stage_relay_retry_trace_refines_source_fairness_kernel,
};
pub(crate) mod v2_beacon;
pub(crate) mod v2_effects;
pub(crate) mod v2_first_release_recovery;
pub(crate) mod v2_lane_work;
pub(crate) mod v2_lifecycle_coordinator;
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
    evidence: &Evidence,
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
    /// Exact QueuePlan admission certificate handed to the current global leader.
    QueuePlanAdmissionCertificate {
        /// Authenticated transport sender.
        sender: PeerId,
        /// Exact canonical quorum-certificate bytes.
        certificate: Arc<Vec<u8>>,
    },
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
/// One normalized network-origin consensus message plus its authenticated identities.
#[derive(Clone, Debug)]
pub struct InboundBlockMessage {
    message: BlockMessage,
    /// Semantic protocol origin used for validation and response routing.
    sender: PeerId,
    /// Authenticated transport hop used exclusively for resource isolation.
    via: PeerId,
    /// Exact authenticated route which may carry a response to this occurrence.
    reply_routes: Option<NetworkReplyRoutes>,
    /// Process-local exact ownership evidence retained across fair admission.
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
}
impl InboundBlockMessage {
    /// Build one message delivered directly by an authenticated transport peer.
    pub(crate) fn from_authenticated_peer(message: BlockMessage, sender: PeerId) -> Self {
        Self {
            message,
            via: sender.clone(),
            sender,
            reply_routes: None,
            ingress_ownership: None,
        }
    }
    /// Build one transport message while preserving a relayed protocol origin.
    ///
    /// `sender` remains visible to consensus validation and response routing;
    /// `via` is the authenticated hop charged for every bounded ingress owner.
    #[cfg(test)]
    fn from_transport(message: BlockMessage, sender: PeerId, via: PeerId) -> Self {
        Self {
            message,
            sender,
            via,
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
            message,
            sender,
            via,
            reply_routes: Some(reply_routes),
            ingress_ownership: None,
        })
    }
    /// Consume the envelope and return the normalized message and semantic origin.
    #[cfg(test)]
    pub(crate) fn into_message_and_sender(self) -> (BlockMessage, PeerId) {
        (self.message, self.sender)
    }
    /// Consume the envelope without losing its local-only authenticated reply authority.
    pub(crate) fn into_message_sender_and_reply_routes(
        self,
    ) -> (BlockMessage, PeerId, Option<NetworkReplyRoutes>) {
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
    /// Borrow the authenticated semantic protocol origin.
    pub(crate) const fn sender(&self) -> &PeerId {
        &self.sender
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
    pub(crate) const fn via(&self) -> &PeerId {
        &self.via
    }
}
#[cfg(test)]
/// Generate an authenticated transport identity for an isolated ingress fixture.
pub(crate) fn authenticated_peer_for_test() -> PeerId {
    PeerId::from(iroha_crypto::KeyPair::random().public_key().clone())
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressSourceClass {
    Validator,
    Authenticated,
}
impl FairV2IngressSource {
    const fn class(&self) -> FairV2IngressSourceClass {
        match self {
            Self::Validator(_) => FairV2IngressSourceClass::Validator,
            Self::Authenticated(_) => FairV2IngressSourceClass::Authenticated,
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
    requires_leader_wire_lifecycle_gate: bool,
    leader_wire_lifecycle_gate: Option<Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>>,
    leader_wire_lifecycle_ordinals: Option<v2_runtime::RuntimeLifecycleOrdinalSource>,
    /// Genesis-derived network identity frozen by the active context setup.
    /// Generic test queues intentionally leave this absent.
    configured_network_id: Option<NetworkId>,
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
    class: FairV2IngressClass,
    wire_key: Option<FairV2IngressWireKey>,
    leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    /// Signature-authenticated request which can serve one lagging replica
    /// from immutable history without advancing or replacing a local owner.
    history_serve_request: Option<FairV2IngressHistoryServeRequest>,
    encoded_bytes: Arc<[u8]>,
    encoded_len: usize,
    /// Admission/coalescence-minted immutable ownership snapshot. Lifecycle
    /// queue cuts clone this `Arc` under the state mutex, then perform all
    /// validation and integrity hashing after releasing that mutex.
    ownership_snapshot: Arc<FairV2IngressOwnershipEvidence>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FairV2IngressHistoryServeRequest {
    height: u64,
    /// `CommitCertificateRequest` carries a signed network ID; the older
    /// `CertifiedBodyRequest` wire shape leaves this absent and relies on its
    /// historical context/QC validation at the service seam.
    required_network_id: Option<NetworkId>,
}
impl FairV2IngressHistoryServeRequest {
    const fn height(self) -> u64 {
        self.height
    }
    fn matches_configured_network(self, configured_network_id: Option<&NetworkId>) -> bool {
        self.required_network_id
            .is_none_or(|network_id| configured_network_id == Some(&network_id))
    }
}
include!("fair_v2_ingress_leader_wire_identity.rs");
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
    Coalesced, // Exact duplicate or WAL-obsolete control stutter.
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
    V2PayloadChunk,
    V2CertifiedBodyRequest,
    V2CertifiedBodyResponse,
    V2CommitCertificateRequest,
    V2CommitCertificateResponse,
    V2GlobalBeaconPartialSignature,
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
        ConsensusMessageV2Payload::PayloadChunk(_)
        | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return None,
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
            ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return None,
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
/// Whether timeout control can advance past the selected view-scoped owner's view.
///
/// A direct Vote may deliberately remain in fair ingress until its Proposal
/// binds the execution commitment. Requiring that blocked Vote to cross before
/// same-view timeout shares creates a circular dependency: those shares must
/// assemble the TC which retires the view's proposal and vote work. The same
/// cycle exists when one validator's already-counted TimeoutVote owns the
/// barrier, so another validator's exact-view share may cross it. An already
/// assembled TC for that view or a later one has the same dependency. A
/// manifest-bound Chunk is also view-scoped: timeout shares must be observable
/// to form the TC which retires a body whose downstream capacity is blocked.
/// Certified responses are request-scoped and deliberately remain excluded.
/// Every candidate still crosses normal downstream authentication and quorum
/// checks; this helper only allows the verifier to observe it when the
/// immutable owner is currently inadmissible.
fn fair_v2_ingress_timeout_control_advances_owner(
    owner: &FairV2IngressLeaderWireToken,
    candidate: Option<&FairV2IngressLeaderWireToken>,
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
            | FairV2IngressLeaderWirePhase::Chunk
    ) {
        return false;
    }
    let BlockMessage::V2(message) = inbound.message() else {
        return false;
    };
    let (round, view_advances) = match &message.payload {
        ConsensusMessageV2Payload::TimeoutVote(vote) => (
            vote.round,
            if owner.identity.phase == FairV2IngressLeaderWirePhase::TimeoutVote {
                candidate.is_some_and(|candidate| {
                    candidate.identity.phase == FairV2IngressLeaderWirePhase::TimeoutVote
                        && candidate.slot.phase == FairV2IngressLeaderWirePhase::TimeoutVote
                        && candidate.source_class == FairV2IngressLeaderWireSourceClass::Control
                        && candidate.slot.chunk_index.is_none()
                        && candidate.identity.context_id == vote.round.context_id
                        && candidate.identity.height == vote.round.height
                        && candidate.identity.view == vote.round.view
                        && candidate.identity.view == owner.identity.view
                        && candidate.identity.semantic_origin == *inbound.sender()
                        && candidate.slot.semantic_origin == candidate.identity.semantic_origin
                        && candidate.identity.semantic_origin != owner.identity.semantic_origin
                        && candidate.identity.canonical_wire_hash
                            == CryptoHash::new(message.encode())
                })
            } else {
                v2_core::timeout_vote_view_is_admissible(owner.identity.view, vote.round.view)
            },
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
/// Whether a certified reducer input can advance the selected productive leader-wire owner.
///
/// Fair ingress observes only authenticated transport provenance at this
/// point; the reducer still verifies the certificate and sender before any
/// state transition. This dependency edge merely prevents a retained
/// Proposal, vote, or body owner from hiding the TC or CommitQC that can
/// advance it. Exact context, height, and nondecreasing-view checks keep the
/// escape scoped to the owner's own consensus incarnation.
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
/// Pre-authenticate a request which can release a lagging replica from Kura.
///
/// Signature work happens once before the fair-ingress state lock is taken.
/// This projection grants only dependency scheduling: the ordinary service
/// seam still validates the historical context, QC, canonical subject/body,
/// and immutable Kura artifact before emitting any response.
fn fair_v2_ingress_history_serve_request(
    inbound: &InboundBlockMessage,
) -> Option<FairV2IngressHistoryServeRequest> {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;
    let BlockMessage::V2(message) = inbound.message() else {
        return None;
    };
    if message.validate_version().is_err() {
        return None;
    }
    match &message.payload {
        ConsensusMessageV2Payload::CertifiedBodyRequest(request)
            if request.certificate.proposal_round == request.round
                && request.certificate.subject == request.subject
                && v2_transport::authenticate_certified_body_request_identity(
                    request,
                    inbound.sender(),
                )
                .is_ok() =>
        {
            Some(FairV2IngressHistoryServeRequest {
                height: request.round.height,
                required_network_id: None,
            })
        }
        ConsensusMessageV2Payload::CommitCertificateRequest(request)
            if v2_transport::authenticate_commit_certificate_request_identity(
                request,
                inbound.sender(),
            )
            .is_ok() =>
        {
            Some(FairV2IngressHistoryServeRequest {
                height: request.height,
                required_network_id: Some(request.network_id),
            })
        }
        ConsensusMessageV2Payload::Proposal(_)
        | ConsensusMessageV2Payload::Vote(_)
        | ConsensusMessageV2Payload::QuorumCertificate(_)
        | ConsensusMessageV2Payload::TimeoutVote(_)
        | ConsensusMessageV2Payload::TimeoutCertificate(_)
        | ConsensusMessageV2Payload::PayloadChunk(_)
        | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => None,
    }
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
        ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
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
        return Ok(FairV2IngressLeaderWireAdmission::Coalesced);
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
        let same_round_timeout_upgrade = identity.phase
            == FairV2IngressLeaderWirePhase::TimeoutCertificate
            && incumbent.token.identity.phase == FairV2IngressLeaderWirePhase::TimeoutCertificate
            && identity.context_id == incumbent.token.identity.context_id
            && identity.height == incumbent.token.identity.height
            && identity.view == incumbent.token.identity.view;
        if incumbent.status.blocks_replacement() {
            return Err(
                if identity.view > incumbent.token.identity.view || same_round_timeout_upgrade {
                    FairV2IngressLeaderWireAdmissionError::Busy
                } else {
                    FairV2IngressLeaderWireAdmissionError::Rejected
                },
            );
        }
        if identity.context_id != incumbent.token.identity.context_id
            || identity.height != incumbent.token.identity.height
            || (identity.view <= incumbent.token.identity.view && !same_round_timeout_upgrade)
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
    const fn projection_code(self) -> u8 {
        match self {
            Self::V2Proposal => 0,
            Self::V2Vote => 1,
            Self::V2QuorumCertificate => 2,
            Self::V2TimeoutVote => 3,
            Self::V2TimeoutCertificate => 4,
            Self::V2PayloadChunk => 5,
            Self::V2CertifiedBodyRequest => 6,
            Self::V2CertifiedBodyResponse => 7,
            Self::V2CommitCertificateRequest => 8,
            Self::V2CommitCertificateResponse => 9,
            Self::KuraReplicaAdvert => 10,
            Self::LaneBlockProposal => 11,
            Self::LaneExecutablePayload => 12,
            Self::LaneBlockNewViewVote => 13,
            Self::LaneBlockNewViewCertificate => 14,
            Self::LaneBlockVote => 15,
            Self::LaneBlockQc => 16,
            Self::LaneBlockCertificate => 17,
            Self::LaneHistoricalRecoveryRequest => 18,
            Self::LaneHistoricalRecoveryResponse => 19,
            Self::V2GlobalBeaconPartialSignature => 20,
        }
    }
    fn classify(message: &BlockMessage) -> Option<Self> {
        use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;
        match message {
            BlockMessage::V2(message) => Some(match &message.payload {
                ConsensusMessageV2Payload::Proposal(_) => Self::V2Proposal,
                ConsensusMessageV2Payload::Vote(_) => Self::V2Vote,
                ConsensusMessageV2Payload::QuorumCertificate(_) => Self::V2QuorumCertificate,
                ConsensusMessageV2Payload::TimeoutVote(_) => Self::V2TimeoutVote,
                ConsensusMessageV2Payload::TimeoutCertificate(_) => Self::V2TimeoutCertificate,
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
                ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
                    Self::V2GlobalBeaconPartialSignature
                }
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
                | Self::V2PayloadChunk
                | Self::V2CertifiedBodyRequest
                | Self::V2CertifiedBodyResponse
                | Self::V2CommitCertificateRequest
                | Self::V2CommitCertificateResponse
                | Self::V2GlobalBeaconPartialSignature
        )
    }
}
#[cfg(test)]
#[test]
fn fair_v2_ingress_projection_codes_are_dense() {
    let kinds = [
        FairV2IngressMessageKind::V2Proposal,
        FairV2IngressMessageKind::V2Vote,
        FairV2IngressMessageKind::V2QuorumCertificate,
        FairV2IngressMessageKind::V2TimeoutVote,
        FairV2IngressMessageKind::V2TimeoutCertificate,
        FairV2IngressMessageKind::V2PayloadChunk,
        FairV2IngressMessageKind::V2CertifiedBodyRequest,
        FairV2IngressMessageKind::V2CertifiedBodyResponse,
        FairV2IngressMessageKind::V2CommitCertificateRequest,
        FairV2IngressMessageKind::V2CommitCertificateResponse,
        FairV2IngressMessageKind::KuraReplicaAdvert,
        FairV2IngressMessageKind::LaneBlockProposal,
        FairV2IngressMessageKind::LaneExecutablePayload,
        FairV2IngressMessageKind::LaneBlockNewViewVote,
        FairV2IngressMessageKind::LaneBlockNewViewCertificate,
        FairV2IngressMessageKind::LaneBlockVote,
        FairV2IngressMessageKind::LaneBlockQc,
        FairV2IngressMessageKind::LaneBlockCertificate,
        FairV2IngressMessageKind::LaneHistoricalRecoveryRequest,
        FairV2IngressMessageKind::LaneHistoricalRecoveryResponse,
        FairV2IngressMessageKind::V2GlobalBeaconPartialSignature,
    ];
    for (expected, kind) in (0_u8..).zip(kinds) {
        assert_eq!(kind.projection_code(), expected);
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
        ConsensusMessageV2Payload::CertifiedBodyRequest(request) => Some(request.round),
        ConsensusMessageV2Payload::CertifiedBodyResponse(response) => Some(response.manifest.round),
        ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            Some(response.certificate.round)
        }
        ConsensusMessageV2Payload::GlobalBeaconPartialSignature(partial) => Some(partial.round),
        ConsensusMessageV2Payload::PayloadChunk(_)
        | ConsensusMessageV2Payload::CommitCertificateRequest(_) => None,
    }
}
#[cfg(test)]
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
    semantic_origin: PeerId,
    authenticated_via: PeerId,
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
    /// Production-gate exception for one authenticated current archive which
    /// is outside the frozen validator roster and may answer only an exact
    /// outstanding certified-body request.
    ///
    /// This carrier deliberately owns no generic leader-wire lifecycle slot:
    /// the downstream request-family claim serializes it instead. The bit is
    /// minted only while the production gate is required and remains sealed
    /// into every ownership projection and merge.
    request_bound_non_roster_completion: bool,
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
fn fair_v2_ingress_append_source_identity(projection: &mut Vec<u8>, source: &FairV2IngressSource) {
    match source {
        FairV2IngressSource::Validator(peer) => {
            projection.push(0);
            fair_v2_ingress_append_peer_identity(projection, peer);
        }
        FairV2IngressSource::Authenticated(peer) => {
            projection.push(1);
            fair_v2_ingress_append_peer_identity(projection, peer);
        }
    }
}
impl FairV2IngressOwnershipEvidence {
    fn new(
        occurrence: FairV2IngressOwnershipOccurrence,
        leader_wire_token: Option<FairV2IngressLeaderWireToken>,
        request_bound_non_roster_completion: bool,
    ) -> Self {
        let mut action_counts = [0; FairV2IngressOwnershipAction::COUNT];
        action_counts[occurrence.action.index()] = 1;
        Self {
            leader_wire_token,
            leader_wire_runtime_receipt: None,
            request_bound_non_roster_completion,
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
            request_bound_non_roster_completion: self.request_bound_non_roster_completion,
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
            request_bound_non_roster_completion: self.request_bound_non_roster_completion,
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
            && self.request_bound_non_roster_completion == other.request_bound_non_roster_completion
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
    /// Whether production ingress sealed this exact certified response as a
    /// request-bound completion from outside the current frozen roster.
    pub(crate) const fn request_bound_non_roster_completion(&self) -> bool {
        self.request_bound_non_roster_completion
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
    pub(crate) fn matches_semantic_origin(&self, origin: &PeerId) -> bool {
        self.validate_exact() && &self.first.semantic_origin == origin
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
        projection.extend_from_slice(b"iroha:sumeragi:v2:fair-ingress-owner:v12");
        for occurrence in [&self.first, &self.latest] {
            projection.push(u8::try_from(occurrence.action.index()).unwrap_or(u8::MAX));
            projection.extend_from_slice(&occurrence.physical_admission_ordinal.to_le_bytes());
            match occurrence.lifecycle_ordinal {
                None => projection.push(0),
                Some(ordinal) => {
                    projection.push(1);
                    projection.extend_from_slice(&ordinal.to_le_bytes());
                }
            }
            fair_v2_ingress_append_peer_identity(&mut projection, &occurrence.semantic_origin);
            fair_v2_ingress_append_peer_identity(&mut projection, &occurrence.authenticated_via);
            projection.push(u8::from(occurrence.authenticated_via_is_validator));
            fair_v2_ingress_append_source_identity(
                &mut projection,
                &occurrence.authenticated_source,
            );
            fair_v2_ingress_append_source_identity(
                &mut projection,
                &occurrence.semantic_owner_source,
            );
            fair_v2_ingress_append_peer_identity(&mut projection, &occurrence.wire_key.origin);
            projection.extend_from_slice(occurrence.wire_key.hash.as_ref());
            projection.push(occurrence.message_kind.projection_code());
            projection.push(match occurrence.class {
                FairV2IngressClass::Auxiliary => 0,
                FairV2IngressClass::Progress => 1,
                FairV2IngressClass::TransportCompletion => 2,
            });
            projection.extend_from_slice(
                &u64::try_from(occurrence.encoded_len)
                    .expect("bounded ingress encoding length fits u64")
                    .to_le_bytes(),
            );
            fair_v2_ingress_append_resource_projection(
                &mut projection,
                &occurrence.resource_before,
            );
            fair_v2_ingress_append_resource_projection(&mut projection, &occurrence.resource_after);
            fair_v2_ingress_append_routes_projection(
                &mut projection,
                occurrence.routes_before.as_ref(),
            );
            fair_v2_ingress_append_routes_projection(
                &mut projection,
                occurrence.routes_candidate.as_ref(),
            );
            fair_v2_ingress_append_routes_projection(
                &mut projection,
                occurrence.routes_after.as_ref(),
            );
            match occurrence.route_capacity {
                None => projection.push(0),
                Some(capacity) => {
                    projection.push(1);
                    projection.extend_from_slice(&capacity.to_le_bytes());
                }
            }
            fair_v2_ingress_append_attempt_projection(&mut projection, &occurrence.attempts_before);
            projection.extend_from_slice(occurrence.attempts_before_hash.as_ref());
            fair_v2_ingress_append_attempt_projection(&mut projection, &occurrence.attempts_after);
            projection.extend_from_slice(occurrence.attempts_after_hash.as_ref());
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
        projection.push(u8::from(self.request_bound_non_roster_completion));
        // `validate_exact` proves this digest and length are the canonical
        // encoding of `first`. Reusing that admission-time seal keeps every
        // later ownership projection bounded independently of body size.
        projection.extend_from_slice(self.first.wire_key.hash.as_ref());
        projection.extend_from_slice(
            &u64::try_from(self.first.encoded_len)
                .expect("bounded ingress encoding length fits u64")
                .to_le_bytes(),
        );
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
                token.identity.semantic_origin == self.first.semantic_origin
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
            && (!self.request_bound_non_roster_completion
                || (self.first.message_kind == FairV2IngressMessageKind::V2CertifiedBodyResponse
                    && self.first.class == FairV2IngressClass::TransportCompletion
                    && self.leader_wire_token.is_none()
                    && self.leader_wire_runtime_receipt.is_none()))
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
fn fair_v2_ingress_append_resource_projection(
    projection: &mut Vec<u8>,
    resource: &FairV2IngressResourceSnapshot,
) {
    for value in [
        resource.source_len,
        resource.source_progress_len,
        resource.source_certified_fence_escape_len,
        resource.source_timeout_vote_len,
        resource.source_transport_completion_len,
        resource.source_bytes,
        resource.source_certified_fence_escape_bytes,
        resource.source_timeout_vote_bytes,
        resource.source_transport_completion_bytes,
        resource.global_len,
        resource.global_bytes,
        resource.protected_slots,
        resource.message_capacity,
        resource.global_byte_capacity,
        resource.source_byte_capacity,
        resource.certified_fence_escape_byte_reserve,
        resource.timeout_vote_byte_reserve,
        resource.transport_completion_byte_reserve,
    ] {
        projection.extend_from_slice(&value.to_le_bytes());
    }
}
fn fair_v2_ingress_append_routes_projection(
    projection: &mut Vec<u8>,
    routes: Option<&NetworkReplyRoutes>,
) {
    match routes {
        None => projection.push(0),
        Some(routes) => {
            projection.push(1);
            projection.extend_from_slice(&routes.source_capacity().to_le_bytes());
            projection.extend_from_slice(&routes.len().to_le_bytes());
            projection.extend_from_slice(routes.process_local_exact_history_hash().as_ref());
        }
    }
}
fn fair_v2_ingress_append_attempt_projection(
    projection: &mut Vec<u8>,
    attempts: &[FairV2IngressReplyAttempt],
) {
    projection.extend_from_slice(&attempts.len().to_le_bytes());
    for attempt in attempts {
        projection.extend_from_slice(attempt.route.process_local_identity_hash().as_ref());
        projection.extend_from_slice(&attempt.message_cursor.to_le_bytes());
        projection.extend_from_slice(&attempt.chunk_cursor.to_le_bytes());
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
        let uses_certified_fence_escape_reserve = is_certified_fence_escape;
        let semantic_exact = self.wire_key.origin == self.semantic_origin
            && self.wire_key.hash == CryptoHash::new(self.encoded_bytes.as_ref())
            && self.encoded_len == self.encoded_bytes.len()
            && cursor.is_empty()
            && decoded_class == self.class
            && decoded_kind == Some(self.message_kind);
        let source_exact = match &self.authenticated_source {
            FairV2IngressSource::Validator(source) => {
                self.authenticated_via_is_validator && source == &self.authenticated_via
            }
            FairV2IngressSource::Authenticated(source) => {
                !self.authenticated_via_is_validator && source == &self.authenticated_via
            }
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
        let routes_bind_semantic_origin = self
            .routes_before
            .iter()
            .chain(self.routes_candidate.iter())
            .chain(self.routes_after.iter())
            .all(|routes| routes.semantic_target() == &self.semantic_origin);
        let candidate_binds_authenticated_via = self
            .routes_candidate
            .iter()
            .flat_map(|routes| routes.iter())
            .all(|route| route.is_authenticated_via(&self.authenticated_via));
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
            | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => Self::Progress,
            ConsensusMessageV2Payload::PayloadChunk(_)
            | ConsensusMessageV2Payload::CertifiedBodyResponse(_) => Self::TransportCompletion,
            ConsensusMessageV2Payload::Proposal(_) | ConsensusMessageV2Payload::Vote(_) => {
                Self::Auxiliary
            }
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
    if authenticated_non_validator_source_capacity.is_none() {
        return 0;
    }
    let materialized = state
        .lanes
        .iter()
        .map(|(source, lane)| {
            fair_v2_ingress_lane_protected_slots(
                source.class(),
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

/// Scalar-only record of the most recent composite fair-selector attempt.
/// It retains no peer, message, predicate, queue cut, or lifecycle digest.
#[derive(Clone, Copy, Debug)]
struct FairV2IngressSelectorAttemptDiagnosticV1 {
    observed_at: Instant,
    physical_cut: u128,
    depth: usize,
    gate_blocked: usize,
    gate_strict: usize,
    gate_dependency: usize,
    predicate_tested: usize,
    predicate_rejected: usize,
    selected: bool,
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
        return roster_len.checked_mul(5);
    };
    roster_len.checked_mul(5).and_then(|required| {
        authenticated_non_validator_source_capacity
            .checked_mul(3)
            .and_then(|authenticated_sources| required.checked_add(authenticated_sources))
    })
}
fn fair_v2_ingress_reserve_ordinary_lifecycle_ordinal(
    state: &FairV2IngressState,
) -> Result<Option<u128>, String> {
    if let Some(source) = state.leader_wire_lifecycle_ordinals.as_ref() {
        return source.reserve_one().map(Some);
    }
    if state.requires_leader_wire_lifecycle_gate {
        return Err("production fair ingress lost its lifecycle ordinal source".to_owned());
    }
    Ok(None)
}
const fn fair_v2_ingress_lane_protected_slots(
    source_class: FairV2IngressSourceClass,
    depth: usize,
    has_ordinary_progress: bool,
    has_certified_fence_escape: bool,
    has_timeout_vote: bool,
    has_transport_completion: bool,
) -> usize {
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
/// Exact bare-Norito bytes of a structurally maximal execution commitment.
///
/// The maximum carries the bounded top-up, Native AMX, lane-finality, and
/// merge-carrier projections. The canonical structural proposal fixture below
/// binds this allocation-free constant to the live wire codec.
const FAIR_V2_INGRESS_MAX_EXECUTION_COMMITMENT_BYTES: usize = 306;
fn fair_v2_ingress_required_quorum_certificate_bytes(roster_len: usize) -> Option<usize> {
    let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
    let signer_vector_bytes = roster_len.checked_mul(5)?.checked_add(8)?;
    let signature_vector_bytes = signature_bytes.checked_add(8)?;
    53_usize
        .checked_add(53)?
        .checked_add(5)?
        .checked_add(102)?
        .checked_add(fair_v2_ingress_framed_bytes(
            FAIR_V2_INGRESS_MAX_EXECUTION_COMMITMENT_BYTES,
        )?)?
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
/// Relay origin and direct target use the exact first-release BLS-normal node
/// identity geometry. The direct frame dominates broadcast. Arithmetic
/// failures fail closed as `usize::MAX`.
fn fair_v2_ingress_required_p2p_frame_bytes(consensus_envelope_bytes: usize) -> usize {
    let required = || -> Option<usize> {
        let network_message_bytes =
            fair_v2_ingress_network_message_bytes(consensus_envelope_bytes)?;
        Some(
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
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
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
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
/// responder; relay origin and target use the canonical BLS-normal node
/// geometry. Arithmetic failures map to `usize::MAX`, so context activation
/// fails closed.
fn fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes() -> usize {
    let required = || -> Option<usize> {
        let network_message_bytes =
            fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key(
                iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
            )?;
        Some(
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
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
    network_id: &NetworkId,
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
        let embedded_network_id_bytes = network_id.encode().len();
        let commit_certificate_request_bytes = 3_usize
            .checked_add(embedded_network_id_bytes)?
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
fn fair_v2_ingress_required_recovery_request_bytes(
    network_id: &NetworkId,
    roster_len: usize,
) -> usize {
    fair_v2_ingress_required_recovery_request_bytes_for_key(
        network_id,
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
/// Exact canonical-wire ceiling for either payload transport completion whose
/// certified-body responder key has `raw_key_bytes` algorithm-specific bytes.
///
/// The checked calculation mirrors bare Norito's fixed v1 layout with default
/// compact lengths. `F(x)` is one compact length prefix plus `x` payload
/// bytes. A `Vec<Hash>` is its eight-byte sequence count plus 33 bytes per
/// element: one compact element-length byte and the 32-byte hash. The numeric
/// constants are the exact maxima for the remaining bounded structural fields
/// (including a 256-byte consensus signature) at each nesting layer. Overflow
/// maps to `usize::MAX`, making height activation fail closed before ingress
/// opens. `raw_key_bytes` excludes the compact public-key algorithm tag.
fn fair_v2_ingress_required_transport_completion_bytes_for_key(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
    raw_key_bytes: usize,
) -> usize {
    let required = || -> Option<usize> {
        let signature_bytes = iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES;
        let payload_bytes = usize::try_from(layout.max_payload_size_bytes).ok()?;
        let chunk_bytes = usize::try_from(layout.chunk_size_bytes).ok()?;
        let manifest_bytes = fair_v2_ingress_required_manifest_bytes(layout)?;
        let encoded_body_bytes = payload_bytes.checked_add(8)?;
        let responder_bytes = fair_v2_ingress_embedded_peer_id_bytes(raw_key_bytes)?;
        let response_bytes = 33_usize
            .checked_add(fair_v2_ingress_framed_bytes(manifest_bytes)?)?
            .checked_add(fair_v2_ingress_framed_bytes(encoded_body_bytes)?)?
            .checked_add(responder_bytes)?
            .checked_add(fair_v2_ingress_framed_bytes(
                signature_bytes.checked_add(8)?,
            )?)?;
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
fn fair_v2_ingress_required_transport_completion_bytes(
    layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout,
) -> usize {
    fair_v2_ingress_required_transport_completion_bytes_for_key(
        layout,
        iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
    )
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

/// Closed selection scope for one checked fair-ingress dequeue.
enum FairV2IngressCheckedSelectionScope {
    /// Preserve the productive leader-wire barrier and ordinary dependency ordering.
    Ordinary,
    /// Admit only independent lane-local traffic under an authenticated lifecycle barrier.
    LifecycleLaneLocal {
        _permit: v2_runner::LifecycleBlockedOrdinaryLaneLocalIngressPermitV1,
    },
}

impl FairV2IngressCheckedSelectionScope {
    const fn is_lifecycle_lane_local(&self) -> bool {
        matches!(self, Self::LifecycleLaneLocal { .. })
    }
}
include!("fair_v2_ingress_selector.rs");
fn select_fair_v2_ingress_candidate<T>(
    candidates: &[Vec<T>],
    projection: impl Fn(&T) -> (u64, FairV2IngressQueueGateVerdict, bool),
    mut predicate: impl FnMut(&T) -> bool,
) -> Option<(usize, u64, FairV2IngressDequeueDisposition)> {
    for dependency_pass in [false, true] {
        for (source_index, source_candidates) in candidates.iter().enumerate() {
            for candidate in source_candidates {
                let (ordinal, gate, obsolete) = projection(candidate);
                let dependency = gate == FairV2IngressQueueGateVerdict::Dependency;
                if gate == FairV2IngressQueueGateVerdict::Blocked || dependency != dependency_pass {
                    continue;
                }
                if obsolete || predicate(candidate) {
                    let disposition = if obsolete {
                        FairV2IngressDequeueDisposition::RetireObsolete
                    } else {
                        FairV2IngressDequeueDisposition::Admit
                    };
                    return Some((source_index, ordinal, disposition));
                }
            }
        }
    }
    None
}

/// Fixed-capacity, roster-aware v2 ingress with per-hop admission and service fairness.
///
/// Every authenticated validator hop owns one protected source slot, one ordinary
/// progress slot, one certified-fence-escape slot, one distinct TimeoutVote slot,
/// and one transport-completion slot. Every authenticated non-validator hop
/// independently owns three slots: general work, certified fence escape, and
/// transport completion. Senderless messages have no ingress representation.
/// A current-roster completion forwarded by a non-validator
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
/// Certified-body requests remain ordinary predicate-selected queue entries.
/// Their authenticated request fence is owned by the lifecycle coordinator's
/// executor join, not by a second queue-local Serve barrier.
///
/// Non-empty lanes are serviced in round-robin order, so a source may use
/// otherwise idle capacity but cannot starve an honest validator's progress.
/// Canonical envelope hashes are computed before taking the shared queue lock,
/// so duplicate detection never compares whole bodies while holding that lock.
/// Canonical wire bytes are charged to fixed aggregate and per-source budgets.
/// Within each validator partition, ordinary traffic, certified fence escape,
/// TimeoutVote, and payload transport completion own disjoint byte regions.
/// Authenticated non-validator partitions isolate certified escape and
/// request-bound historical transport completion as well.
/// Lane-local control uses ordinary progress while atomic certificate recovery
/// owns its isolated certified reservation; exact
/// executable-payload and proof-carrying historical-recovery response bytes
/// share the completion reservation. Roster
/// installation succeeds only when the configured authenticated-source
/// geometry owns isolated byte partitions.
/// `CommitCertificateResponse` remains reducer-producing Progress and uses the
/// certified reservation only when it embeds a CommitQC.
pub(crate) struct FairV2Ingress {
    /// Process-local identity shared only with opaque prepared queue witnesses.
    queue_identity: Arc<()>,
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
    /// Excludes producer mutation only while an exact dequeue crosses durable
    /// lifecycle publication.
    ///
    /// Lock order is `service_lock`, then `producer_publication_lock`, then
    /// `state`. Producers acquire only `producer_publication_lock`, then
    /// `state`. Ordinary selector preparation deliberately does not acquire
    /// this fence, so a concurrent producer can still invalidate its
    /// pre-publication compare-and-swap witness.
    producer_publication_lock: Mutex<()>,
    state: Mutex<FairV2IngressState>,
    /// Published only after dropping queue state; readers follow the same
    /// state-then-diagnostic, never nested, lock order.
    last_selector_attempt: Mutex<Option<FairV2IngressSelectorAttemptDiagnosticV1>>,
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
        let lanes = BTreeMap::new();
        Self {
            queue_identity: Arc::new(()),
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
            producer_publication_lock: Mutex::new(()),
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
                requires_leader_wire_lifecycle_gate: false,
                leader_wire_lifecycle_gate: None,
                leader_wire_lifecycle_ordinals: None,
                configured_network_id: None,
                leader_wire_context: None,
                open: false,
            }),
            last_selector_attempt: Mutex::new(None),
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
                debug_assert_eq!(
                    lane.certified_fence_escape_len,
                    lane.entries
                        .iter()
                        .filter(|entry| fair_v2_ingress_is_certified_fence_escape(&entry.inbound))
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
                let actual_certified_fence_escape_bytes = lane
                    .entries
                    .iter()
                    .filter(|entry| fair_v2_ingress_is_certified_fence_escape(&entry.inbound))
                    .map(|entry| {
                        debug_assert!(
                            entry.encoded_len <= self.certified_fence_escape_byte_reserve
                        );
                        entry.encoded_len
                    })
                    .sum::<usize>();
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
                } else {
                    debug_assert!(matches!(source, FairV2IngressSource::Authenticated(_)));
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
    #[cfg(any(test, feature = "sumeragi-main-loop-tests"))]
    pub(crate) fn configure_roster(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
    ) -> Result<(), FairV2IngressCapacityError> {
        self.configure_roster_with_byte_requirements(roster, None, 0, 0, 0, 0, 0, 0, 0)
    }
    /// Install a frozen roster and validate every progress envelope against
    /// its ingress, topic-frame, and outbound encrypted-frame byte owner.
    pub(crate) fn configure_roster_for_context(
        &self,
        roster: impl IntoIterator<Item = PeerId>,
        network_id: &NetworkId,
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
            fair_v2_ingress_required_recovery_request_bytes(network_id, roster.len());
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
            Some(*network_id),
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
        configured_network_id: Option<NetworkId>,
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
        let _service_guard = self.service_lock.lock();
        let mut state = self.state.lock();
        state.open = false;
        state.roster = roster;
        state.lanes = lanes;
        state.pending_wire_owners.clear();
        state.leader_wire_lifecycles.clear();
        state.configured_network_id = configured_network_id;
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
                    && next.retires(&record.token))
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
    /// Seal one height, durably park its queued productive carriers, and detach.
    ///
    /// `service_lock` excludes a consumer whose predicate snapshot temporarily
    /// lives outside the state mutex. Closing under the state mutex then excludes
    /// every producer. Productive carriers are returned to Dormant before any
    /// volatile queue bytes disappear; auxiliary and future packets own no
    /// height lifecycle and may be retransmitted into the successor ingress.
    pub(crate) fn retire_leader_wire_lifecycle_gate(
        &self,
        gate: &Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    ) -> Result<(), String> {
        let _service_guard = self.service_lock.lock();
        let mut state = self.state.lock();
        state.open = false;
        let bound = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .ok_or_else(|| "leader-wire lifecycle gate was already unbound".to_owned())?;
        if !serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(&bound, gate) {
            return Err("leader-wire lifecycle gate changed per-height ownership".to_owned());
        }
        let (context_id, height) = state
            .leader_wire_context
            .ok_or_else(|| "leader-wire lifecycle gate lost its height context".to_owned())?;
        let mut carriers = BTreeMap::new();
        for entry in state.lanes.values().flat_map(|lane| lane.entries.iter()) {
            let Some(inbound_ownership) = entry.inbound.ingress_ownership() else {
                return Err("sealed leader-wire ingress lost queued ownership evidence".to_owned());
            };
            if !inbound_ownership.validate_exact()
                || !entry.ownership_snapshot.validate_exact()
                || entry.leader_wire_token.as_ref() != inbound_ownership.leader_wire_token()
                || entry.leader_wire_token.as_ref() != entry.ownership_snapshot.leader_wire_token()
            {
                return Err(
                    "sealed leader-wire ingress changed a queued ownership projection".to_owned(),
                );
            }
            let Some(token) = entry.leader_wire_token.as_ref() else {
                continue;
            };
            if token.identity.context_id != context_id
                || token.identity.height != height
                || carriers.insert(token.slot.clone(), token.clone()).is_some()
            {
                return Err(
                    "sealed leader-wire ingress changed its exact retiring carrier set".to_owned(),
                );
            }
        }
        let mirrored_ingress = state
            .leader_wire_lifecycles
            .iter()
            .filter_map(|(slot, record)| {
                (record.status == FairV2IngressLeaderWireStatus::Ingress)
                    .then(|| (slot.clone(), record.token.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if carriers != mirrored_ingress {
            return Err(
                "sealed leader-wire ingress disagreed with live carrier ownership".to_owned(),
            );
        }
        let retirement = bound.park_sealed_ingress(carriers)?;

        let empty_lanes = state
            .roster
            .iter()
            .cloned()
            .map(|peer| {
                (
                    FairV2IngressSource::Validator(peer),
                    FairV2IngressLane::default(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        state.lanes = empty_lanes;
        state.pending_wire_owners.clear();
        state.ready.clear();
        state.len = 0;
        state.bytes = 0;
        state.nonempty_since = None;
        state.last_service_attempt_at = None;
        state.leader_wire_lifecycles.clear();
        state.leader_wire_lifecycle_gate = None;
        state.leader_wire_lifecycle_ordinals = None;
        state.leader_wire_context = None;
        self.debug_assert_consistent(&state);
        retirement.complete();
        Ok(())
    }
    /// Open admission for the already-configured immutable height.
    pub(crate) fn open(&self) -> Result<(), FairV2IngressCapacityError> {
        let mut state = self.state.lock();
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
    /// Linearize one lane-relay transfer against the shared ingress close.
    ///
    /// The caller retains `value` when admission is already closed. Otherwise
    /// the nonblocking channel transfer completes while the same state mutex
    /// used by [`Self::close`] remains held, so the receiver owns a finite
    /// pre-close prefix after closure returns.
    fn try_with_open_lane_relay_admission<T, R>(
        &self,
        value: T,
        operation: impl FnOnce(T) -> R,
    ) -> Result<R, T> {
        let state = self.state.lock();
        if !state.open {
            return Err(value);
        }
        Ok(operation(value))
    }
    /// Prove that the closed physical ingress has no queued or in-flight owner.
    pub(crate) fn ensure_closed_drained_cut(&self) -> Result<(), String> {
        let _service_guard = self.service_lock.lock();
        let _publication_guard = self.producer_publication_lock.lock();
        let state = self.state.lock();
        if state.open {
            return Err("finalized ingress cut remained open".to_owned());
        }
        let has_live_leader_wire_owner = state.leader_wire_lifecycles.values().any(|record| {
            matches!(
                record.status,
                FairV2IngressLeaderWireStatus::Ingress | FairV2IngressLeaderWireStatus::Runtime
            )
        });
        let has_lane_physical_ownership = state.lanes.values().any(|lane| {
            !lane.entries.is_empty()
                || !lane.pending_wire.is_empty()
                || lane.progress_len != 0
                || lane.certified_fence_escape_len != 0
                || lane.timeout_vote_len != 0
                || lane.transport_completion_len != 0
                || lane.bytes != 0
                || lane.certified_fence_escape_bytes != 0
                || lane.timeout_vote_bytes != 0
                || lane.transport_completion_bytes != 0
        });
        if state.len != 0
            || state.bytes != 0
            || state.nonempty_since.is_some()
            || state.last_service_attempt_at.is_some()
            || !state.ready.is_empty()
            || !state.pending_wire_owners.is_empty()
            || has_lane_physical_ownership
            || has_live_leader_wire_owner
        {
            return Err("finalized ingress cut retained physical ownership".to_owned());
        }
        self.debug_assert_consistent(&state);
        Ok(())
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
        assert!(
            ownership.install_leader_wire_runtime_receipt(receipt),
            "the durable leader-wire receipt must bind its already validated staged ownership"
        );
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
                if inbound.sender() != &advert.keeper
                    || inbound.via() != &advert.keeper
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
        let history_serve_request = fair_v2_ingress_history_serve_request(&inbound);
        let wire_hash = CryptoHash::new(encoded.as_ref());
        // Delivery deduplication remains scoped to the semantic origin: two
        // requesters behind one trusted relay can require distinct responses.
        // Count, byte, and fair-service ownership below is instead charged to
        // the authenticated hop so origin churn cannot multiply resources.
        let wire_key = Some(FairV2IngressWireKey {
            origin: inbound.sender.clone(),
            hash: wire_hash,
        });
        // The exact lifecycle dequeue retains this fence from its final
        // pre-publication comparison through LedgerV1 fsync and physical
        // removal. Encoding and protocol-shape validation above remain
        // outside the fence; every queue-state mutation below is serialized
        // with that durable publication window.
        let _producer_publication_guard = self.producer_publication_lock.lock();
        let mut state = self.state.lock();
        if !state.open {
            return Err(FairV2IngressPushError::Closed(inbound));
        }
        let history_serve_request = history_serve_request.filter(|request| {
            request.matches_configured_network(state.configured_network_id.as_ref())
        });
        let source = if state.roster.contains(inbound.via()) {
            FairV2IngressSource::Validator(inbound.via.clone())
        } else {
            FairV2IngressSource::Authenticated(inbound.via.clone())
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
                // TimeoutVote is a one-way authenticated control carrier. A
                // retained transport retry can arrive after a newer delivery
                // of the exact same wire bytes has already acquired this
                // queue owner, but its reply route grants no authority that
                // consensus will consume. Coalesce only that single-route
                // stale retry and leave the newer route and ownership evidence
                // unchanged. Response-capable families and every other route
                // error remain fail-closed below.
                Err(NetworkReplyRouteError::Stale)
                    if is_timeout_vote
                        && routes_candidate
                            .as_ref()
                            .is_some_and(|routes| routes.len() == 1) =>
                {
                    return Ok(FairV2IngressPushDisposition::Coalesced);
                }
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
            let ownership_snapshot = Arc::new(evidence.clone());
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
            queued.ownership_snapshot = ownership_snapshot;
            return Ok(FairV2IngressPushDisposition::Coalesced);
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
        let uses_certified_fence_escape_reserve =
            fair_v2_ingress_is_certified_fence_escape(&inbound);
        let is_current_validator_origin = state.roster.contains(inbound.sender());
        // Ordinary transport completions are protocol-valid only for a
        // current frozen-roster semantic origin. Request-bound historical
        // responses are the narrow exceptions: the lane adapter verifies the
        // frozen CommitQC or READY certificate, while certified-body handling
        // verifies the exact request/QC/manifest/body binding and the current
        // responder signature before persistence. Keep those responses in the
        // bounded authenticated completion partition without granting
        // non-roster peers current-height PayloadChunk authority.
        let authenticated_request_bound_response = matches!(
            message_kind,
            FairV2IngressMessageKind::LaneHistoricalRecoveryResponse
                | FairV2IngressMessageKind::V2CertifiedBodyResponse
        );
        // A current archive outside the frozen roster cannot consume the
        // roster-sized generic leader-wire lifecycle. Seal that fact into the
        // physical ownership only when the production gate is mandatory; the
        // certified-Fetch selector must later prove an exact outstanding
        // request before this carrier may complete.
        let request_bound_non_roster_completion = state.requires_leader_wire_lifecycle_gate
            && message_kind == FairV2IngressMessageKind::V2CertifiedBodyResponse
            && !is_current_validator_origin;
        if is_transport_completion
            && !is_current_validator_origin
            && !authenticated_request_bound_response
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
        } else {
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
            && !request_bound_non_roster_completion
        {
            let semantic_origin = inbound.sender().clone();
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
        // Every authenticated source has one source-isolated payload-completion owner.
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
        let protected_slots_after_admission = self
            .authenticated_non_validator_source_capacity
            .map_or(0, |_| {
                state
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
                            projected_len,
                            projected_ordinary_progress_len != 0,
                            projected_certified_fence_escape_len != 0,
                            projected_timeout_vote_len != 0,
                            projected_transport_completion_len != 0,
                        )
                    })
                    .sum::<usize>()
            });
        let Some(protected_slots_after_admission) = protected_slots_after_admission.checked_add(
            if self.authenticated_non_validator_source_capacity.is_some() && source_lane_is_new {
                fair_v2_ingress_lane_protected_slots(
                    source.class(),
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
            },
        ) else {
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
        if let Some(request) = certified_request.as_ref() {
            if matches!(
                inbound.message(),
                BlockMessage::V2(message) if message.validate_version().is_err()
            ) {
                reject_after_leader_wire_admission!();
            }
            if inbound.sender() != &request.requester {
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
        }
        occurrence.lifecycle_ordinal = if let Some(token) = leader_wire_token.as_ref() {
            Some(token.scheduler_ordinal())
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
        let ingress_ownership = FairV2IngressOwnershipEvidence::new(
            occurrence,
            leader_wire_token.clone(),
            request_bound_non_roster_completion,
        );
        let ownership_snapshot = Arc::new(ingress_ownership.clone());
        inbound.ingress_ownership = Some(ingress_ownership);
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
            class,
            wire_key,
            leader_wire_token,
            history_serve_request,
            encoded_bytes: encoded,
            encoded_len,
            ownership_snapshot,
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
    /// that request. Certified-body requests own no queue-local Serve barrier;
    /// their exact authenticated request fence is enforced by the lifecycle
    /// coordinator's executor join. Once a blocked entry becomes admissible,
    /// the head-first search selects it before later entries. When every entry
    /// is rejected, the source order and total length remain unchanged.
    #[cfg(any(test, feature = "sumeragi-main-loop-tests"))]
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
    /// Dequeue one exact lane-local occurrence while lifecycle ownership blocks ordinary ingress.
    ///
    /// The sealed permit grants no global leader-wire authority. This path still validates the
    /// durable leader-wire census, but lane-local traffic is selected independently of its global
    /// ingress barrier and committed through the ordinary ownership/accounting tail.
    pub(in crate::sumeragi) fn try_recv_lifecycle_lane_local_checked(
        &self,
        permit: v2_runner::LifecycleBlockedOrdinaryLaneLocalIngressPermitV1,
    ) -> Result<Option<InboundBlockMessage>, String> {
        self.try_recv_if_at_checked_classified(
            Instant::now(),
            false,
            FairV2IngressCheckedSelectionScope::LifecycleLaneLocal { _permit: permit },
            |inbound| inbound.message().is_lane_local(),
        )
        .map(|selected| selected.map(|(inbound, _)| inbound))
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
            FairV2IngressCheckedSelectionScope::Ordinary,
            predicate,
        )
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
            FairV2IngressCheckedSelectionScope::Ordinary,
            predicate,
        )
        .map(|selected| selected.map(|(inbound, _)| inbound))
    }
    fn try_recv_if_at_checked_classified(
        &self,
        service_attempt_at: Instant,
        retire_obsolete_leader_wire: bool,
        selection_scope: FairV2IngressCheckedSelectionScope,
        mut predicate: impl FnMut(&InboundBlockMessage) -> bool,
    ) -> Result<Option<(InboundBlockMessage, FairV2IngressDequeueDisposition)>, String> {
        let _service_guard = self.service_lock.lock();
        let lifecycle_lane_local = selection_scope.is_lifecycle_lane_local();
        let (ready_sources, candidates) = {
            let mut state = self.state.lock();
            if state.len != 0 {
                // A rejected scan is still proof that the outer runner
                // scheduler reached this queue. Downstream admission owns any
                // remaining delay; queue age alone does not establish
                // scheduler starvation.
                state.last_service_attempt_at = Some(service_attempt_at);
            }
            let leader_wire_projection = fair_v2_ingress_leader_wire_selector_projection(
                &state,
                retire_obsolete_leader_wire,
                None,
            )?;
            let obsolete_leader_wire_tokens = &leader_wire_projection.obsolete_tokens;
            let ready_sources = state.ready.iter().cloned().collect::<Vec<_>>();
            let candidates = ready_sources
                .iter()
                .map(|source| {
                    state
                        .lanes
                        .get(source)
                        .into_iter()
                        .flat_map(|lane| {
                            lane.entries.iter().enumerate().map(|(index, entry)| {
                                let verdict = if selection_scope.is_lifecycle_lane_local() {
                                    if entry.inbound.message().is_lane_local() {
                                        FairV2IngressQueueGateVerdict::Dependency
                                    } else {
                                        FairV2IngressQueueGateVerdict::Blocked
                                    }
                                } else {
                                    fair_v2_ingress_queue_gate_verdict(
                                        source,
                                        lane,
                                        index,
                                        &leader_wire_projection,
                                    )
                                };
                                (
                                    entry.admission_ordinal,
                                    Arc::clone(&entry.inbound),
                                    verdict,
                                    entry.leader_wire_token.as_ref().is_some_and(|token| {
                                        obsolete_leader_wire_tokens.contains(token)
                                    }),
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            (ready_sources, candidates)
        };
        // Preserve the durable physical prefix whenever its selected owner is
        // currently admissible. Only after downstream admission rejects that
        // entire strict set may a dependency cross the control barrier.
        let selected = select_fair_v2_ingress_candidate(
            &candidates,
            |(admission_ordinal, _, gate, obsolete)| (*admission_ordinal, *gate, *obsolete),
            |(_, inbound, _, _)| predicate(inbound.as_ref()),
        );
        let Some((selected_source_index, admission_ordinal, disposition)) = selected else {
            return Ok(None);
        };
        drop(candidates);
        let mut state = self.state.lock();
        self.dequeue_selected_locked(
            &mut state,
            &ready_sources,
            selected_source_index,
            admission_ordinal,
            disposition,
            retire_obsolete_leader_wire,
            lifecycle_lane_local,
            service_attempt_at,
        )
        .map(Some)
    }
    /// Commit one already selected occurrence using the sole production
    /// durable-handoff, accounting, rotation, and physical-cut mutation tail.
    ///
    /// The caller must retain `service_lock` and the supplied state guard. All
    /// fallible ownership checks which can fail without publishing a durable
    /// transition run before the first mutation below.
    fn dequeue_selected_locked(
        &self,
        state: &mut FairV2IngressState,
        ready_sources: &[FairV2IngressSource],
        selected_source_index: usize,
        admission_ordinal: u64,
        mut disposition: FairV2IngressDequeueDisposition,
        retire_obsolete_leader_wire: bool,
        lifecycle_lane_local: bool,
        service_attempt_at: Instant,
    ) -> Result<(InboundBlockMessage, FairV2IngressDequeueDisposition), String> {
        let source = ready_sources
            .get(selected_source_index)
            .cloned()
            .ok_or_else(|| "selected fair-ingress source left its ready snapshot".to_owned())?;
        if !state
            .ready
            .iter()
            .take(ready_sources.len())
            .eq(ready_sources.iter())
        {
            return Err(
                "serialized fair-ingress service changed its snapshotted ready prefix".to_owned(),
            );
        }
        let admitted_index = state
            .lanes
            .get(&source)
            .and_then(|lane| {
                lane.entries
                    .iter()
                    .position(|entry| entry.admission_ordinal == admission_ordinal)
            })
            .ok_or_else(|| {
                "serialized fair-ingress candidate left its selected source lane".to_owned()
            })?;
        let mut staged_ownership = state
            .lanes
            .get(&source)
            .and_then(|lane| lane.entries.get(admitted_index))
            .and_then(|entry| entry.inbound.ingress_ownership.as_ref())
            .cloned()
            .ok_or_else(|| {
                "selected fair-ingress envelope lost its physical ownership evidence".to_owned()
            })?;
        if lifecycle_lane_local {
            let entry = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .expect("selected lifecycle lane-local entry was just resolved");
            if disposition != FairV2IngressDequeueDisposition::Admit
                || !entry.inbound.message().is_lane_local()
                || entry.leader_wire_token.is_some()
                || !staged_ownership.validate_exact()
                || !staged_ownership.matches_message(entry.inbound.message())
                || !staged_ownership.matches_semantic_origin(entry.inbound.sender())
                || !staged_ownership.matches_reply_routes(entry.inbound.reply_routes())
                || staged_ownership.leader_wire_token().is_some()
                || staged_ownership.leader_wire_runtime_receipt().is_some()
            {
                return Err(
                    "lifecycle lane-local dequeue crossed global leader-wire ownership".to_owned(),
                );
            }
        }
        let runtime_physical_cut = u128::from(state.last_admission_ordinal) + 1;
        if Arc::strong_count(
            &state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .expect("selected entry was just resolved")
                .inbound,
        ) != 1
            || staged_ownership.first.physical_admission_ordinal != admission_ordinal
            || staged_ownership.runtime_physical_cut.is_some()
            || runtime_physical_cut <= u128::from(admission_ordinal)
            || !staged_ownership.freeze_runtime_physical_cut(runtime_physical_cut)
        {
            return Err("selected fair-ingress ownership is not exclusively drainable".to_owned());
        }
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
        let has_leader_wire_ownership = {
            let entry = state
                .lanes
                .get(&source)
                .and_then(|lane| lane.entries.get(admitted_index))
                .expect("selected fair-ingress entry remains queued for runtime handoff");
            match entry.leader_wire_token.as_ref() {
                None => {
                    if staged_ownership.leader_wire_token().is_some()
                        || staged_ownership.leader_wire_runtime_receipt().is_some()
                    {
                        return Err(
                            "nonproductive dequeue carried leader-wire ownership".to_owned()
                        );
                    }
                    false
                }
                Some(token) => {
                    if staged_ownership.leader_wire_token() != Some(token)
                        || staged_ownership.leader_wire_runtime_receipt().is_some()
                    {
                        return Err(
                            "leader-wire dequeue changed its exact ingress ownership".to_owned()
                        );
                    }
                    true
                }
            }
        };
        if has_leader_wire_ownership {
            // Persist and install the deterministic runtime owner while the
            // physical carrier, durable Ingress record, and queue lock still
            // form one atomic handoff. Existing downstream bind calls then
            // validate this receipt idempotently.
            Self::bind_leader_wire_runtime_ownership_locked(state, &mut staged_ownership)?;
        }
        // Install only the already validated staged ownership after every
        // fallible durable transition. The remaining queue/accounting tail is
        // infallible under the exact structural checks above.
        let entry = state
            .lanes
            .get_mut(&source)
            .and_then(|lane| lane.entries.get_mut(admitted_index))
            .expect("selected fair-ingress entry remains queued through durable handoff");
        Arc::make_mut(&mut entry.inbound).ingress_ownership = Some(staged_ownership);
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
            if fair_v2_ingress_is_certified_fence_escape(&entry.inbound) {
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
        } else {
            state.last_service_attempt_at = Some(service_attempt_at);
        }
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
        self.debug_assert_consistent(state);
        let inbound = Arc::try_unwrap(entry.inbound)
            .expect("serialized fair-ingress service must own the selected envelope");
        let ownership = inbound
            .ingress_ownership
            .as_ref()
            .expect("validated staged ownership remains installed through dequeue");
        debug_assert_eq!(
            ownership.first.physical_admission_ordinal,
            entry.admission_ordinal
        );
        debug_assert_eq!(ownership.runtime_physical_cut, Some(runtime_physical_cut));
        Ok((inbound, disposition))
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
    /// Whether one exact physical carrier remains queued without a leader-wire handoff.
    #[cfg(test)]
    pub(crate) fn exact_queued_ungated_occurrence_for_test(
        &self,
        physical_admission_ordinal: u64,
    ) -> bool {
        let state = self.state.lock();
        let mut matches = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .filter(|entry| entry.admission_ordinal == physical_admission_ordinal);
        let Some(entry) = matches.next() else {
            return false;
        };
        if matches.next().is_some() || entry.leader_wire_token.is_some() {
            return false;
        }
        let Some(ownership) = entry.inbound.ingress_ownership() else {
            return false;
        };
        ownership.validate_exact()
            && entry.ownership_snapshot.same_semantic_request(ownership)
            && ownership.physical_admission_ordinal() == Some(physical_admission_ordinal)
            && ownership.leader_wire_token().is_none()
            && ownership.leader_wire_runtime_receipt().is_none()
            && ownership.matches_message(entry.inbound.message())
            && ownership.matches_semantic_origin(entry.inbound.sender())
            && ownership.matches_reply_routes(entry.inbound.reply_routes())
    }
}
/// Admit one test envelope through the real fair-ingress ownership seam.
#[cfg(test)]
pub(crate) fn fair_v2_ingress_admit_for_test(inbound: InboundBlockMessage) -> InboundBlockMessage {
    let roster = vec![inbound.via().clone()];
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
        512 * 1024 * 1024,
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
    pending_queue_plan_admission_dirty: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
    emergency_fast_disabled: bool,
}
impl SumeragiHandle {
    fn new(
        block: Arc<FairV2Ingress>,
        lane_relay: mpsc::SyncSender<LaneRelayMessage>,
        wake: mpsc::SyncSender<()>,
        ingress_ready: Arc<AtomicBool>,
        pending_queue_plan_admission_dirty: Arc<AtomicBool>,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            block,
            lane_relay,
            wake,
            ingress_ready,
            pending_queue_plan_admission_dirty,
            output_guard,
            emergency_fast_disabled: false,
        }
    }
    /// Construct a permanently closed consensus ingress without launching an
    /// OS thread or allocating production queue geometry.
    ///
    /// Emergency Fast mode is read-only for its entire process lifetime. The
    /// disabled marker terminally classifies every owned ingress as
    /// [`SumeragiIngressDisposition::Obsolete`] and prevents queue-plan wake
    /// publication while preserving the ordinary handle type expected by P2P
    /// and Torii wiring.
    #[must_use]
    pub fn emergency_fast_disabled() -> Self {
        let block = Arc::new(
            FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, None,
            ),
        );
        let (lane_relay, lane_relay_rx) = mpsc::sync_channel(0);
        let (wake, wake_rx) = mpsc::sync_channel(0);
        drop(lane_relay_rx);
        drop(wake_rx);
        let mut handle = Self::new(
            block,
            lane_relay,
            wake,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            ConsensusOutputGuard::isolated(),
        );
        handle.emergency_fast_disabled = true;
        handle
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
        self.pending_queue_plan_admission_dirty
            .store(true, Ordering::Release);
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
        if self.emergency_fast_disabled {
            return SumeragiIngressDisposition::Obsolete;
        }
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(inbound);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            iroha_logger::debug!(
                "deferring Sumeragi ingress until context and safety WAL replay complete"
            );
            return SumeragiIngressDisposition::Retry(inbound);
        }
        let queue = status::WorkerQueueKind::Blocks;
        match self.block.try_push(inbound) {
            Ok(FairV2IngressPushDisposition::Enqueued) => {
                status::record_worker_queue_enqueue(queue);
                self.wake();
                SumeragiIngressDisposition::Accepted
            }
            Ok(FairV2IngressPushDisposition::Coalesced) => SumeragiIngressDisposition::Coalesced,
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
                let message_kind = FairV2IngressMessageKind::classify(rejection.inbound.message());
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
        }
    }
    /// Try to enqueue a canonical message and preserve it on retryable pressure.
    pub fn try_incoming_block_message_from_owned(
        &self,
        sender: PeerId,
        message: BlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        self.try_incoming_block_message_owned(InboundBlockMessage::from_authenticated_peer(
            message, sender,
        ))
    }
    /// Enqueue a canonical message from an authenticated transport peer.
    pub fn incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) {
        let _ = self.try_incoming_block_message_from_owned(sender, message);
    }
    /// Try to enqueue a canonical message from an authenticated transport peer.
    pub fn try_incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) -> bool {
        self.try_incoming_block_message_from_owned(sender, message)
            .accepted_or_coalesced()
    }
    /// Try to transfer one exact lane-relay item to its serialized owner.
    pub fn try_incoming_lane_relay_owned(
        &self,
        message: LaneRelayMessage,
    ) -> SumeragiIngressDisposition<LaneRelayMessage> {
        if self.emergency_fast_disabled {
            return SumeragiIngressDisposition::Obsolete;
        }
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(message);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            return SumeragiIngressDisposition::Retry(message);
        }
        if let LaneRelayMessage::QueuePlanAdmissionCertificate { certificate, .. } = &message
            && (certificate.is_empty()
                || certificate.len() > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES)
        {
            iroha_logger::debug!(
                bytes = certificate.len(),
                "rejecting malformed QueuePlan admission certificate before lane ingress"
            );
            return SumeragiIngressDisposition::Rejected(message);
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
        let send = match self
            .block
            .try_with_open_lane_relay_admission(message, |message| {
                self.lane_relay.try_send(message)
            }) {
            Ok(send) => send,
            Err(message) => return SumeragiIngressDisposition::Retry(message),
        };
        match send {
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
#[cfg(test)]
mod emergency_fast_handle_tests {
    use super::*;
    use iroha_crypto::KeyPair;

    #[test]
    fn disabled_handle_never_opens_consensus_admission() {
        let handle = SumeragiHandle::emergency_fast_disabled();
        assert!(!handle.notify_pending_queue_plan_admission());
        assert!(!handle.ingress_ready.load(Ordering::Acquire));
        assert!(!handle.restart_required());
        assert!(handle.emergency_fast_disabled);
        let sender = PeerId::new(KeyPair::random().public_key().clone());
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(LaneRelayMessage::QueuePlanAdmissionCertificate {
                sender,
                certificate: Arc::new(Vec::new()),
            }),
            SumeragiIngressDisposition::Obsolete
        ));
    }
}
#[cfg(any(test, feature = "sumeragi-main-loop-tests"))]
fn test_sumeragi_handle(
    block_capacity: usize,
) -> (
    SumeragiHandle,
    Arc<FairV2Ingress>,
    mpsc::Receiver<LaneRelayMessage>,
) {
    test_sumeragi_handle_with_source_geometry(block_capacity, None)
}
#[cfg(any(test, feature = "sumeragi-main-loop-tests"))]
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
        .expect("an empty test roster requires no ingress reservation");
    block.open().expect("open configured test ingress");
    let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(block_capacity);
    let (wake_tx, _wake_rx) = mpsc::sync_channel(1);
    let handle = SumeragiHandle::new(
        Arc::clone(&block),
        lane_relay_tx,
        wake_tx,
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(false)),
        ConsensusOutputGuard::isolated(),
    );
    (handle, block, lane_relay_rx)
}
include!("tests/queue_plan_admission_handoff.rs");
/// Spawn configuration for the authoritative serialized Sumeragi v2 worker.
pub struct SumeragiStartArgs {
    /// Canonical frozen Sumeragi-v2 consensus configuration.
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
    /// Runtime-only owner of this validator's adaptive global-beacon signing share.
    ///
    /// The owner is injected by the daemon/runtime boundary and is never
    /// serialized into configuration or World state.
    pub global_beacon_partial_signer:
        Option<Arc<dyn crate::beacon::GlobalThresholdBeaconPartialSignerV1>>,
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
            global_beacon_partial_signer,
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
        block.require_leader_wire_lifecycle_gate();
        let (lane_relay_tx, lane_relay_rx) = mpsc::sync_channel(lane_relay_channel_cap);
        let (wake_tx, wake_rx) = mpsc::sync_channel(WORKER_WAKE_CHANNEL_CAP);
        let queue_wake = Arc::clone(&queue);
        let queue_wake_tx = wake_tx.clone();
        let ingress_ready = Arc::new(AtomicBool::new(false));
        let pending_queue_plan_admission_dirty = Arc::new(AtomicBool::new(true));
        let handle = SumeragiHandle::new(
            Arc::clone(&block),
            lane_relay_tx,
            wake_tx,
            Arc::clone(&ingress_ready),
            Arc::clone(&pending_queue_plan_admission_dirty),
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
            global_beacon_partial_signer,
            startup_replay_plan,
            startup_replay_inventory_guard,
            network,
            genesis_network,
            lane_relay_rx,
            ingress_ready,
            pending_queue_plan_admission_dirty,
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
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
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
    global_beacon_partial_signer:
        Option<Arc<dyn crate::beacon::GlobalThresholdBeaconPartialSignerV1>>,
    startup_replay_plan: V2StartupReplayPlan,
    startup_replay_inventory_guard: V2StartupReplayInventoryGuard,
    network: IrohaNetwork,
    genesis_network: GenesisWithPubKey,
    lane_relay_rx: mpsc::Receiver<LaneRelayMessage>,
    ingress_ready: Arc<AtomicBool>,
    pending_queue_plan_admission_dirty: Arc<AtomicBool>,
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
    include!("tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs");
    #[test]
    fn lane_relay_admission_gate_drains_pre_cut_sender_before_close() {
        let (_handle, ingress, _lane_relay_rx) = super::test_sumeragi_handle(1);
        let sender_ingress = std::sync::Arc::clone(&ingress);
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
        let (lane_tx, lane_rx) = std::sync::mpsc::sync_channel(1);
        let sender = std::thread::spawn(move || {
            let result = sender_ingress.try_with_open_lane_relay_admission(7_u8, |value| {
                entered_tx.send(()).expect("announce pre-cut lane sender");
                release_rx
                    .recv()
                    .expect("release pre-cut lane sender after close blocks");
                lane_tx.try_send(value)
            });
            result_tx
                .send(result)
                .expect("publish lane admission result");
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("pre-cut sender acquires the ingress state gate");

        let closer_ingress = std::sync::Arc::clone(&ingress);
        let (closed_tx, closed_rx) = std::sync::mpsc::sync_channel(1);
        let closer = std::thread::spawn(move || {
            closer_ingress.close();
            closed_tx.send(()).expect("publish completed ingress close");
        });
        assert!(
            closed_rx
                .recv_timeout(std::time::Duration::from_millis(25))
                .is_err(),
            "close must wait for the already-admitted lane transfer"
        );
        release_tx
            .send(())
            .expect("complete the pre-cut lane transfer");
        assert!(
            matches!(
                result_rx
                    .recv_timeout(std::time::Duration::from_secs(1))
                    .expect("pre-cut lane transfer completes"),
                Ok(Ok(()))
            ),
            "the channel owns the pre-close occurrence"
        );
        closed_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("close completes after the sender releases the state gate");
        sender.join().expect("join pre-cut lane sender");
        closer.join().expect("join ingress closer");
        assert_eq!(
            lane_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("finite relay prefix retains the pre-cut occurrence"),
            7
        );
        assert_eq!(
            ingress.try_with_open_lane_relay_admission(9_u8, |value| value),
            Err(9),
            "post-cut lane ownership remains with its caller"
        );
    }
    #[test]
    fn authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains() {
        const SOURCE_BYTES: usize = 1024 * 1024;
        let ingress = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            11,
            3 * SOURCE_BYTES,
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
            .expect("one validator and two authenticated relays fit exactly");
        ingress.open().expect("open exact source geometry");
        {
            let state = ingress.state.lock();
            assert_eq!(
                state.len
                    + super::fair_v2_ingress_current_protected_slots(
                        &state,
                        ingress.authenticated_non_validator_source_capacity,
                    ),
                11,
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
                11,
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
        assert_eq!(first.via(), &source_a);
        {
            let state = ingress.state.lock();
            assert_eq!(
                state.len
                    + super::fair_v2_ingress_current_protected_slots(
                        &state,
                        ingress.authenticated_non_validator_source_capacity,
                    ),
                11,
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
        assert_eq!(second.via(), &source_b);
        let third = ingress.try_recv().expect("source C receives its fair turn");
        assert_eq!(third.via(), &source_c);
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
        assert_eq!(sender, semantic_origin.clone());
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
            .expect("two authenticated validator lanes fit");
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
        assert_eq!(delivered.sender(), &semantic_origin);
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
    fn fair_v2_ingress_coalesces_stale_timeout_vote_retry_without_regressing_route() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
        let validator = validator_peers(1).pop().expect("validator fixture");
        let mut routes = NetworkReplyRouteTestFixture::new(validator.clone());
        let stale_route = routes.mint(validator.clone());
        let retained_route = routes
            .redeliver(&stale_route)
            .expect("same-source later delivery capability");
        assert_eq!(
            stale_route.source_update_from(&retained_route),
            Err(NetworkReplyRouteError::Stale),
            "the delayed retry must be strictly older than the retained delivery"
        );
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("one authenticated validator lane fits");
        ingress.open().expect("open configured roster");
        let timeout_vote = v2_timeout_vote();
        let inbound = |route: NetworkReplyRoute| {
            InboundBlockMessage::try_from_transport_with_reply_route(
                timeout_vote.clone(),
                validator.clone(),
                validator.clone(),
                route,
            )
            .expect("live route binds the TimeoutVote origin and authenticated source")
        };
        assert!(matches!(
            ingress.try_push(inbound(retained_route.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(inbound(stale_route)),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert_eq!(
            ingress.len(),
            1,
            "the exact TimeoutVote remains queued once"
        );
        let delivered = ingress
            .try_recv()
            .expect("deliver the retained TimeoutVote");
        let evidence = delivered
            .ingress_ownership()
            .expect("queued TimeoutVote retains exact ingress ownership");
        assert!(evidence.validate_exact());
        assert_eq!(
            evidence.occurrence_count, 1,
            "a stale retry does not enter the admitted ownership history"
        );
        assert_eq!(
            evidence.latest_action(),
            super::FairV2IngressOwnershipAction::New
        );
        let retained = evidence
            .current_reply_routes()
            .expect("the newer reply-route snapshot remains attached");
        assert_eq!(retained.len(), 1);
        assert!(
            retained
                .iter()
                .any(|route| route.same_delivery(&retained_route)),
            "coalescing an older retry must not regress the retained route"
        );
    }
    #[test]
    fn fair_v2_ingress_rejects_stale_non_timeout_vote_route() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
        let validator = validator_peers(1).pop().expect("validator fixture");
        let mut routes = NetworkReplyRouteTestFixture::new(validator.clone());
        let stale_route = routes.mint(validator.clone());
        let retained_route = routes
            .redeliver(&stale_route)
            .expect("same-source later delivery capability");
        assert_eq!(
            stale_route.source_update_from(&retained_route),
            Err(NetworkReplyRouteError::Stale),
            "the negative control must exercise the same stale-route relation"
        );
        ingress.close();
        ingress
            .configure_roster([validator.clone()])
            .expect("one authenticated validator lane fits");
        ingress.open().expect("open configured roster");
        let prepare = v2_auxiliary_prepare(0);
        let inbound = |route: NetworkReplyRoute| {
            InboundBlockMessage::try_from_transport_with_reply_route(
                prepare.clone(),
                validator.clone(),
                validator.clone(),
                route,
            )
            .expect("live route binds the Vote origin and authenticated source")
        };
        assert!(matches!(
            ingress.try_push(inbound(retained_route.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let rejection = ingress.try_push(inbound(stale_route));
        assert!(matches!(
            rejection,
            Err(super::FairV2IngressPushError::Rejected(
                super::FairV2IngressRejection {
                    reason: super::FairV2IngressRejectReason::RouteOwnershipInvalid,
                    ..
                }
            ))
        ));
        assert_eq!(
            ingress.len(),
            1,
            "rejection preserves the queued Vote owner"
        );
        let delivered = ingress.try_recv().expect("deliver the retained Vote");
        let retained = delivered
            .ingress_ownership()
            .and_then(|evidence| evidence.current_reply_routes())
            .expect("the newer reply-route snapshot remains attached");
        assert_eq!(retained.len(), 1);
        assert!(
            retained
                .iter()
                .any(|route| route.same_delivery(&retained_route)),
            "rejecting a stale Vote route must not regress the retained route"
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
            .expect("one validator and one authenticated relay fit exactly");
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
            .expect("two authenticated validator lanes fit");
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
        mutated.latest.semantic_origin = super::authenticated_peer_for_test();
        rejected("semantic origin", mutated);
        let mut mutated = evidence.clone();
        mutated.latest.authenticated_via = super::authenticated_peer_for_test();
        rejected("authenticated delivery peer", mutated);
        let mut mutated = evidence.clone();
        mutated.latest.authenticated_source =
            super::FairV2IngressSource::Authenticated(super::authenticated_peer_for_test());
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
            .expect("authenticated validator lane fits");
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
        let first_projection = first.process_local_projection_hash();
        let mut widened_capacity = first;
        widened_capacity.first.resource_before.message_capacity = widened_capacity
            .first
            .resource_before
            .message_capacity
            .checked_add(1)
            .expect("test capacity fits usize");
        widened_capacity.first.resource_after.message_capacity = widened_capacity
            .first
            .resource_after
            .message_capacity
            .checked_add(1)
            .expect("test capacity fits usize");
        widened_capacity.latest.resource_before.message_capacity = widened_capacity
            .latest
            .resource_before
            .message_capacity
            .checked_add(1)
            .expect("test capacity fits usize");
        widened_capacity.latest.resource_after.message_capacity = widened_capacity
            .latest
            .resource_after
            .message_capacity
            .checked_add(1)
            .expect("test capacity fits usize");
        assert!(
            widened_capacity.validate_exact(),
            "a consistently wider resource envelope remains individually valid"
        );
        assert_ne!(
            first_projection,
            widened_capacity.process_local_projection_hash(),
            "the process-local cut must bind complete occurrence resource geometry"
        );
    }
    include!("tests/mod_authoritative_runtime_gate_06_source_isolation.rs");
    macro_rules! assert_push {
        ($ingress:expr, $message:expr, $sender:expr, Ok($expected:ident) $(, $reason:literal)?) => {
            assert!(matches!(
                $ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                    $message, $sender
                )),
                Ok(super::FairV2IngressPushDisposition::$expected)
            ) $(, $reason)?);
        };
        ($ingress:expr, $message:expr, $sender:expr, Err($expected:ident) $(, $reason:literal)?) => {
            assert!(matches!(
                $ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                    $message, $sender
                )),
                Err(super::FairV2IngressPushError::$expected(_))
            ) $(, $reason)?);
        };
    }
    #[test]
    fn fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let auxiliary = v2_auxiliary_prepare(0);
        let progress = v2_commit_certificate_request(0, &validator);
        let second_progress = v2_commit_certificate_request(1, &validator);
        let timeout = v2_timeout_vote();
        let body_response = v2_certified_body_response(0, validator.clone(), 64);
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
            .expect("validator partition fits");
        ingress.open().expect("open configured roster");
        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::from_authenticated_peer(
                progress.clone(),
                validator.clone(),
            )),
            FairV2IngressClass::Progress
        );
        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::from_authenticated_peer(
                body_response.clone(),
                validator.clone(),
            )),
            FairV2IngressClass::TransportCompletion
        );
        for message in [auxiliary, progress, timeout] {
            assert_push!(ingress, message, validator.clone(), Ok(Enqueued));
        }
        assert_push!(
            ingress,
            second_progress,
            validator.clone(),
            Err(Full),
            "ordinary Progress cannot spend the completion corridor"
        );
        assert_push!(ingress, body_response, validator.clone(), Ok(Enqueued));
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
        assert_push!(
            ingress,
            chunk,
            validator,
            Ok(Enqueued),
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
        let response = v2_certified_body_response(0, first.clone(), 64);
        let second_response = v2_certified_body_response(0, second.clone(), 64);
        let rotated_response = v2_certified_body_response(0, outsider.clone(), 64);
        let completion_bytes = encoded_v2_len(&chunk)
            .max(encoded_v2_len(&response))
            .max(encoded_v2_len(&second_response))
            .max(encoded_v2_len(&rotated_response));
        let source_bytes = completion_bytes + 1;
        let ingress =
            super::FairV2Ingress::new(12, 3 * source_bytes, source_bytes, 0, completion_bytes);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator sources fit");
        ingress.open().expect("open configured roster");
        let oversized = v2_message_with_bytes(7, completion_bytes + 1);
        assert_push!(ingress, oversized, first.clone(), Err(Rejected));
        assert_push!(ingress, chunk.clone(), first.clone(), Ok(Enqueued));
        assert_push!(ingress, chunk, first.clone(), Ok(Coalesced));
        assert_push!(
            ingress,
            response.clone(),
            first.clone(),
            Err(Full),
            "one source serializes chunk/response conflicts through one shared owner"
        );
        assert_push!(
            ingress,
            second_response,
            second.clone(),
            Ok(Enqueued),
            "one validator cannot consume another validator's completion owner"
        );
        assert_push!(
            ingress,
            rotated_response,
            outsider.clone(),
            Ok(Enqueued),
            "a current authenticated archive can complete an exact historical request after rotation"
        );
        assert_push!(
            ingress,
            v2_message_with_bytes(1, 64),
            outsider,
            Err(Rejected),
            "a non-roster archive cannot inject current-height PayloadChunk authority"
        );
        let first_completion = ingress
            .try_recv_if(|inbound| inbound.sender() == &first)
            .expect("fair service releases the first validator's completion owner");
        assert!(super::fair_v2_ingress_is_transport_completion(
            &first_completion
        ));
        assert_push!(ingress, response, first, Ok(Enqueued));
    }
    #[test]
    fn fair_v2_ingress_rejects_insufficient_roster_byte_partition() {
        let validators = validator_peers(2);
        let ingress = super::FairV2Ingress::new(12, 2 * 1024 - 1, 1024, 0, 0);
        let error = ingress
            .configure_roster(validators)
            .expect_err("two validators require two exact byte partitions");
        assert!(error.is_bytes());
        assert_eq!(error.configured(), 2 * 1024 - 1);
        assert_eq!(error.required(), 2 * 1024);
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
            .expect("validator byte partition fits");
        ingress.open().expect("open configured roster");
        assert_push!(ingress, auxiliary, validator.clone(), Ok(Enqueued));
        assert_push!(
            ingress,
            v2_auxiliary_prepare(1),
            validator.clone(),
            Err(Full)
        );
        assert_push!(ingress, timeout_vote, validator.clone(), Ok(Enqueued));
        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
            .expect("reserved timeout vote bypasses the byte-saturated auxiliary prefix");
        assert_eq!(delivered.sender(), &validator);
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
            .expect("validator byte partition fits");
        ingress.open().expect("open configured roster");
        assert_push!(ingress, first, validator.clone(), Ok(Enqueued));
        assert_push!(ingress, second.clone(), validator.clone(), Err(Full));
        let delivered = ingress
            .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
            .expect("fair service releases the first TimeoutVote byte owner");
        assert!(super::fair_v2_ingress_is_timeout_vote(&delivered));
        assert_push!(ingress, second, validator, Ok(Enqueued));
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
            required, 16_828_108,
            "recommended wire ceiling is a regression boundary"
        );
        let required_proposal =
            super::fair_v2_ingress_required_proposal_bytes(layout, wire::MAX_VALIDATORS_PER_HEIGHT);
        assert_eq!(
            required_proposal, 73_916,
            "maximal proposal wire geometry is a regression boundary"
        );
        let proposal = v2_maximum_structural_proposal_wire(layout, wire::MAX_VALIDATORS_PER_HEIGHT);
        let maximum_execution_commitment_bytes = match &proposal {
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
                ..
            }) => match &proposal.justification {
                wire::ProposalJustification::Timeout(timeout) => timeout
                    .highest_prepare_qc
                    .as_ref()
                    .expect("maximum proposal has a highest PrepareQC")
                    .execution_commitment
                    .encode()
                    .len(),
                wire::ProposalJustification::ParentCommit(_) => {
                    unreachable!("maximum proposal uses Timeout justification")
                }
            },
            _ => unreachable!("maximum proposal fixture is v2"),
        };
        assert_eq!(
            maximum_execution_commitment_bytes,
            super::FAIR_V2_INGRESS_MAX_EXECUTION_COMMITMENT_BYTES,
            "allocation-free execution-commitment geometry must match canonical bare Norito",
        );
        assert_eq!(
            encoded_v2_len(&proposal),
            required_proposal,
            "checked activation geometry must equal canonical bare Norito; execution commitment bytes={maximum_execution_commitment_bytes}"
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
        let maximal_roster = (0..wire::MAX_VALIDATORS_PER_HEIGHT)
            .map(|index| {
                let seed = u8::try_from(index)
                    .expect("validator bound fits u8")
                    .saturating_add(1);
                PeerId::new(
                    KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
                        .expect("derive canonical relay-node roster fixture")
                        .public_key()
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
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
            &network_message,
        );
        let exact_broadcast_frame =
            iroha_p2p::network::data_frame_wire_len(maximal_peer, None, &network_message);
        let required_control_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(required_proposal);
        let network_message_bytes = network_message.encoded_len();
        assert_eq!(
            required_control_frame,
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
                network_message_bytes
            ),
            "canonical node identities must use the exact complete direct P2P wire"
        );
        assert!(required_control_frame >= exact_direct_frame);
        assert!(exact_direct_frame > exact_broadcast_frame);
        let minimal_layout = minimal_rs16_layout();
        let minimal_proposal_bytes =
            super::fair_v2_ingress_required_proposal_bytes(minimal_layout, 1);
        assert_eq!(minimal_proposal_bytes, 2_709);
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
        let recovery_network_id = crate::sumeragi::synthetic_network_id("fair-v2-ingress-test");
        let (body_request, commit_request, commit_response) =
            v2_maximum_recovery_wires(&recovery_network_id, minimal_peer, 1);
        assert_eq!(
            super::fair_v2_ingress_required_recovery_request_bytes_for_key(
                &recovery_network_id,
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
                &crate::sumeragi::synthetic_network_id("fair-v2-ingress-test"),
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
        let validator = PeerId::new(
            KeyPair::try_from_seed(vec![0xD7; 32], iroha_crypto::Algorithm::BlsNormal)
                .expect("derive canonical relay-node fixture")
                .public_key()
                .clone(),
        );
        let (_, responder_key_bytes) = validator
            .public_key()
            .try_to_bytes()
            .expect("validator fixture key encodes");
        let response = v2_maximum_certified_body_response(layout, validator.clone());
        let actual_required = super::fair_v2_ingress_required_transport_completion_bytes_for_key(
            layout,
            responder_key_bytes.len(),
        );
        let required = super::fair_v2_ingress_required_transport_completion_bytes(layout);
        assert_eq!(encoded_v2_len(&response), actual_required);
        assert!(actual_required <= required);
        let network_response = crate::NetworkMessage::SumeragiBlock(Arc::new(
            super::message::BlockMessageWire::new(response.clone()),
        ));
        assert_eq!(
            super::fair_v2_ingress_network_message_bytes(actual_required),
            Some(network_response.encoded_len()),
            "maximum completion must cross the exact full NetworkMessage framing"
        );
        let actual_direct_response_frame = iroha_p2p::network::data_frame_wire_len(
            &validator,
            Some(&validator),
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
        let actual_response_frame =
            super::fair_v2_ingress_required_p2p_frame_bytes(actual_required);
        assert_eq!(
            actual_response_frame,
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
                network_response.encoded_len()
            ),
            "the concrete responder key must retain exact canonical direct-relay geometry"
        );
        assert!(protocol_maximum_response_frame >= actual_response_frame);
        assert!(protocol_maximum_response_frame >= actual_direct_response_frame);
        let network_id = crate::sumeragi::synthetic_network_id("fair-v2-ingress-test");
        let roster_len = 1;
        let certified_bytes =
            super::fair_v2_ingress_required_certified_fence_escape_bytes(roster_len);
        let proposal_bytes = super::fair_v2_ingress_required_proposal_bytes(layout, roster_len);
        let control_message_bytes = proposal_bytes
            .max(super::fair_v2_ingress_required_commit_certificate_response_bytes(roster_len));
        let request_bytes =
            super::fair_v2_ingress_required_recovery_request_bytes(&network_id, roster_len);
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
            .expect_err("completion reserve cannot consume the reviewed ordinary region");
        assert_eq!(ordinary_error.configured(), ordinary_bytes - 1);
        assert_eq!(ordinary_error.required(), ordinary_bytes);
        assert_eq!(ordinary_short.open(), Err(ordinary_error));
        let source_bytes = ordinary_bytes
            .checked_add(certified_bytes)
            .and_then(|bytes| bytes.checked_add(required))
            .expect("test source bound fits usize");
        let other_network_id =
            crate::sumeragi::synthetic_network_id("fair-v2-ingress-other-genesis");
        let other_network_request_bytes =
            super::fair_v2_ingress_required_recovery_request_bytes(&other_network_id, roster_len);
        assert_eq!(
            other_network_request_bytes, request_bytes,
            "fixed-width exact network identity keeps ingress sizing genesis-independent"
        );
        let other_network_ingress =
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
        other_network_ingress
            .configure_roster_for_context([validator.clone()], &other_network_id, layout)
            .expect("every exact network id fits its fixed-width ordinary byte owner");
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
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
            .configure_roster_for_context([validator.clone()], &network_id, layout)
            .expect("exact completion ceiling leaves a reviewed ordinary partition");
        ingress.open().expect("exactly sized ingress opens");
        let oversized = v2_certified_body_response(9, validator.clone(), required + 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                oversized,
                validator.clone()
            )),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                response, validator
            )),
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
                &crate::sumeragi::synthetic_network_id("overflow-test"),
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
    fn fair_v2_ingress_predicate_scan_selects_same_lane_request_without_queue_local_serve_gate() {
        let (handle, ingress, _relay_receiver) =
            test_sumeragi_handle_with_source_geometry(10, Some(0));
        let validators = validator_peers(2);
        let first_ready_source = validators[0].clone();
        let target_source = validators[1].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("two validators and their protected owners fit");
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
        let same_lane_request = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the same-lane request bypasses an unrelated auxiliary predecessor");
        assert_eq!(same_lane_request.sender(), &first_ready_source);
        let target = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the next ready source receives its fair predicate-selected turn");
        assert_eq!(target.sender(), &target_source);
        let predecessor = ingress
            .try_recv_if(|_| true)
            .expect("the blocked preexisting entry remains queued");
        assert_eq!(predecessor.sender(), &first_ready_source);
        assert_eq!(vote_height(&predecessor), Some(1));
        assert_eq!(ingress.len(), 0);
    }
    #[test]
    fn fair_v2_ingress_predicate_scan_does_not_create_queue_local_serve_gate() {
        let (handle, ingress, _relay_receiver) =
            test_sumeragi_handle_with_source_geometry(25, Some(0));
        let validators = validator_peers(5);
        let target_source = validators[0].clone();
        let control_source = validators[1].clone();
        let completion_source = validators[2].clone();
        let priority_source = validators[3].clone();
        let causal_source = validators[4].clone();
        ingress.close();
        ingress
            .configure_roster(validators)
            .expect("five validators and their protected owners fit");
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
            v2_certified_body_response(7, completion_source.clone(), 64),
        ));
        assert!(
            handle.try_incoming_block_message_from(priority_source.clone(), v2_timeout_vote(),)
        );
        assert!(
            handle
                .try_incoming_block_message_from(causal_source.clone(), v2_auxiliary_prepare(11),)
        );
        let control = ingress
            .try_recv_if(|inbound| !fair_v2_ingress_is_certified_body_request(inbound))
            .expect("ordinary control remains selectable without a queue-local Serve gate");
        assert_eq!(control.sender(), &control_source);
        assert_eq!(vote_phase(&control), Some(wire::GlobalPhase::Commit));
        let target = ingress
            .try_recv_if(fair_v2_ingress_is_certified_body_request)
            .expect("the request remains independently predicate-selectable");
        assert_eq!(target.sender(), &target_source);
        let completion = ingress
            .try_recv_if(|_| true)
            .expect("completion keeps its fair source turn");
        assert_eq!(completion.sender(), &completion_source);
        assert!(matches!(
            completion.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_),
                ..
            })
        ));
        let priority = ingress
            .try_recv_if(|_| true)
            .expect("priority work keeps its fair source turn");
        assert_eq!(priority.sender(), &priority_source);
        assert!(matches!(
            priority.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        ));
        let causal = ingress
            .try_recv_if(|_| true)
            .expect("causal work keeps its fair source turn");
        assert_eq!(causal.sender(), &causal_source);
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
            .expect("validator protected owners fit");
        ingress.open().expect("open configured roster");
        let message = v2_certified_body_response(7, validator.clone(), 64);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                validator.clone(),
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
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message,
                validator.clone()
            )),
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
            .expect("rollover reinstalls the validator lane");
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
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                v2_auxiliary_prepare(8),
                validator.clone(),
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
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                v2_auxiliary_prepare(9),
                validator,
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
    include!("tests/mod_authoritative_runtime_gate_09_checked_dequeue.rs");
    include!("tests/mod_authoritative_runtime_gate_09_snapshot_and_source_lanes.rs");
    #[test]
    fn fair_v2_ingress_production_message_capacity_delegates_to_config_geometry() {
        for (roster_len, authenticated_sources) in [(0, 0), (4, 2), (5, 2), (31, 2)] {
            assert_eq!(
                super::fair_v2_ingress_required_capacity(
                    roster_len,
                    Some(authenticated_sources),
                ),
                iroha_config::parameters::actual::sumeragi_v2_body_ingress_required_message_capacity(
                    roster_len,
                    authenticated_sources,
                ),
            );
        }
        assert_eq!(
            super::fair_v2_ingress_required_capacity(0, None),
            Some(0),
            "an empty authenticated roster needs no hidden source lane",
        );
        assert_eq!(
            super::fair_v2_ingress_required_capacity(4, None),
            Some(20),
            "the roster-only geometry reserves five classes per validator",
        );
    }
    #[test]
    fn v2_ingress_rejects_capacity_without_per_validator_progress_reservations() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(19);
        ingress.close();
        let error = ingress
            .configure_roster(validator_peers(4))
            .expect_err(
                "four validators require ordinary, progress, certified, TimeoutVote, and transport-completion slots",
            );
        assert_eq!(error.configured(), 19);
        assert_eq!(error.required(), 20);
        assert_eq!(ingress.open(), Err(error));
    }
}
impl SumeragiWorker {
    fn run(self) {
        v2_runner::run(self);
    }
}
