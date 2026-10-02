//! Actual four-node signed software-clock observations retained by the Native owner.
//! No handset/FI/block timestamp, accepting callback or hardware-clock claim enters this path.

use super::{KagemushaRecoveryJournalPrefixV1, PrivateJournal, PrivateJournalFormat};
use iroha_crypto::{Algorithm, Hash};
use iroha_data_model::{
    NetworkId,
    sumeragi_finality::{
        SumeragiFinalityAttestation, SumeragiFinalityCheckpoint, SumeragiFinalityProof,
        SumeragiFinalityVerifier,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_primitives::time::NativeContinuousReading;
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, num::NonZeroU64, path::Path, sync::Arc, time::Duration};

#[path = "ordinary_native_clock/signed_originals.rs"]
mod signed_originals;
pub(crate) use signed_originals::KagemushaRetainedOrdinaryNativeClockOriginalsV1;
pub use signed_originals::{
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
    KagemushaOrdinaryNativeSignedClockOriginalV1,
    KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
    verify_ordinary_native_signed_clock_original_v1,
};

#[path = "ordinary_native_clock/selection_original.rs"]
mod selection_original;
pub use selection_original::{
    KAGEMUSHA_ORDINARY_NATIVE_CLOCK_SELECTION_ORIGINAL_MAX_BYTES_V1,
    KagemushaOrdinaryNativeClockSelectionOriginalV1,
};

const MAX_FRAME: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-native-clock.norito.wal",
    magic: b"KGMCLOK1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-native-clock-wal\0",
    maximum_payload_bytes: MAX_FRAME as u64,
};

/// Closed failures; node/proof/private-clock material never enters error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaOrdinaryNativeClockErrorV1 {
    /// Selected originals, signed current replies, nonce, skew, age or high-water differ.
    #[error("ordinary native clock original rejected")]
    Rejected,
    /// Native elapsed clock, owned WAL or durable publication is unavailable.
    #[error("ordinary native clock custody unavailable")]
    Custody,
}
type Result<T> = core::result::Result<T, KagemushaOrdinaryNativeClockErrorV1>;
use KagemushaOrdinaryNativeClockErrorV1::{Custody, Rejected};

/// Independently governed software-clock bounds selected by the installed runtime owner.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeClockPolicyV1"
)]
pub struct KagemushaOrdinaryNativeClockPolicyV1 {
    /// Maximum nonce-to-admission elapsed time, including suspension; at most ten seconds.
    pub maximum_reply_age_ms: u64,
    /// Maximum four-node clock spread; at most thirty seconds.
    pub maximum_node_skew_ms: u64,
    /// Maximum projection before another fresh original observation; at most one day.
    pub maximum_projection_age_ms: u64,
    /// Maximum durable publication stall and reserved high-water margin; at most ten seconds.
    /// This ceiling is never lent as the current reading or used to satisfy not-before checks.
    pub maximum_persistence_age_ms: u64,
}
impl KagemushaOrdinaryNativeClockPolicyV1 {
    fn validate(self) -> Result<()> {
        if self.maximum_reply_age_ms == 0
            || self.maximum_reply_age_ms > 10_000
            || self.maximum_node_skew_ms == 0
            || self.maximum_node_skew_ms > 30_000
            || self.maximum_projection_age_ms < self.maximum_reply_age_ms
            || self.maximum_projection_age_ms > 86_400_000
            || self.maximum_persistence_age_ms == 0
            || self.maximum_persistence_age_ms > self.maximum_reply_age_ms
        {
            return Err(Rejected);
        }
        Ok(())
    }
}

/// Actual Native interval projected from the same four signed observations. Neither bound is
/// a universal clock point: activation must pass at the lower bound, expiry at the upper bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaOrdinaryNativeTimeIntervalV1 {
    lower_ms: u64,
    upper_ms: u64,
}
impl KagemushaOrdinaryNativeTimeIntervalV1 {
    /// Earliest current reading supported by the authenticated original reply interval.
    #[must_use]
    pub const fn lower_ms(self) -> u64 {
        self.lower_ms
    }
    /// Latest current reading supported by the original Native request interval.
    #[must_use]
    pub const fn upper_ms(self) -> u64 {
        self.upper_ms
    }
    /// Require the complete actual current interval inside an immutable half-open validity window.
    /// # Errors
    /// Rejects future activation, expiry or a malformed validity interval.
    pub fn require_validity(self, not_before_ms: u64, expires_at_ms: u64) -> Result<()> {
        if not_before_ms == 0
            || not_before_ms >= expires_at_ms
            || self.lower_ms < not_before_ms
            || self.upper_ms >= expires_at_ms
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Run a pure validity check at both supported endpoints. The closure cannot supply time;
    /// this interval was already lent by the actual Native owner.
    /// # Errors
    /// Returns the original validation failure at either endpoint.
    pub fn check_both<E>(
        self,
        mut validate: impl FnMut(u64) -> core::result::Result<(), E>,
    ) -> core::result::Result<(), E> {
        validate(self.lower_ms)?;
        if self.upper_ms != self.lower_ms {
            validate(self.upper_ms)?;
        }
        Ok(())
    }
    // Exact synthetic point is restricted to existing test/harness custody.
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    pub(crate) const fn fixture_point(ms: u64) -> Self {
        Self {
            lower_ms: ms,
            upper_ms: ms,
        }
    }
}

/// Exact current node identity and executable/configuration pins selected before any reply.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeClockNodeV1")]
pub struct KagemushaOrdinaryNativeClockNodeV1 {
    /// Installed BLS reporting identity; replies cannot select it.
    pub peer_id: PeerId,
    /// Exact independently admitted validator executable.
    pub build_fingerprint: Hash,
    /// Exact independently admitted effective native configuration.
    pub config_fingerprint: Hash,
}

/// Immutable Rust-native selection from the owner-admitted signed runtime inventory.
/// The constructor checks crypto/shape relationships; its installing native owner must retain
/// the independent signed inventory/root custody. Neither decoded public pins nor this value
/// alone admits an installation, account session, FI or money. There is no C/JNI constructor.
pub struct KagemushaOrdinaryNativeClockOriginalsV1 {
    checkpoint: SumeragiFinalityCheckpoint,
    network: NetworkId,
    chain_id: String,
    nodes: [KagemushaOrdinaryNativeClockNodeV1; 4],
    policy: KagemushaOrdinaryNativeClockPolicyV1,
    digest: [u8; 32],
}
impl KagemushaOrdinaryNativeClockOriginalsV1 {
    /// Bind the independently installed checkpoint/network, four current node pins and policy.
    /// A downloaded reply cannot supply this trusted Rust provisioning boundary.
    /// # Errors
    /// Refuses malformed checkpoint/network/chain, repeated node identities or invalid policy.
    pub fn from_selected_originals(
        checkpoint: SumeragiFinalityCheckpoint,
        network: NetworkId,
        chain_id: String,
        nodes: [KagemushaOrdinaryNativeClockNodeV1; 4],
        policy: KagemushaOrdinaryNativeClockPolicyV1,
    ) -> Result<Self> {
        policy.validate()?;
        if chain_id.is_empty() || chain_id.len() > 256 || chain_id.chars().any(char::is_control) {
            return Err(Rejected);
        }
        SumeragiFinalityVerifier::from_trusted_checkpoint(&checkpoint, &network, &chain_id)
            .map_err(|_| Rejected)?;
        for (index, node) in nodes.iter().enumerate() {
            if node.peer_id.public_key().algorithm() != Algorithm::BlsNormal
                || nodes[..index]
                    .iter()
                    .any(|earlier| earlier.peer_id == node.peer_id)
            {
                return Err(Rejected);
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-native-clock-selection\0");
        hash.update(network.as_bytes());
        for raw in [
            checkpoint.encode_canonical().map_err(|_| Rejected)?,
            norito::encode_canonical(&nodes).map_err(|_| Rejected)?,
            norito::encode_canonical(&policy).map_err(|_| Rejected)?,
            chain_id.as_bytes().to_vec(),
        ] {
            hash.update(
                u32::try_from(raw.len())
                    .map_err(|_| Rejected)?
                    .to_le_bytes(),
            );
            hash.update(raw);
        }
        Ok(Self {
            checkpoint,
            network,
            chain_id,
            nodes,
            policy,
            digest: hash.finalize().into(),
        })
    }
}

impl KagemushaOrdinaryNativeClockOriginalsV1 {
    /// Exact public identity of the complete root/node/policy selection. This shape projection
    /// creates neither installation authority nor an actual current clock owner.
    #[must_use]
    pub fn selection_digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Fresh move-only Native read reservation. Its public nonce is transport correlation only.
pub struct KagemushaOrdinaryNativeClockReadV1 {
    owner: Arc<()>,
    nonce: [u8; 32],
    current_height: u64,
    started: NativeContinuousReading,
}
impl KagemushaOrdinaryNativeClockReadV1 {
    /// Actual Native-generated nonce supplied to the four selected finality HTTP reads.
    #[must_use]
    pub fn nonce(&self) -> [u8; 32] {
        self.nonce
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::RecordV1")]
enum Record {
    Initialize(Box<Initialize>),
    Finality(Box<Vec<u8>>),
    Observation(Box<Observation>),
    Projected(Box<Projected>),
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::InitializeV1")]
struct Initialize {
    selection_digest: [u8; 32],
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::ObservationV1")]
struct Observation {
    nonce: [u8; 32],
    originals: [Vec<u8>; 4],
    median_ms: u64,
    high_water_ms: u64,
    certified_context_id: Hash,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::ProjectedV1")]
struct Projected {
    observation_digest: [u8; 32],
    sampled_at_ms: u64,
    high_water_ms: u64,
}
struct Reference {
    started: NativeContinuousReading,
    received: NativeContinuousReading,
    median_ms: u64,
    last_lent_lower_ms: u64,
    last_lent_upper_ms: u64,
}

/// Exclusive actual Native current-clock owner and its original append-only fsynced WAL.
/// Cold recovery authenticates original signatures/prefix but lends no old elapsed clock;
/// another fresh four-node nonce is required. Storage/media honesty remains an OS assumption,
/// and this owner asserts no hardware anti-rollback or installation/monetary qualification.
pub struct KagemushaOrdinaryNativeClockOwnerV1 {
    selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
    verifier: SumeragiFinalityVerifier,
    journal: PrivateJournal,
    prefix: Option<KagemushaRecoveryJournalPrefixV1>,
    rows: usize,
    current_height: u64,
    identity: Arc<()>,
    nonce: Option<[u8; 32]>,
    consumed_nonces: BTreeSet<[u8; 32]>,
    observation_digest: [u8; 32],
    signed_observations_original_digest: [u8; 32],
    high_water_ms: u64,
    observation_median_ms: Option<u64>,
    latest_observation: Option<Arc<Observation>>,
    reference: Option<Reference>,
    #[cfg(test)]
    persistence_delay: Duration,
}
impl KagemushaOrdinaryNativeClockOwnerV1 {
    /// Create exactly one purpose directory under the actual selected Native storage parent.
    /// # Errors
    /// Refuses existing/unsafe storage, malformed independently selected originals or uncertain fsync.
    pub fn create(
        root: &Path,
        selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
    ) -> Result<Self> {
        let mut this = Self::new(
            PrivateJournal::create_new(&root.join("native-clock"), FORMAT).map_err(|_| Custody)?,
            selected,
        )?;
        this.append(&Record::Initialize(Box::new(Initialize {
            selection_digest: this.selected.digest,
        })))?;
        Ok(this)
    }

    /// Recover only the same complete original WAL/root, preserving its durable high-water.
    /// No previous boot's projected time is exposed before another fresh signed observation.
    /// # Errors
    /// Refuses missing storage, altered roots/records, invalid original signatures or uncertain suffix.
    pub fn open_existing(
        root: &Path,
        selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
    ) -> Result<Self> {
        let mut this = Self::new(
            PrivateJournal::open_existing(&root.join("native-clock"), FORMAT)
                .map_err(|_| Custody)?,
            selected,
        )?;
        while let Some((_, raw)) = this.journal.replay_next().map_err(|_| Custody)? {
            if this.rows >= MAX_ROWS {
                return Err(Rejected);
            }
            let record = decode(&raw)?;
            match record {
                Record::Initialize(original)
                    if this.rows == 0 && original.selection_digest == this.selected.digest => {}
                Record::Finality(original) if this.rows > 0 => {
                    let proof = decode_proof(&original)?;
                    if this.current_height.checked_add(1) != Some(proof.height()) {
                        return Err(Rejected);
                    }
                    this.verifier.verify(&proof).map_err(|_| Rejected)?;
                    this.current_height = proof.height();
                    this.nonce = None;
                    this.observation_digest = [0; 32];
                    this.signed_observations_original_digest = [0; 32];
                    this.observation_median_ms = None;
                    this.latest_observation = None;
                }
                Record::Observation(original) if this.rows > 0 => {
                    let (median, context) =
                        this.verify_observation(original.nonce, &original.originals)?;
                    if original.nonce == [0; 32]
                        || this.consumed_nonces.contains(&original.nonce)
                        || median != original.median_ms
                        || context != original.certified_context_id
                        || original.high_water_ms < median
                        || median < this.high_water_ms
                        || original.high_water_ms < this.high_water_ms
                        || original.high_water_ms - median
                            >= this.selected.policy.maximum_reply_age_ms
                    {
                        return Err(Rejected);
                    }
                    this.consumed_nonces.insert(original.nonce);
                    this.nonce = Some(original.nonce);
                    this.high_water_ms = original.high_water_ms;
                    this.observation_digest = Sha256::digest(&raw).into();
                    this.observation_median_ms = Some(median);
                    this.signed_observations_original_digest =
                        signed_observation_digest(original.nonce, context, &original.originals)?;
                    this.latest_observation = Some(Arc::new(*original));
                }
                Record::Projected(original) if this.rows > 0 => {
                    let median = this.observation_median_ms.ok_or(Rejected)?;
                    let end = median
                        .checked_add(this.selected.policy.maximum_projection_age_ms)
                        .ok_or(Rejected)?;
                    if this.nonce.is_none()
                        || original.observation_digest != this.observation_digest
                        || original.sampled_at_ms < median
                        || original.sampled_at_ms < this.high_water_ms
                        || original
                            .sampled_at_ms
                            .checked_add(this.selected.policy.maximum_persistence_age_ms)
                            != Some(original.high_water_ms)
                        || original.high_water_ms >= end
                        || original.high_water_ms <= this.high_water_ms
                    {
                        return Err(Rejected);
                    }
                    this.high_water_ms = original.high_water_ms;
                }
                _ => return Err(Rejected),
            }
            this.rows += 1;
        }
        if this.rows == 0 {
            return Err(Rejected);
        }
        this.prefix = Some(this.journal.recovery_prefix().map_err(|_| Custody)?);
        this.recheck()?;
        Ok(this)
    }

    fn new(
        journal: PrivateJournal,
        selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
    ) -> Result<Self> {
        let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &selected.checkpoint,
            &selected.network,
            &selected.chain_id,
        )
        .map_err(|_| Rejected)?;
        let current_height = selected.checkpoint.height();
        Ok(Self {
            selected,
            verifier,
            journal,
            prefix: None,
            rows: 0,
            current_height,
            identity: Arc::new(()),
            nonce: None,
            consumed_nonces: BTreeSet::new(),
            observation_digest: [0; 32],
            signed_observations_original_digest: [0; 32],
            high_water_ms: 0,
            observation_median_ms: None,
            latest_observation: None,
            reference: None,
            #[cfg(test)]
            persistence_delay: Duration::ZERO,
        })
    }

    /// Admit only the immediate genuine certified successor of the retained installed prefix.
    /// Reply-supplied checkpoints/committees never replace the original verifier.
    /// # Errors
    /// Refuses a gap, wrong certified root/committee, malformed canonical proof or failed durability.
    pub fn advance_certified_prefix(&mut self, proof_original: &[u8]) -> Result<()> {
        self.recheck()?;
        let proof = decode_proof(proof_original)?;
        if self.current_height.checked_add(1) != Some(proof.height()) {
            return Err(Rejected);
        }
        let mut candidate = self.verifier.clone();
        candidate.verify(&proof).map_err(|_| Rejected)?;
        self.append(&Record::Finality(Box::new(proof_original.to_vec())))?;
        self.verifier = candidate;
        self.current_height = proof.height();
        self.nonce = None;
        self.observation_digest = [0; 32];
        self.signed_observations_original_digest = [0; 32];
        self.observation_median_ms = None;
        self.latest_observation = None;
        self.reference = None;
        self.recheck()
    }

    /// Generate a fresh process-bound read before contacting the four independently selected nodes.
    /// # Errors
    /// Refuses stale journal custody, unavailable Native elapsed clock or failed real entropy.
    pub fn reserve_current_read(&self) -> Result<KagemushaOrdinaryNativeClockReadV1> {
        self.recheck()?;
        let started = NativeContinuousReading::now().map_err(|_| Custody)?;
        let mut entropy = [0; 32];
        OsRng.try_fill_bytes(&mut entropy).map_err(|_| Custody)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-native-clock-read\0");
        hash.update(self.selected.digest);
        hash.update(entropy);
        let nonce = hash.finalize().into();
        if nonce == [0; 32] || self.consumed_nonces.contains(&nonce) {
            return Err(Custody);
        }
        Ok(KagemushaOrdinaryNativeClockReadV1 {
            owner: self.identity.clone(),
            nonce,
            current_height: self.current_height,
            started,
        })
    }

    /// Recheck this owner's exact move-only read and lend only its original transport targets.
    /// These public selectors do not authenticate an account, install a root or answer the read.
    /// # Errors
    /// Refuses a foreign/consumed read, expired native budget or changed owned journal.
    pub fn current_read_targets(
        &self,
        read: &KagemushaOrdinaryNativeClockReadV1,
    ) -> Result<(
        NonZeroU64,
        [KagemushaOrdinaryNativeClockNodeV1; 4],
        Duration,
    )> {
        self.recheck()?;
        if !Arc::ptr_eq(&read.owner, &self.identity)
            || read.current_height != self.current_height
            || self.consumed_nonces.contains(&read.nonce)
        {
            return Err(Rejected);
        }
        let elapsed = read.started.elapsed().map_err(|_| Custody)?;
        let budget = Duration::from_millis(self.selected.policy.maximum_reply_age_ms);
        let remaining = budget
            .checked_sub(elapsed)
            .filter(|value| !value.is_zero())
            .ok_or(Rejected)?;
        let height = NonZeroU64::new(self.current_height).ok_or(Rejected)?;
        Ok((height, self.selected.nodes.clone(), remaining))
    }

    /// Authenticate exact current signed replies under the retained root/node identities,
    /// original Native reply budget and governed skew, then fsync before retaining their median.
    /// # Errors
    /// Refuses foreign reservation, signature/root/runtime/nonce mismatch, expiry, replay or regression.
    pub fn admit_current_read(
        &mut self,
        read: KagemushaOrdinaryNativeClockReadV1,
        originals: [Vec<u8>; 4],
    ) -> Result<()> {
        self.recheck()?;
        if !Arc::ptr_eq(&read.owner, &self.identity)
            || read.current_height != self.current_height
            || self.consumed_nonces.contains(&read.nonce)
        {
            return Err(Rejected);
        }
        self.require_reply_budget(read.started)?;
        let (median_ms, certified_context_id) = self.verify_observation(read.nonce, &originals)?;
        // Every honest nonce-bound sample precedes this native reception/verification sample.
        // Count only elapsed after reception for the lower bound, never whole request latency.
        let received = NativeContinuousReading::now().map_err(|_| Custody)?;
        let high_water_ms = self.projected(read.started, median_ms)?;
        // A fresh lower bound must reach the durable ceiling; variable RTT cannot repair a
        // regressing signed reading by adding request latency or maxing with old public state.
        if median_ms < self.high_water_ms || high_water_ms < self.high_water_ms {
            return Err(Rejected);
        }
        let signed_observations_original_digest =
            signed_observation_digest(read.nonce, certified_context_id, &originals)?;
        let record = Record::Observation(Box::new(Observation {
            nonce: read.nonce,
            originals,
            median_ms,
            high_water_ms,
            certified_context_id,
        }));
        let original = encode(&record)?;
        self.require_reply_budget(read.started)?;
        self.append_original(&original)?;
        self.consumed_nonces.insert(read.nonce);
        self.nonce = Some(read.nonce);
        self.high_water_ms = high_water_ms;
        self.observation_digest = Sha256::digest(original).into();
        self.observation_median_ms = Some(median_ms);
        self.signed_observations_original_digest = signed_observations_original_digest;
        let Record::Observation(observation) = record else {
            return Err(Custody);
        };
        self.latest_observation = Some(Arc::new(*observation));
        self.reference = None;
        self.require_reply_budget(read.started)?;
        self.reference = Some(Reference {
            started: read.started,
            received,
            median_ms,
            last_lent_lower_ms: median_ms,
            last_lent_upper_ms: high_water_ms,
        });
        self.recheck()
    }

    /// Lend a current lower/upper interval from the same signed originals after durable publication.
    /// The lower bound advances from Native reply reception; the upper bound counts the whole
    /// nonce request. A future fsynced ceiling exists only for rollback detection and is never lent.
    /// # Errors
    /// Refuses unrefreshed recovery, expiry, regression, publication stall, drift or failed fsync.
    pub fn current_native_time_interval(
        &mut self,
    ) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        self.recheck()?;
        let reference = self.reference.as_ref().ok_or(Rejected)?;
        let (started, received, median, last_lower, last_upper) = (
            reference.started,
            reference.received,
            reference.median_ms,
            reference.last_lent_lower_ms,
            reference.last_lent_upper_ms,
        );
        let sampled = self.projected(started, median)?;
        if sampled < last_upper {
            return Err(Rejected);
        }
        let persistence_started = if sampled >= self.high_water_ms {
            let ceiling = sampled
                .checked_add(self.selected.policy.maximum_persistence_age_ms)
                .ok_or(Custody)?;
            let end = median
                .checked_add(self.selected.policy.maximum_projection_age_ms)
                .ok_or(Custody)?;
            if ceiling >= end {
                return Err(Rejected);
            }
            let publication = NativeContinuousReading::now().map_err(|_| Custody)?;
            self.append(&Record::Projected(Box::new(Projected {
                observation_digest: self.observation_digest,
                sampled_at_ms: sampled,
                high_water_ms: ceiling,
            })))?;
            self.high_water_ms = ceiling;
            #[cfg(test)]
            std::thread::sleep(self.persistence_delay);
            Some(publication)
        } else {
            None
        };
        self.recheck()?;
        // Sample lower first; a later upper sample cannot narrow the supported interval.
        let lower_ms = self.projected(received, median)?;
        let upper_ms = self.projected(started, median)?;
        if persistence_started.is_some_and(|publication| {
            publication.elapsed().map_or(true, |elapsed| {
                elapsed.as_millis() >= u128::from(self.selected.policy.maximum_persistence_age_ms)
            })
        }) || upper_ms > self.high_water_ms
            || lower_ms < last_lower
            || upper_ms < last_upper
            || lower_ms > upper_ms
        {
            return Err(Rejected);
        }
        let reference = self.reference.as_mut().ok_or(Rejected)?;
        reference.last_lent_lower_ms = lower_ms;
        reference.last_lent_upper_ms = upper_ms;
        Ok(KagemushaOrdinaryNativeTimeIntervalV1 { lower_ms, upper_ms })
    }

    /// Fixture-only scalar view of this owner's current native time interval.
    /// Returns the upper bound; shipping callers must retain and validate the full interval.
    /// # Errors
    /// Propagates the current interval's freshness, custody, persistence and range failures.
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    pub fn current_native_time_ms(&mut self) -> Result<u64> {
        self.current_native_time_interval()
            .map(|interval| interval.upper_ms())
    }

    /// Project data from this exact fresh signed clock owner for the ordinary cash transcript.
    /// It creates no clock, account, FI/current-State or monetary authority and takes no caller fields.
    /// # Errors
    /// Rejects unrefreshed recovery, absent original observations or the same current interval failures.
    pub fn current_cash_clock_context(
        &mut self,
    ) -> Result<iroha_data_model::kagemusha::KagemushaOrdinaryCashClockContextV1> {
        let interval = self.current_native_time_interval()?;
        let request_nonce = self.nonce.ok_or(Rejected)?;
        if self.signed_observations_original_digest == [0; 32] {
            return Err(Rejected);
        }
        Ok(
            iroha_data_model::kagemusha::KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce,
                signed_observations_original_digest: self.signed_observations_original_digest,
                lower_at_ms: interval.lower_ms(),
                upper_at_ms: interval.upper_ms(),
            },
        )
    }

    /// Exact retained certified prefix height for a fresh request under this same clock owner.
    /// Reading it grants no current account or FI authority.
    /// # Errors
    /// Refuses unavailable actual current interval or changed original journal custody.
    pub fn current_certified_height(&mut self) -> Result<u64> {
        self.current_native_time_interval()?;
        let height = self.current_height;
        self.recheck()?;
        self.current_native_time_interval()?;
        Ok(height)
    }

    /// Clone the verifier retained by this exact authenticated clock WAL for the same current
    /// certified prefix. This is proof checking material; it does not grant a Native account.
    /// # Errors
    /// Refuses changed journal custody or an unrefreshed/expired current software-clock reading.
    pub fn current_finality_verifier(&mut self) -> Result<SumeragiFinalityVerifier> {
        self.current_native_time_interval()?;
        let verifier = self.verifier.clone();
        self.recheck()?;
        self.current_native_time_interval()?;
        Ok(verifier)
    }

    /// Same original installed network, for the private account/release selection join.
    /// # Errors
    /// Refuses changed original WAL custody.
    pub fn network_id(&self) -> Result<NetworkId> {
        self.recheck()?;
        Ok(self.selected.network)
    }

    /// Exact original checkpoint/node/policy selection identity for the retained independent
    /// SDK/runtime inventory join. Reading it creates neither a root nor installation authority.
    /// # Errors
    /// Refuses changed original WAL custody.
    pub fn installed_selection_digest(&self) -> Result<[u8; 32]> {
        self.recheck()?;
        Ok(self.selected.digest)
    }

    /// Historical cryptographic prefix custody only; this private projection cannot restore
    /// an elapsed-clock reference or grant current FI/money authority after cold recovery.
    pub(crate) fn retained_finality_verifier_for_original_custody(
        &self,
    ) -> Result<SumeragiFinalityVerifier> {
        self.recheck()?;
        let verifier = self.verifier.clone();
        self.recheck()?;
        Ok(verifier)
    }

    fn recheck(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        if let Some(prefix) = self.prefix {
            if self.journal.recovery_prefix().map_err(|_| Custody)? != prefix {
                return Err(Custody);
            }
        }
        Ok(())
    }
    fn append(&mut self, record: &Record) -> Result<()> {
        self.append_original(&encode(record)?)
    }
    fn append_original(&mut self, original: &[u8]) -> Result<()> {
        self.recheck()?;
        if self.rows >= MAX_ROWS {
            return Err(Rejected);
        }
        self.journal.append(original).map_err(|_| Custody)?;
        self.rows += 1;
        self.prefix = Some(self.journal.recovery_prefix().map_err(|_| Custody)?);
        self.recheck()
    }
    fn require_reply_budget(&self, started: NativeContinuousReading) -> Result<()> {
        if started.elapsed().map_err(|_| Custody)?.as_millis()
            >= u128::from(self.selected.policy.maximum_reply_age_ms)
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn projected(&self, started: NativeContinuousReading, median_ms: u64) -> Result<u64> {
        let elapsed = started.elapsed().map_err(|_| Custody)?.as_millis();
        if elapsed >= u128::from(self.selected.policy.maximum_projection_age_ms) {
            return Err(Rejected);
        }
        // Count the whole original request interval. Subtracting network time could extend expiry.
        median_ms
            .checked_add(u64::try_from(elapsed).map_err(|_| Custody)?)
            .ok_or(Custody)
    }
    fn verify_observation(&self, nonce: [u8; 32], originals: &[Vec<u8>; 4]) -> Result<(u64, Hash)> {
        verify_signed_observations(
            &self.selected,
            &self.verifier,
            nonce,
            originals,
            Some(self.current_height),
        )
    }
}
fn verify_signed_observations(
    selected: &KagemushaOrdinaryNativeClockOriginalsV1,
    verifier: &SumeragiFinalityVerifier,
    nonce: [u8; 32],
    originals: &[Vec<u8>; 4],
    required_height: Option<u64>,
) -> Result<(u64, Hash)> {
    if nonce == [0; 32] {
        return Err(Rejected);
    }
    let mut times = [0; 4];
    let mut context = None;
    for (index, original) in originals.iter().enumerate() {
        if original.is_empty() || original.len() > MAX_FRAME / 4 {
            return Err(Rejected);
        }
        let reply: SumeragiFinalityAttestation = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(MAX_FRAME / 4),
        )
        .map_err(|_| Rejected)?;
        if norito::encode_canonical(&reply).map_err(|_| Rejected)? != *original {
            return Err(Rejected);
        }
        reply.verify().map_err(|_| Rejected)?;
        let body = &reply.body;
        let node = &selected.nodes[index];
        if body.challenge != nonce
            || body.network_id != selected.network
            || body.node_id != node.peer_id
            || body.build_fingerprint != node.build_fingerprint
            || body.config_fingerprint != node.config_fingerprint
            || body.status.config_fingerprint != node.config_fingerprint
            || body.status.instance != verifier.instance().0
            || body.status.signer.as_ref() != Some(node.peer_id.public_key())
            || body.status.unanchored
            || body.status.abstaining
            || body.status.halted.is_some()
            || required_height.is_some_and(|height| body.finality_proof.height() != height)
            || NetworkId::from_genesis_hash(body.genesis_block_hash) != selected.network
        {
            return Err(Rejected);
        }
        let block = verifier
            .verify_retained_decision(&body.finality_proof)
            .map_err(|_| Rejected)?;
        if context.is_some_and(|prior| prior != block.context_id()) {
            return Err(Rejected);
        }
        context = Some(block.context_id());
        times[index] = body.observed_at_unix_ms;
    }
    times.sort_unstable();
    if times[0] == 0 || times[3] - times[0] > selected.policy.maximum_node_skew_ms {
        return Err(Rejected);
    }
    Ok((
        times[1]
            .checked_add((times[2] - times[1]) / 2)
            .ok_or(Rejected)?,
        context.ok_or(Rejected)?,
    ))
}
fn encode(record: &Record) -> Result<Vec<u8>> {
    let original = norito::encode_canonical(record).map_err(|_| Rejected)?;
    if original.is_empty() || original.len() > MAX_FRAME {
        return Err(Rejected);
    }
    Ok(original)
}
fn decode(original: &[u8]) -> Result<Record> {
    if original.is_empty() || original.len() > MAX_FRAME {
        return Err(Rejected);
    }
    let record =
        norito::decode_canonical_with_limits(original, norito::canonical_decode_limits(MAX_FRAME))
            .map_err(|_| Rejected)?;
    if encode(&record)? != original {
        return Err(Rejected);
    }
    Ok(record)
}
fn decode_proof(original: &[u8]) -> Result<SumeragiFinalityProof> {
    if original.is_empty() || original.len() > MAX_FRAME {
        return Err(Rejected);
    }
    let proof =
        norito::decode_canonical_with_limits(original, norito::canonical_decode_limits(MAX_FRAME))
            .map_err(|_| Rejected)?;
    if norito::encode_canonical(&proof).map_err(|_| Rejected)? != original {
        return Err(Rejected);
    }
    Ok(proof)
}

#[cfg(test)]
#[path = "ordinary_native_clock_tests.rs"]
mod tests;

fn signed_observation_digest(
    nonce: [u8; 32],
    context: Hash,
    originals: &[Vec<u8>; 4],
) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    digest.update(b"iroha:kagemusha:v1:ordinary-native-clock-signed-observations\0");
    digest.update(nonce);
    digest.update(context.as_ref());
    for original in originals {
        digest.update(
            u32::try_from(original.len())
                .map_err(|_| Rejected)?
                .to_le_bytes(),
        );
        digest.update(original);
    }
    Ok(digest.finalize().into())
}
