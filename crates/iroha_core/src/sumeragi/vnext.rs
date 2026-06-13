//! Quarantined non-canonical Sumeragi control-frame state.
//!
//! Sumeragi V1 ingress rejects these control frames before they can drive
//! consensus decisions. The types remain available only so older sidecars and
//! local diagnostics can be decoded, labelled, and discarded deterministically.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, PrivateKey, Signature};
use iroha_data_model::{
    ChainId,
    block::{
        BlockHeader,
        consensus::{Height, View},
    },
    peer::PeerId,
};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use norito::codec::{Decode, Encode};

/// Consensus slot identity carried by non-canonical control messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SlotId {
    /// Block height.
    pub height: Height,
    /// Consensus view.
    pub view: View,
    /// NPoS epoch, or zero in permissioned mode.
    pub epoch: u64,
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
}

/// Explicit slot progress state used by quarantined control-frame diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotState {
    /// No proposal is active for this slot.
    Idle,
    /// A proposal header and payload hash have been accepted.
    Proposed {
        /// Proposed block hash.
        block_hash: HashOf<BlockHeader>,
        /// Data-availability payload hash.
        payload_hash: Hash,
    },
    /// The proposal is waiting for asynchronous validation.
    AwaitingValidation {
        /// Proposed block hash.
        block_hash: HashOf<BlockHeader>,
    },
    /// The slot has enough prepare evidence to continue.
    Prepared {
        /// Prepared block hash.
        block_hash: HashOf<BlockHeader>,
    },
    /// The slot has committed.
    Committed {
        /// Committed block hash.
        block_hash: HashOf<BlockHeader>,
    },
    /// The slot is in recovery after missing data or performance suspicion.
    Recovering {
        /// Block hash being recovered, when known.
        block_hash: Option<HashOf<BlockHeader>>,
    },
    /// The slot was abandoned by a certified view change or validation failure.
    Aborted {
        /// Human-readable reason label for diagnostics.
        reason_label: String,
    },
}

/// Validation roots produced by stateful block validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationRoots {
    /// State root before executing the block.
    pub parent_state_root: Hash,
    /// State root after executing the block.
    pub post_state_root: Hash,
}

/// Validation failure summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationFailure {
    /// Stable reason label for telemetry and tests.
    pub reason_label: String,
    /// Optional evidence hash when the failure has slashable evidence.
    pub evidence_hash: Option<Hash>,
}

/// Result returned by an asynchronous validation worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationWorkerResult {
    /// Worker assignment id.
    pub id: u64,
    /// Validation generation captured when the work was dispatched.
    pub generation: u64,
    /// Worker outcome.
    pub outcome: ValidationWorkerOutcome,
}

/// Outcome returned by an asynchronous validation worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationWorkerOutcome {
    /// Validation completed successfully.
    Valid(ValidationRoots),
    /// Validation rejected the block.
    Invalid(ValidationFailure),
}

/// Validation ownership state for one pending block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationState {
    /// No work has been queued.
    Unqueued,
    /// Work is queued but not known to be running.
    Queued {
        /// Worker assignment id.
        id: u64,
        /// Validation generation captured at queue time.
        generation: u64,
        /// Logical queue timestamp in milliseconds.
        queued_at_ms: u64,
    },
    /// Work is running in an asynchronous worker.
    Running {
        /// Worker assignment id.
        id: u64,
        /// Validation generation captured at dispatch time.
        generation: u64,
        /// Logical start timestamp in milliseconds.
        started_at_ms: u64,
    },
    /// Validation could not be queued because the worker lane was saturated.
    Backpressured {
        /// First observed backpressure timestamp in milliseconds.
        since_ms: u64,
    },
    /// Validation completed successfully.
    Valid {
        /// Validated roots.
        roots: ValidationRoots,
    },
    /// Validation completed with a rejection.
    Invalid {
        /// Failure summary.
        failure: ValidationFailure,
    },
}

impl ValidationState {
    /// Return the scheduling decision for this validation state at `now_ms`.
    #[must_use]
    pub fn decision_at(&self, now_ms: u64, config: &PerformanceFaultConfig) -> ValidationDecision {
        match self {
            Self::Unqueued => ValidationDecision::DispatchWorker,
            Self::Queued { .. } => ValidationDecision::AwaitWorker,
            Self::Running { started_at_ms, .. } => {
                if elapsed_ms(now_ms, *started_at_ms) >= config.suspicion_timeout_ms {
                    ValidationDecision::RaiseSuspicion
                } else {
                    ValidationDecision::AwaitWorker
                }
            }
            Self::Backpressured { since_ms } => {
                if elapsed_ms(now_ms, *since_ms) >= config.suspicion_timeout_ms {
                    ValidationDecision::RaiseSuspicion
                } else {
                    ValidationDecision::Backpressure
                }
            }
            Self::Valid { .. } => ValidationDecision::Accept,
            Self::Invalid { .. } => ValidationDecision::Reject,
        }
    }

    /// Mark the validation as running in a worker.
    #[must_use]
    pub fn worker_started(self, id: u64, generation: u64, started_at_ms: u64) -> Self {
        Self::Running {
            id,
            generation,
            started_at_ms,
        }
    }

    /// Apply a worker result if it still matches the owned worker generation.
    pub fn apply_worker_result(&mut self, result: ValidationWorkerResult) -> WorkerResultAction {
        let Self::Running { id, generation, .. } = self else {
            return WorkerResultAction::IgnoredStale;
        };
        if *id != result.id || *generation != result.generation {
            return WorkerResultAction::IgnoredStale;
        }
        *self = match result.outcome {
            ValidationWorkerOutcome::Valid(roots) => Self::Valid { roots },
            ValidationWorkerOutcome::Invalid(failure) => Self::Invalid { failure },
        };
        WorkerResultAction::Applied
    }
}

/// Scheduling decision for validation ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationDecision {
    /// Dispatch asynchronous worker validation.
    DispatchWorker,
    /// Wait for the running worker result.
    AwaitWorker,
    /// Emit a signed performance suspicion and enter recovery.
    RaiseSuspicion,
    /// Keep the block deferred because worker lanes are backpressured.
    Backpressure,
    /// Accept the validated block.
    Accept,
    /// Reject the invalid block.
    Reject,
}

/// Action taken when applying a worker result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerResultAction {
    /// The result matched the current owner and was applied.
    Applied,
    /// The result was stale or from a superseded generation.
    IgnoredStale,
}

/// Performance-fault configuration for vNext.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PerformanceFaultConfig {
    /// Number of samples in the EWMA performance window.
    pub performance_window_samples: u16,
    /// Hard suspicion timeout in milliseconds.
    pub suspicion_timeout_ms: u64,
    /// Performance threshold in basis points over the EWMA baseline.
    pub performance_threshold_bps: u16,
    /// Maximum validators that may be tainted in one view before view change.
    pub max_tainted_per_view: u16,
    /// Minimum delay between accepted re-chainings in milliseconds.
    pub rechain_cooldown_ms: u64,
}

impl Default for PerformanceFaultConfig {
    fn default() -> Self {
        use iroha_config::parameters::defaults::sumeragi;

        Self {
            performance_window_samples: sumeragi::VNEXT_PERFORMANCE_WINDOW_SAMPLES,
            suspicion_timeout_ms: sumeragi::VNEXT_SUSPICION_TIMEOUT_MS,
            performance_threshold_bps: sumeragi::VNEXT_PERFORMANCE_THRESHOLD_BPS,
            max_tainted_per_view: sumeragi::VNEXT_MAX_TAINTED_PER_VIEW,
            rechain_cooldown_ms: sumeragi::VNEXT_RECHAIN_COOLDOWN_MS,
        }
    }
}

impl From<&iroha_config::parameters::actual::SumeragiVNext> for PerformanceFaultConfig {
    fn from(config: &iroha_config::parameters::actual::SumeragiVNext) -> Self {
        Self {
            performance_window_samples: config.performance_window_samples,
            suspicion_timeout_ms: duration_millis(config.suspicion_timeout),
            performance_threshold_bps: config.performance_threshold_bps,
            max_tainted_per_view: config.max_tainted_per_view,
            rechain_cooldown_ms: duration_millis(config.rechain_cooldown),
        }
    }
}

impl From<iroha_config::parameters::actual::SumeragiVNext> for PerformanceFaultConfig {
    fn from(config: iroha_config::parameters::actual::SumeragiVNext) -> Self {
        Self::from(&config)
    }
}

/// Deterministic validator chain order used by vNext.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ChainOrder {
    /// Height associated with the order.
    pub height: Height,
    /// View associated with the order.
    pub view: View,
    /// Epoch associated with the order.
    pub epoch: u64,
    /// Re-chain sequence within the view.
    pub rechain_seq: u64,
    /// Ordered validators.
    pub ordered_validators: Vec<PeerId>,
    /// Number of validators in the critical path.
    pub critical_prefix_len: u16,
    /// Index where the quarantine tail starts.
    pub quarantine_start: u16,
}

impl ChainOrder {
    /// Build and validate a chain order.
    ///
    /// # Errors
    ///
    /// Returns [`ChainOrderError`] when the order is empty or its critical and
    /// quarantine bounds are inconsistent.
    pub fn new(
        height: Height,
        view: View,
        epoch: u64,
        rechain_seq: u64,
        ordered_validators: Vec<PeerId>,
        critical_prefix_len: u16,
        quarantine_start: u16,
    ) -> Result<Self, ChainOrderError> {
        if ordered_validators.is_empty() {
            return Err(ChainOrderError::EmptyOrder);
        }
        let len = ordered_validators.len();
        let critical = usize::from(critical_prefix_len);
        let quarantine = usize::from(quarantine_start);
        if critical == 0 || critical > len {
            return Err(ChainOrderError::InvalidCriticalPrefix);
        }
        if quarantine < critical || quarantine > len {
            return Err(ChainOrderError::InvalidQuarantineStart);
        }
        Ok(Self {
            height,
            view,
            epoch,
            rechain_seq,
            ordered_validators,
            critical_prefix_len,
            quarantine_start,
        })
    }

    /// Return the deterministic Norito hash of this chain order.
    #[must_use]
    pub fn hash(&self) -> Hash {
        let bytes = norito::to_bytes(self).expect("chain order should encode");
        Hash::new(&bytes)
    }

    /// Return the validators currently on the critical path.
    #[must_use]
    pub fn critical_path(&self) -> &[PeerId] {
        &self.ordered_validators[..usize::from(self.critical_prefix_len)]
    }

    /// Return the successor of a critical-path peer, excluding the proxy tail.
    #[must_use]
    pub fn successor_of(&self, peer: &PeerId) -> Option<&PeerId> {
        let pos = self
            .ordered_validators
            .iter()
            .position(|candidate| candidate == peer)?;
        let successor_pos = pos.checked_add(1)?;
        (successor_pos < usize::from(self.critical_prefix_len))
            .then(|| &self.ordered_validators[successor_pos])
    }

    /// Apply successor-scoped suspicions and return the deterministic re-chain
    /// certificate body.
    ///
    /// All suspicions must target this order's slot, chain-order hash, and
    /// re-chain sequence. The evidence is then canonicalized by the suspicion
    /// signing-body hash and applied sequentially to the evolving order. If an
    /// earlier suspicion changes the order so a later suspicion is no longer
    /// successor-scoped, the whole re-chain is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`RechainError`] when evidence is empty, belongs to another
    /// order, is not successor-scoped after deterministic ordering, or
    /// quarantine would weaken the required quorum.
    pub fn rechain_after_suspicions(
        &self,
        suspicions: Vec<Suspect>,
        quorum: &QuorumPolicy,
    ) -> Result<RechainCertificate, RechainError> {
        if suspicions.is_empty() {
            return Err(RechainError::EmptyEvidence);
        }

        let previous_chain_order_hash = self.hash();
        let mut canonical = BTreeMap::new();
        for suspect in suspicions {
            self.validate_successor_suspicion(&suspect)?;
            let evidence_hash = suspect
                .signing_body_hash()
                .ok_or(RechainError::CanonicalEvidenceEncoding)?;
            if canonical.insert(evidence_hash, suspect).is_some() {
                return Err(RechainError::DuplicateEvidence);
            }
        }

        let ordered_suspicions = canonical.into_values().collect::<Vec<_>>();
        let mut current = self.clone();
        let mut tainted = Vec::with_capacity(ordered_suspicions.len().saturating_mul(2));
        let mut tainted_peers = Vec::<PeerId>::with_capacity(tainted.capacity());

        for suspect in &ordered_suspicions {
            current.validate_successor_pair(&suspect.accuser, &suspect.accused)?;
            push_tainted(
                &mut tainted,
                &mut tainted_peers,
                suspect.accuser.clone(),
                TaintReason::Accuser,
            );
            push_tainted(
                &mut tainted,
                &mut tainted_peers,
                suspect.accused.clone(),
                TaintReason::Accused,
            );
            current = current.rebuild_with_tainted_tail(&tainted_peers, quorum)?;
        }

        Ok(RechainCertificate {
            slot: ordered_suspicions[0].slot,
            previous_chain_order_hash,
            new_chain_order_hash: current.hash(),
            new_order: current.clone(),
            rechain_seq: current.rechain_seq,
            tainted,
            suspicions: ordered_suspicions,
            signer_bitmap: Vec::new(),
            aggregate_signature: Vec::new(),
        })
    }

    /// Validate that `suspect` is scoped to this order and accuses the
    /// accuser's current critical-path successor.
    ///
    /// # Errors
    ///
    /// Returns [`RechainError`] when the suspicion belongs to another slot or
    /// re-chain sequence, or when it is not successor-scoped.
    pub fn validate_successor_suspicion(&self, suspect: &Suspect) -> Result<(), RechainError> {
        self.validate_suspicion(suspect)?;
        self.validate_successor_pair(&suspect.accuser, &suspect.accused)
    }

    fn validate_successor_pair(
        &self,
        accuser: &PeerId,
        accused: &PeerId,
    ) -> Result<(), RechainError> {
        let expected_successor = self
            .successor_of(accuser)
            .cloned()
            .ok_or(RechainError::AccuserHasNoCriticalSuccessor)?;
        if &expected_successor != accused {
            return Err(RechainError::AccusedIsNotSuccessor {
                expected: expected_successor,
            });
        }
        Ok(())
    }

    fn validate_suspicion(&self, suspect: &Suspect) -> Result<(), RechainError> {
        if suspect.slot.height != self.height
            || suspect.slot.view != self.view
            || suspect.slot.epoch != self.epoch
        {
            return Err(RechainError::SlotMismatch);
        }
        if suspect.rechain_seq != self.rechain_seq {
            return Err(RechainError::RechainSequenceMismatch);
        }
        if suspect.chain_order_hash != self.hash() {
            return Err(RechainError::ChainOrderHashMismatch);
        }
        Ok(())
    }

    fn rebuild_with_tainted_tail(
        &self,
        tainted_peers: &[PeerId],
        quorum: &QuorumPolicy,
    ) -> Result<Self, RechainError> {
        let mut untainted = Vec::with_capacity(self.ordered_validators.len());
        for peer in &self.ordered_validators {
            if !tainted_peers.iter().any(|tainted| tainted == peer) {
                untainted.push(peer.clone());
            }
        }

        if untainted.len() < usize::from(self.critical_prefix_len) {
            return Err(RechainError::InsufficientUntaintedValidators {
                untainted: untainted.len(),
                critical_prefix_len: usize::from(self.critical_prefix_len),
            });
        }

        let rechain_seq = self
            .rechain_seq
            .checked_add(1)
            .ok_or(RechainError::RechainSequenceExhausted)?;
        let mut reordered = untainted;
        reordered.extend(tainted_peers.iter().cloned());
        let next = Self::new(
            self.height,
            self.view,
            self.epoch,
            rechain_seq,
            reordered,
            self.critical_prefix_len,
            self.quarantine_start,
        )
        .expect("reordered chain must preserve validated bounds");

        if !quorum.satisfied_by(next.critical_path()) {
            return Err(RechainError::InsufficientQuorumAfterQuarantine);
        }

        Ok(next)
    }
}

fn push_tainted(
    tainted: &mut Vec<TaintedValidator>,
    tainted_peers: &mut Vec<PeerId>,
    peer_id: PeerId,
    reason: TaintReason,
) {
    if tainted_peers.iter().any(|peer| peer == &peer_id) {
        return;
    }
    tainted_peers.push(peer_id.clone());
    tainted.push(TaintedValidator { peer_id, reason });
}

/// Chain-order construction error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainOrderError {
    /// The validator order was empty.
    EmptyOrder,
    /// The critical prefix was zero or longer than the order.
    InvalidCriticalPrefix,
    /// The quarantine tail started before the critical prefix or past the end.
    InvalidQuarantineStart,
}

/// Validator quorum policy.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum QuorumPolicy {
    /// Count-based quorum for permissioned mode.
    Count {
        /// Required number of critical-path validators.
        required: u16,
    },
    /// Stake-based quorum for NPoS mode.
    Stake {
        /// Total active stake in the validator roster.
        total: Numeric,
        /// Stake weights by validator.
        weights: Vec<StakeWeight>,
    },
}

impl QuorumPolicy {
    /// Return whether `validators` satisfy this quorum policy.
    #[must_use]
    pub fn satisfied_by(&self, validators: &[PeerId]) -> bool {
        match self {
            Self::Count { required } => validators.len() >= usize::from(*required),
            Self::Stake { total, weights } => stake_quorum_satisfied(total, weights, validators),
        }
    }

    /// Return the smallest deterministic prefix of `validators` that satisfies
    /// this quorum policy.
    #[must_use]
    pub fn smallest_satisfying_prefix_len(&self, validators: &[PeerId]) -> Option<u16> {
        let mut prefix_len = 1usize;
        while prefix_len <= validators.len() {
            if self.satisfied_by(&validators[..prefix_len]) {
                return u16::try_from(prefix_len).ok();
            }
            prefix_len = prefix_len.saturating_add(1);
        }
        None
    }
}

/// Stake weight assigned to one validator.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct StakeWeight {
    /// Validator peer id.
    pub peer_id: PeerId,
    /// Stake weight used for quorum checks.
    pub weight: Numeric,
}

fn stake_quorum_satisfied(total: &Numeric, weights: &[StakeWeight], validators: &[PeerId]) -> bool {
    if total.is_zero() {
        return false;
    }
    let mut stake = Numeric::from(0_u64);
    for validator in validators {
        let Some(weight) = stake_weight(weights, validator) else {
            return false;
        };
        stake = match stake.checked_add(weight) {
            Some(stake) => stake,
            None => return false,
        };
    }
    let Some(signed_scaled) = stake.checked_mul(Numeric::from(3_u64), NumericSpec::default())
    else {
        return false;
    };
    let Some(total_scaled) = total
        .clone()
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
    else {
        return false;
    };
    signed_scaled > total_scaled
}

/// A missed obligation that can justify performance suspicion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum MissedObligation {
    /// The successor did not forward a proposal in time.
    ForwardProposal,
    /// The successor did not acknowledge validation progress in time.
    AckValidation,
    /// The successor did not emit RBC READY in time.
    RbcReady,
    /// The successor did not relay votes in time.
    VoteRelay,
    /// The successor did not relay commit acknowledgement in time.
    CommitAck,
}

/// Signed suspicion raised by one validator against its successor.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct Suspect {
    /// Slot where the obligation was missed.
    pub slot: SlotId,
    /// Validator raising the suspicion.
    pub accuser: PeerId,
    /// Successor being accused.
    pub accused: PeerId,
    /// Missed obligation.
    pub obligation: MissedObligation,
    /// Hash of the chain order used to interpret successor scope.
    pub chain_order_hash: Hash,
    /// Re-chain sequence in which the suspicion was produced.
    pub rechain_seq: u64,
    /// Observed delay in milliseconds.
    pub observed_delay_ms: u64,
    /// Signature over the canonical suspicion preimage.
    pub signature: Vec<u8>,
}

impl Suspect {
    fn signing_body(&self) -> SuspectSigningBody {
        SuspectSigningBody {
            slot: self.slot,
            accuser: self.accuser.clone(),
            accused: self.accused.clone(),
            obligation: self.obligation,
            chain_order_hash: self.chain_order_hash,
            rechain_seq: self.rechain_seq,
            observed_delay_ms: self.observed_delay_ms,
        }
    }

    fn signing_body_hash(&self) -> Option<Hash> {
        norito::to_bytes(&self.signing_body())
            .ok()
            .map(|bytes| Hash::new(&bytes))
    }
}

/// Validator vote accepting a proposed re-chain certificate body.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RechainVote {
    /// Slot being re-chained.
    pub slot: SlotId,
    /// Previous chain-order hash.
    pub previous_chain_order_hash: Hash,
    /// New chain-order hash.
    pub new_chain_order_hash: Hash,
    /// New chain order accepted by this vote.
    pub new_order: ChainOrder,
    /// Re-chain sequence accepted by this vote.
    pub rechain_seq: u64,
    /// Validators tainted by this re-chain.
    pub tainted: Vec<TaintedValidator>,
    /// Suspicion messages that triggered this re-chain.
    pub suspicions: Vec<Suspect>,
    /// Validator signing the certificate body.
    pub signer: PeerId,
    /// BLS signature over the canonical re-chain certificate preimage.
    pub signature: Vec<u8>,
}

impl RechainVote {
    /// Build an unsigned vote from the deterministic re-chain certificate body.
    #[must_use]
    pub fn unsigned(certificate: &RechainCertificate, signer: PeerId) -> Self {
        Self {
            slot: certificate.slot,
            previous_chain_order_hash: certificate.previous_chain_order_hash,
            new_chain_order_hash: certificate.new_chain_order_hash,
            new_order: certificate.new_order.clone(),
            rechain_seq: certificate.rechain_seq,
            tainted: certificate.tainted.clone(),
            suspicions: certificate.suspicions.clone(),
            signer,
            signature: Vec::new(),
        }
    }

    /// Return the canonical preimage that can later be aggregated into a
    /// [`RechainCertificate`].
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded.
    pub fn signing_preimage(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
    ) -> Result<Vec<u8>, VNextSignatureError> {
        rechain_certificate_signing_preimage(
            chain_id,
            mode_tag,
            self.slot,
            self.previous_chain_order_hash,
            self.new_chain_order_hash,
            &self.new_order,
            self.rechain_seq,
            &self.tainted,
            &self.suspicions,
        )
    }

    /// Sign this vote with the validator's private key.
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded, or [`VNextSignatureError::Signing`] if the signing
    /// backend rejects the private key or preimage.
    pub fn sign(
        &mut self,
        chain_id: &ChainId,
        mode_tag: &str,
        private_key: &PrivateKey,
    ) -> Result<(), VNextSignatureError> {
        let preimage = self.signing_preimage(chain_id, mode_tag)?;
        self.signature = Signature::try_new(private_key, &preimage)
            .map_err(|_| VNextSignatureError::Signing)?
            .payload()
            .to_vec();
        Ok(())
    }
}

/// Validator tainted by a re-chain.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TaintedValidator {
    /// Tainted validator peer id.
    pub peer_id: PeerId,
    /// Why the validator was tainted.
    pub reason: TaintReason,
}

/// Reason a validator entered the quarantine tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum TaintReason {
    /// The validator raised a suspicion.
    Accuser,
    /// The validator was accused by its predecessor.
    Accused,
}

/// Certificate proving that validators accepted a new chain order.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RechainCertificate {
    /// Slot being re-chained.
    pub slot: SlotId,
    /// Previous chain-order hash.
    pub previous_chain_order_hash: Hash,
    /// New chain-order hash.
    pub new_chain_order_hash: Hash,
    /// New chain order accepted by the certificate.
    pub new_order: ChainOrder,
    /// Re-chain sequence certified by this message.
    pub rechain_seq: u64,
    /// Validators tainted by this re-chain.
    pub tainted: Vec<TaintedValidator>,
    /// Suspicion messages that triggered this re-chain.
    pub suspicions: Vec<Suspect>,
    /// Compact signer bitmap.
    pub signer_bitmap: Vec<u8>,
    /// Aggregate signature over the certificate.
    pub aggregate_signature: Vec<u8>,
}

impl RechainCertificate {
    /// Return the canonical vNext re-chain certificate aggregate-signing preimage.
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded.
    pub fn signing_preimage(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
    ) -> Result<Vec<u8>, VNextSignatureError> {
        rechain_certificate_signing_preimage(
            chain_id,
            mode_tag,
            self.slot,
            self.previous_chain_order_hash,
            self.new_chain_order_hash,
            &self.new_order,
            self.rechain_seq,
            &self.tainted,
            &self.suspicions,
        )
    }

    /// Verify this certificate's aggregate BLS signature and quorum.
    ///
    /// The caller supplies the signer roster that the bitmap indexes. The helper
    /// rejects malformed bitmaps instead of ignoring out-of-range bits so network
    /// messages are not malleable.
    ///
    /// # Errors
    ///
    /// Returns a [`VNextSignatureError`] when the certificate body is
    /// inconsistent, the signer bitmap is malformed, quorum is not met, or the
    /// aggregate signature does not verify.
    pub fn verify_aggregate_signature(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
        signer_roster: &[PeerId],
        signer_pops: &[&[u8]],
        quorum: &QuorumPolicy,
    ) -> Result<Vec<PeerId>, VNextSignatureError> {
        self.validate_body_consistency()?;
        verify_preaggregated_vnext_signature(
            self.signing_preimage(chain_id, mode_tag)?,
            &self.signer_bitmap,
            &self.aggregate_signature,
            signer_roster,
            signer_pops,
            quorum,
        )
    }

    fn validate_body_consistency(&self) -> Result<(), VNextSignatureError> {
        if self.slot.height != self.new_order.height
            || self.slot.view != self.new_order.view
            || self.slot.epoch != self.new_order.epoch
        {
            return Err(VNextSignatureError::SlotMismatch);
        }
        if self.new_chain_order_hash != self.new_order.hash() {
            return Err(VNextSignatureError::ChainOrderHashMismatch);
        }
        if self.rechain_seq != self.new_order.rechain_seq {
            return Err(VNextSignatureError::RechainSequenceMismatch);
        }
        Ok(())
    }
}

/// Validator vote accepting a new-view certificate body.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ViewChangeVote {
    /// New view number.
    pub new_view: View,
    /// Highest certified slot known to the signer.
    pub highest_slot: Option<SlotId>,
    /// Chain order hash carried into the new view.
    pub chain_order_hash: Hash,
    /// Validator signing the view-change body.
    pub signer: PeerId,
    /// BLS signature over the canonical view-change certificate preimage.
    pub signature: Vec<u8>,
}

impl ViewChangeVote {
    /// Build an unsigned vote from a view-change certificate body.
    #[must_use]
    pub fn unsigned(certificate: &ViewChangeCertificate, signer: PeerId) -> Self {
        Self {
            new_view: certificate.new_view,
            highest_slot: certificate.highest_slot,
            chain_order_hash: certificate.chain_order_hash,
            signer,
            signature: Vec::new(),
        }
    }

    /// Return the canonical preimage that can later be aggregated into a
    /// [`ViewChangeCertificate`].
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded, or [`VNextSignatureError::Signing`] if the signing
    /// backend rejects the private key or preimage.
    pub fn signing_preimage(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
    ) -> Result<Vec<u8>, VNextSignatureError> {
        view_change_certificate_signing_preimage(
            chain_id,
            mode_tag,
            self.new_view,
            self.highest_slot,
            self.chain_order_hash,
        )
    }

    /// Sign this vote with the validator's private key.
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded.
    pub fn sign(
        &mut self,
        chain_id: &ChainId,
        mode_tag: &str,
        private_key: &PrivateKey,
    ) -> Result<(), VNextSignatureError> {
        let preimage = self.signing_preimage(chain_id, mode_tag)?;
        self.signature = Signature::try_new(private_key, &preimage)
            .map_err(|_| VNextSignatureError::Signing)?
            .payload()
            .to_vec();
        Ok(())
    }
}

/// View-change certificate for replacing a faulty head.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ViewChangeCertificate {
    /// New view number.
    pub new_view: View,
    /// Highest certified slot known to the signers.
    pub highest_slot: Option<SlotId>,
    /// Chain order hash carried into the new view.
    pub chain_order_hash: Hash,
    /// Compact signer bitmap.
    pub signer_bitmap: Vec<u8>,
    /// Aggregate signature over the view-change certificate.
    pub aggregate_signature: Vec<u8>,
}

impl ViewChangeCertificate {
    /// Return the canonical vNext view-change aggregate-signing preimage.
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded.
    pub fn signing_preimage(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
    ) -> Result<Vec<u8>, VNextSignatureError> {
        view_change_certificate_signing_preimage(
            chain_id,
            mode_tag,
            self.new_view,
            self.highest_slot,
            self.chain_order_hash,
        )
    }

    /// Verify this certificate's aggregate BLS signature and quorum.
    ///
    /// # Errors
    ///
    /// Returns a [`VNextSignatureError`] when the signer bitmap is malformed,
    /// quorum is not met, or the aggregate signature does not verify.
    pub fn verify_aggregate_signature(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
        signer_roster: &[PeerId],
        signer_pops: &[&[u8]],
        quorum: &QuorumPolicy,
    ) -> Result<Vec<PeerId>, VNextSignatureError> {
        verify_preaggregated_vnext_signature(
            self.signing_preimage(chain_id, mode_tag)?,
            &self.signer_bitmap,
            &self.aggregate_signature,
            signer_roster,
            signer_pops,
            quorum,
        )
    }
}

/// Signature/certificate validation error for vNext control messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VNextSignatureError {
    /// An aggregate signature field was empty.
    MissingAggregateSignature,
    /// An aggregate signature failed BLS verification.
    BadAggregateSignature,
    /// The canonical signing body could not be encoded.
    CanonicalEncoding,
    /// The signing backend rejected the private key or preimage.
    Signing,
    /// Signer roster is empty.
    EmptySignerRoster,
    /// Signer bitmap selected no signers.
    EmptySignerSet,
    /// Signer bitmap length was not canonical for the roster.
    SignerBitmapLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Signer bitmap referenced a signer outside the roster.
    SignerBitmapOutOfRange {
        /// Signer index set in the bitmap.
        index: usize,
        /// Roster length.
        roster_len: usize,
    },
    /// Signer proof-of-possession list was not aligned with the signer roster.
    SignerPopLength {
        /// Expected number of PoP entries.
        expected: usize,
        /// Actual number of PoP entries.
        actual: usize,
    },
    /// Caller attempted to build a bitmap with an out-of-range signer index.
    SignerIndexOutOfRange {
        /// Signer index requested.
        index: usize,
        /// Roster length.
        roster_len: usize,
    },
    /// Caller attempted to build a bitmap with the same signer twice.
    DuplicateSignerIndex {
        /// Duplicated signer index.
        index: usize,
    },
    /// Aggregate verification currently accepts only BLS normal validator keys.
    UnsupportedAggregateKeyAlgorithm {
        /// Unsupported public-key algorithm.
        algorithm: Algorithm,
    },
    /// Aggregate verification saw a malformed public-key envelope.
    MalformedAggregatePublicKey,
    /// Signers selected by the bitmap did not satisfy the quorum policy.
    QuorumNotMet {
        /// Number of signers selected by the bitmap.
        signer_count: usize,
    },
    /// Certificate slot and embedded order do not agree.
    SlotMismatch,
    /// Certificate chain-order hash does not match the embedded order.
    ChainOrderHashMismatch,
    /// Certificate re-chain sequence does not match the embedded order.
    RechainSequenceMismatch,
}

/// Build a canonical little-endian signer bitmap from signer indices.
///
/// # Errors
///
/// Returns [`VNextSignatureError::SignerIndexOutOfRange`] for indices outside
/// the roster and [`VNextSignatureError::DuplicateSignerIndex`] for duplicates.
pub fn build_signer_bitmap(
    signer_indices: &[usize],
    roster_len: usize,
) -> Result<Vec<u8>, VNextSignatureError> {
    let mut bitmap = vec![0u8; signer_bitmap_len(roster_len)];
    let mut seen = std::collections::BTreeSet::new();
    for &index in signer_indices {
        if index >= roster_len {
            return Err(VNextSignatureError::SignerIndexOutOfRange { index, roster_len });
        }
        if !seen.insert(index) {
            return Err(VNextSignatureError::DuplicateSignerIndex { index });
        }
        bitmap[index / 8] |= 1u8 << (index % 8);
    }
    Ok(bitmap)
}

fn signer_peers_from_bitmap(
    signer_bitmap: &[u8],
    signer_roster: &[PeerId],
) -> Result<Vec<(usize, PeerId)>, VNextSignatureError> {
    if signer_roster.is_empty() {
        return Err(VNextSignatureError::EmptySignerRoster);
    }
    let expected = signer_bitmap_len(signer_roster.len());
    if signer_bitmap.len() != expected {
        return Err(VNextSignatureError::SignerBitmapLength {
            expected,
            actual: signer_bitmap.len(),
        });
    }

    let mut signers = Vec::new();
    for (byte_idx, byte) in signer_bitmap.iter().copied().enumerate() {
        for bit in 0u8..8 {
            if byte & (1u8 << bit) == 0 {
                continue;
            }
            let index = byte_idx * 8 + usize::from(bit);
            let Some(peer) = signer_roster.get(index) else {
                return Err(VNextSignatureError::SignerBitmapOutOfRange {
                    index,
                    roster_len: signer_roster.len(),
                });
            };
            signers.push((index, peer.clone()));
        }
    }
    if signers.is_empty() {
        return Err(VNextSignatureError::EmptySignerSet);
    }
    Ok(signers)
}

fn verify_preaggregated_vnext_signature(
    preimage: Vec<u8>,
    signer_bitmap: &[u8],
    aggregate_signature: &[u8],
    signer_roster: &[PeerId],
    signer_pops: &[&[u8]],
    quorum: &QuorumPolicy,
) -> Result<Vec<PeerId>, VNextSignatureError> {
    if aggregate_signature.is_empty() {
        return Err(VNextSignatureError::MissingAggregateSignature);
    }
    if signer_pops.len() != signer_roster.len() {
        return Err(VNextSignatureError::SignerPopLength {
            expected: signer_roster.len(),
            actual: signer_pops.len(),
        });
    }
    let signer_entries = signer_peers_from_bitmap(signer_bitmap, signer_roster)?;
    let signers = signer_entries
        .iter()
        .map(|(_, signer)| signer.clone())
        .collect::<Vec<_>>();
    if !quorum.satisfied_by(&signers) {
        return Err(VNextSignatureError::QuorumNotMet {
            signer_count: signers.len(),
        });
    }
    for signer in &signers {
        let algorithm = signer
            .public_key()
            .try_algorithm()
            .map_err(|_| VNextSignatureError::MalformedAggregatePublicKey)?;
        if algorithm != Algorithm::BlsNormal {
            return Err(VNextSignatureError::UnsupportedAggregateKeyAlgorithm { algorithm });
        }
    }
    let public_keys = signers
        .iter()
        .map(|signer| signer.public_key())
        .collect::<Vec<_>>();
    let signer_pop_refs = signer_entries
        .iter()
        .map(|(index, _)| signer_pops[*index])
        .collect::<Vec<_>>();
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        aggregate_signature,
        &public_keys,
        &signer_pop_refs,
    )
    .map_err(|_| VNextSignatureError::BadAggregateSignature)?;
    Ok(signers)
}

fn signer_bitmap_len(roster_len: usize) -> usize {
    roster_len.div_ceil(8)
}

fn vnext_signing_preimage<T: Encode>(
    chain_id: &ChainId,
    mode_tag: &str,
    message_type_tag: &str,
    body: &T,
) -> Result<Vec<u8>, VNextSignatureError> {
    let body = norito::to_bytes(body).map_err(|_| VNextSignatureError::CanonicalEncoding)?;
    let domain = crate::sumeragi::consensus::consensus_domain(
        chain_id,
        message_type_tag,
        b"vnext-v1",
        mode_tag,
    );
    let mut preimage = Vec::with_capacity(domain.len() + body.len());
    preimage.extend_from_slice(&domain);
    preimage.extend_from_slice(&body);
    Ok(preimage)
}

#[allow(clippy::too_many_arguments)]
fn rechain_certificate_signing_preimage(
    chain_id: &ChainId,
    mode_tag: &str,
    slot: SlotId,
    previous_chain_order_hash: Hash,
    new_chain_order_hash: Hash,
    new_order: &ChainOrder,
    rechain_seq: u64,
    tainted: &[TaintedValidator],
    suspicions: &[Suspect],
) -> Result<Vec<u8>, VNextSignatureError> {
    let body = RechainCertificateSigningBody {
        slot,
        previous_chain_order_hash,
        new_chain_order_hash,
        new_order: new_order.clone(),
        rechain_seq,
        tainted: tainted.to_vec(),
        suspicions: suspicions.to_vec(),
    };
    vnext_signing_preimage(chain_id, mode_tag, "RechainCertificate", &body)
}

fn view_change_certificate_signing_preimage(
    chain_id: &ChainId,
    mode_tag: &str,
    new_view: View,
    highest_slot: Option<SlotId>,
    chain_order_hash: Hash,
) -> Result<Vec<u8>, VNextSignatureError> {
    let body = ViewChangeCertificateSigningBody {
        new_view,
        highest_slot,
        chain_order_hash,
    };
    vnext_signing_preimage(chain_id, mode_tag, "ViewChangeCertificate", &body)
}

#[derive(Clone, Debug, Decode, Encode)]
struct SuspectSigningBody {
    slot: SlotId,
    accuser: PeerId,
    accused: PeerId,
    obligation: MissedObligation,
    chain_order_hash: Hash,
    rechain_seq: u64,
    observed_delay_ms: u64,
}

#[derive(Clone, Debug, Decode, Encode)]
struct RechainCertificateSigningBody {
    slot: SlotId,
    previous_chain_order_hash: Hash,
    new_chain_order_hash: Hash,
    new_order: ChainOrder,
    rechain_seq: u64,
    tainted: Vec<TaintedValidator>,
    suspicions: Vec<Suspect>,
}

#[derive(Clone, Debug, Decode, Encode)]
struct ViewChangeCertificateSigningBody {
    new_view: View,
    highest_slot: Option<SlotId>,
    chain_order_hash: Hash,
}

/// Re-chain validation error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RechainError {
    /// Re-chain evidence was empty.
    EmptyEvidence,
    /// Re-chain evidence could not be canonically encoded.
    CanonicalEvidenceEncoding,
    /// Re-chain evidence repeated the same canonical suspicion body.
    DuplicateEvidence,
    /// Suspicion slot does not match the chain order.
    SlotMismatch,
    /// Suspicion chain-order hash does not match the local order.
    ChainOrderHashMismatch,
    /// Suspicion sequence does not match the local re-chain sequence.
    RechainSequenceMismatch,
    /// The accuser is at the proxy tail and has no critical successor.
    AccuserHasNoCriticalSuccessor,
    /// The accused validator is not the accuser's successor.
    AccusedIsNotSuccessor {
        /// Expected successor.
        expected: PeerId,
    },
    /// Quarantine would leave too few untainted validators for the critical path.
    InsufficientUntaintedValidators {
        /// Number of validators left untainted.
        untainted: usize,
        /// Required critical prefix length.
        critical_prefix_len: usize,
    },
    /// Quarantine would violate the configured quorum policy.
    InsufficientQuorumAfterQuarantine,
    /// Re-chain sequence overflowed.
    RechainSequenceExhausted,
}

fn elapsed_ms(now_ms: u64, started_at_ms: u64) -> u64 {
    now_ms.saturating_sub(started_at_ms)
}

fn duration_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn stake_weight(weights: &[StakeWeight], validator: &PeerId) -> Option<Numeric> {
    weights
        .iter()
        .find_map(|entry| (&entry.peer_id == validator).then_some(entry.weight.clone()))
}

pub(super) fn rechain_error_label(err: &RechainError) -> &'static str {
    match err {
        RechainError::EmptyEvidence => "empty_evidence",
        RechainError::CanonicalEvidenceEncoding => "canonical_evidence_encoding",
        RechainError::DuplicateEvidence => "duplicate_evidence",
        RechainError::SlotMismatch => "slot_mismatch",
        RechainError::ChainOrderHashMismatch => "chain_order_hash_mismatch",
        RechainError::RechainSequenceMismatch => "rechain_sequence_mismatch",
        RechainError::AccuserHasNoCriticalSuccessor => "accuser_has_no_critical_successor",
        RechainError::AccusedIsNotSuccessor { .. } => "accused_is_not_successor",
        RechainError::InsufficientUntaintedValidators { .. } => "insufficient_untainted_validators",
        RechainError::InsufficientQuorumAfterQuarantine => "insufficient_quorum_after_quarantine",
        RechainError::RechainSequenceExhausted => "rechain_sequence_exhausted",
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    fn peer() -> PeerId {
        PeerId::new(KeyPair::random().public_key().clone())
    }

    fn peers(count: usize) -> Vec<PeerId> {
        (0..count).map(|_| peer()).collect()
    }

    fn bls_keypairs(count: usize) -> Vec<KeyPair> {
        (0..count)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect()
    }

    fn peers_from_keypairs(keypairs: &[KeyPair]) -> Vec<PeerId> {
        keypairs
            .iter()
            .map(|key_pair| PeerId::new(key_pair.public_key().clone()))
            .collect()
    }

    fn chain_id() -> ChainId {
        ChainId::from("iroha:test:sumeragi-vnext")
    }

    fn aggregate_for_signers(
        preimage: &[u8],
        signer_roster: &[PeerId],
        signer_indices: &[usize],
        keypairs: &[KeyPair],
    ) -> Vec<u8> {
        let signatures = signer_indices
            .iter()
            .map(|index| {
                let signer = &signer_roster[*index];
                let key_pair = keypairs
                    .iter()
                    .find(|candidate| candidate.public_key() == signer.public_key())
                    .expect("signer key pair exists");
                Signature::try_new(key_pair.private_key(), preimage)
                    .expect("checked aggregate fixture signature")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs).expect("aggregate signature")
    }

    fn pops_for_roster(signer_roster: &[PeerId], keypairs: &[KeyPair]) -> Vec<Vec<u8>> {
        signer_roster
            .iter()
            .map(|signer| {
                let key_pair = keypairs
                    .iter()
                    .find(|candidate| candidate.public_key() == signer.public_key())
                    .expect("signer key pair exists");
                iroha_crypto::bls_normal_pop_prove(key_pair.private_key()).expect("pop proves")
            })
            .collect()
    }

    fn block_hash(seed: u8) -> HashOf<BlockHeader> {
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH]))
    }

    fn slot(seed: u8) -> SlotId {
        SlotId {
            height: 7,
            view: 2,
            epoch: 0,
            block_hash: block_hash(seed),
        }
    }

    fn unsigned_suspect(
        slot: SlotId,
        accuser: PeerId,
        accused: PeerId,
        obligation: MissedObligation,
        chain_order: &ChainOrder,
        observed_delay_ms: u64,
    ) -> Suspect {
        Suspect {
            slot,
            accuser,
            accused,
            obligation,
            chain_order_hash: chain_order.hash(),
            rechain_seq: chain_order.rechain_seq,
            observed_delay_ms,
            signature: Vec::new(),
        }
    }

    #[test]
    fn running_validation_never_inlines_after_timeout() {
        let cfg = PerformanceFaultConfig::default();
        let state = ValidationState::Running {
            id: 42,
            generation: 3,
            started_at_ms: 1_000,
        };

        assert_eq!(
            state.decision_at(1_000 + cfg.suspicion_timeout_ms + 1, &cfg),
            ValidationDecision::RaiseSuspicion
        );
    }

    #[test]
    fn performance_fault_config_converts_from_sumeragi_vnext_config() {
        let actual = iroha_config::parameters::actual::SumeragiVNext {
            performance_window_samples: 9,
            suspicion_timeout: std::time::Duration::from_millis(123),
            performance_threshold_bps: 1_234,
            max_tainted_per_view: 3,
            rechain_cooldown: std::time::Duration::from_millis(45),
        };

        assert_eq!(
            PerformanceFaultConfig::from(&actual),
            PerformanceFaultConfig {
                performance_window_samples: 9,
                suspicion_timeout_ms: 123,
                performance_threshold_bps: 1_234,
                max_tainted_per_view: 3,
                rechain_cooldown_ms: 45,
            }
        );
    }

    #[test]
    fn late_worker_result_is_ignored_after_generation_change() {
        let mut state = ValidationState::Running {
            id: 42,
            generation: 3,
            started_at_ms: 1_000,
        };
        let result = ValidationWorkerResult {
            id: 42,
            generation: 4,
            outcome: ValidationWorkerOutcome::Valid(ValidationRoots {
                parent_state_root: Hash::new(b"parent"),
                post_state_root: Hash::new(b"post"),
            }),
        };

        assert_eq!(
            state.apply_worker_result(result),
            WorkerResultAction::IgnoredStale
        );
        assert!(matches!(state, ValidationState::Running { .. }));
    }

    #[test]
    fn successor_scoped_suspicion_rechains_accuser_and_accused_to_tail() {
        let validators = peers(5);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        let cert = order
            .rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 })
            .expect("successor suspicion should rechain");

        assert_eq!(cert.rechain_seq, 1);
        assert_eq!(
            cert.tainted
                .iter()
                .map(|entry| (&entry.peer_id, entry.reason))
                .collect::<Vec<_>>(),
            vec![
                (&validators[1], TaintReason::Accuser),
                (&validators[2], TaintReason::Accused),
            ]
        );
        assert_ne!(cert.previous_chain_order_hash, cert.new_chain_order_hash);
        assert!(!cert.new_order.critical_path().contains(&validators[1]));
        assert!(!cert.new_order.critical_path().contains(&validators[2]));
        assert_eq!(
            &cert.new_order.ordered_validators[3..],
            &[validators[1].clone(), validators[2].clone()]
        );
    }

    #[test]
    fn non_successor_suspicion_is_rejected() {
        let validators = peers(5);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[0].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        assert_eq!(
            order.rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 }),
            Err(RechainError::AccusedIsNotSuccessor {
                expected: validators[1].clone(),
            })
        );
    }

    #[test]
    fn quarantine_rejects_when_count_quorum_would_drop() {
        let validators = peers(4);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        assert_eq!(
            order.rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 }),
            Err(RechainError::InsufficientUntaintedValidators {
                untainted: 2,
                critical_prefix_len: 3,
            })
        );
    }

    #[test]
    fn stake_quorum_is_checked_after_quarantine() {
        let validators = peers(5);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let weights = validators
            .iter()
            .cloned()
            .map(|peer_id| StakeWeight {
                peer_id,
                weight: Numeric::from(1_u64),
            })
            .collect();

        assert_eq!(
            order.rechain_after_suspicions(
                vec![suspect],
                &QuorumPolicy::Stake {
                    total: Numeric::from(5_u64),
                    weights,
                },
            ),
            Err(RechainError::InsufficientQuorumAfterQuarantine)
        );
    }

    #[test]
    fn stake_quorum_uses_numeric_two_thirds_without_rounding() {
        let validators = peers(3);
        let policy = QuorumPolicy::Stake {
            total: Numeric::new(100, 2),
            weights: vec![
                StakeWeight {
                    peer_id: validators[0].clone(),
                    weight: Numeric::new(50, 2),
                },
                StakeWeight {
                    peer_id: validators[1].clone(),
                    weight: Numeric::new(25, 2),
                },
                StakeWeight {
                    peer_id: validators[2].clone(),
                    weight: Numeric::new(25, 2),
                },
            ],
        };

        assert!(!policy.satisfied_by(&validators[..1]));
        assert!(policy.satisfied_by(&validators[..2]));
        assert_eq!(policy.smallest_satisfying_prefix_len(&validators), Some(2));

        let exact_boundary = QuorumPolicy::Stake {
            total: Numeric::from(3_u64),
            weights: validators
                .iter()
                .cloned()
                .map(|peer_id| StakeWeight {
                    peer_id,
                    weight: Numeric::from(1_u64),
                })
                .collect(),
        };
        assert!(
            !exact_boundary.satisfied_by(&validators[..2]),
            "exact 2/3 stake must fail closed"
        );
        assert!(exact_boundary.satisfied_by(&validators));
    }

    #[test]
    fn multi_suspicion_rechain_rejects_no_longer_successor_after_prior_application() {
        let keypairs = bls_keypairs(9);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 5, 5).expect("valid order");
        let quorum = QuorumPolicy::Count { required: 3 };
        let first = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let second = unsigned_suspect(
            slot(9),
            validators[2].clone(),
            validators[3].clone(),
            MissedObligation::RbcReady,
            &order,
            950,
        );

        assert!(matches!(
            order.rechain_after_suspicions(vec![first, second], &quorum),
            Err(RechainError::AccusedIsNotSuccessor { .. }
                | RechainError::AccuserHasNoCriticalSuccessor)
        ));
    }

    #[test]
    fn rechain_vote_checked_signature_verifies_and_commits_domain() {
        let keypairs = bls_keypairs(5);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let certificate = order
            .rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 })
            .expect("rechain certificate");
        let chain = chain_id();
        let mut vote = RechainVote::unsigned(&certificate, validators[0].clone());

        vote.sign(
            &chain,
            crate::sumeragi::consensus::PERMISSIONED_TAG,
            keypairs[0].private_key(),
        )
        .expect("checked rechain vote signature");

        let preimage = vote
            .signing_preimage(&chain, crate::sumeragi::consensus::PERMISSIONED_TAG)
            .expect("rechain vote preimage");
        let signature = Signature::from_bytes(&vote.signature);
        signature
            .verify(keypairs[0].public_key(), &preimage)
            .expect("checked rechain vote signature verifies");
        let wrong_domain_preimage = vote
            .signing_preimage(&chain, "wrong-mode")
            .expect("wrong-domain rechain vote preimage");
        assert!(
            signature
                .verify(keypairs[0].public_key(), &wrong_domain_preimage)
                .is_err(),
            "rechain vote signature must commit the consensus domain"
        );
    }

    #[test]
    fn view_change_vote_checked_signature_verifies_and_commits_domain() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let signer = PeerId::new(keypair.public_key().clone());
        let certificate = ViewChangeCertificate {
            new_view: 3,
            highest_slot: Some(slot(9)),
            chain_order_hash: Hash::new(b"chain-order"),
            signer_bitmap: Vec::new(),
            aggregate_signature: Vec::new(),
        };
        let chain = chain_id();
        let mut vote = ViewChangeVote::unsigned(&certificate, signer);

        vote.sign(
            &chain,
            crate::sumeragi::consensus::PERMISSIONED_TAG,
            keypair.private_key(),
        )
        .expect("checked view-change vote signature");

        let preimage = vote
            .signing_preimage(&chain, crate::sumeragi::consensus::PERMISSIONED_TAG)
            .expect("view-change vote preimage");
        let signature = Signature::from_bytes(&vote.signature);
        signature
            .verify(keypair.public_key(), &preimage)
            .expect("checked view-change vote signature verifies");
        let wrong_domain_preimage = vote
            .signing_preimage(&chain, "wrong-mode")
            .expect("wrong-domain view-change vote preimage");
        assert!(
            signature
                .verify(keypair.public_key(), &wrong_domain_preimage)
                .is_err(),
            "view-change vote signature must commit the consensus domain"
        );
    }

    #[test]
    fn rechain_certificate_aggregate_verifies_bitmap_quorum_and_body() {
        let keypairs = bls_keypairs(5);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let mut cert = order
            .rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 })
            .expect("rechain certificate");
        let signer_roster = cert.new_order.critical_path().to_vec();
        let signer_indices = [0, 1, 2];
        cert.signer_bitmap =
            build_signer_bitmap(&signer_indices, signer_roster.len()).expect("signer bitmap");
        let chain = chain_id();
        let preimage = cert
            .signing_preimage(&chain, crate::sumeragi::consensus::PERMISSIONED_TAG)
            .expect("preimage");
        cert.aggregate_signature =
            aggregate_for_signers(&preimage, &signer_roster, &signer_indices, &keypairs);
        let pops = pops_for_roster(&signer_roster, &keypairs);
        let pop_refs = pops.iter().map(Vec::as_slice).collect::<Vec<_>>();

        let signers = cert
            .verify_aggregate_signature(
                &chain,
                crate::sumeragi::consensus::PERMISSIONED_TAG,
                &signer_roster,
                &pop_refs,
                &QuorumPolicy::Count { required: 3 },
            )
            .expect("aggregate verifies");

        assert_eq!(signers, signer_roster);
    }

    #[test]
    fn rechain_certificate_rejects_out_of_range_signer_bitmap() {
        let keypairs = bls_keypairs(5);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = unsigned_suspect(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let mut cert = order
            .rechain_after_suspicions(vec![suspect], &QuorumPolicy::Count { required: 3 })
            .expect("rechain certificate");
        let signer_roster = cert.new_order.critical_path().to_vec();
        cert.signer_bitmap = vec![0b0000_1111];
        cert.aggregate_signature = vec![1; 96];
        let pops = pops_for_roster(&signer_roster, &keypairs);
        let pop_refs = pops.iter().map(Vec::as_slice).collect::<Vec<_>>();

        assert_eq!(
            cert.verify_aggregate_signature(
                &chain_id(),
                crate::sumeragi::consensus::PERMISSIONED_TAG,
                &signer_roster,
                &pop_refs,
                &QuorumPolicy::Count { required: 3 },
            ),
            Err(VNextSignatureError::SignerBitmapOutOfRange {
                index: 3,
                roster_len: 3,
            })
        );
    }

    #[test]
    fn preaggregated_vnext_signature_rejects_non_bls_signer_after_checked_algorithm() {
        let keypair = KeyPair::from_seed(b"vnext-non-bls-signer".to_vec(), Algorithm::Ed25519);
        let signer_roster = vec![PeerId::new(keypair.public_key().clone())];
        let pops = [Vec::new()];
        let pop_refs = pops.iter().map(Vec::as_slice).collect::<Vec<_>>();

        let err = verify_preaggregated_vnext_signature(
            b"vnext-preimage".to_vec(),
            &[0b0000_0001],
            &[0xA5; 96],
            &signer_roster,
            &pop_refs,
            &QuorumPolicy::Count { required: 1 },
        )
        .expect_err("non-BLS signer must be rejected before aggregate verification");

        assert_eq!(
            err,
            VNextSignatureError::UnsupportedAggregateKeyAlgorithm {
                algorithm: Algorithm::Ed25519
            }
        );
    }

    #[test]
    fn view_change_certificate_aggregate_verifies_quorum() {
        let keypairs = bls_keypairs(4);
        let signer_roster = peers_from_keypairs(&keypairs);
        let signer_indices = [0, 1, 2];
        let chain = chain_id();
        let mut certificate = ViewChangeCertificate {
            new_view: 3,
            highest_slot: Some(slot(9)),
            chain_order_hash: Hash::new(b"chain-order"),
            signer_bitmap: build_signer_bitmap(&signer_indices, signer_roster.len())
                .expect("signer bitmap"),
            aggregate_signature: Vec::new(),
        };
        let preimage = certificate
            .signing_preimage(&chain, crate::sumeragi::consensus::PERMISSIONED_TAG)
            .expect("preimage");
        certificate.aggregate_signature =
            aggregate_for_signers(&preimage, &signer_roster, &signer_indices, &keypairs);
        let pops = pops_for_roster(&signer_roster, &keypairs);
        let pop_refs = pops.iter().map(Vec::as_slice).collect::<Vec<_>>();

        assert_eq!(
            certificate
                .verify_aggregate_signature(
                    &chain,
                    crate::sumeragi::consensus::PERMISSIONED_TAG,
                    &signer_roster,
                    &pop_refs,
                    &QuorumPolicy::Count { required: 3 },
                )
                .expect("aggregate verifies"),
            signer_roster[..3].to_vec()
        );
    }
}
