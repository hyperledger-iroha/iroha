//! Experimental Sumeragi vNext protocol state.
//!
//! This module contains the first breaking-branch building blocks for a
//! replacement consensus core. It deliberately models validation ownership,
//! performance-fault suspicion, and re-chaining as explicit state instead of
//! letting timeout recovery fall through to blocking inline validation.

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
use norito::codec::{Decode, Encode};

// TODO: Wire these vNext types through the live consensus runner once the
// replacement reactor is introduced.

/// Consensus slot identity used by vNext control messages.
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

/// Explicit slot progress state for the vNext reactor.
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
    /// The proposal is waiting for RBC/DA availability.
    AwaitingAvailability {
        /// Proposed block hash.
        block_hash: HashOf<BlockHeader>,
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
    /// Reactor generation captured when the work was dispatched.
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
        /// Reactor generation captured at queue time.
        generation: u64,
        /// Logical queue timestamp in milliseconds.
        queued_at_ms: u64,
    },
    /// Work is running in an asynchronous worker.
    Running {
        /// Worker assignment id.
        id: u64,
        /// Reactor generation captured at dispatch time.
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
    /// Return the reactor decision for this validation state at `now_ms`.
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

/// Reactor decision for validation ownership.
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

    /// Apply a successor-scoped suspicion and return the re-chain certificate.
    ///
    /// # Errors
    ///
    /// Returns [`RechainError`] when the suspicion is not for this chain order,
    /// is not successor-scoped, or quarantine would weaken the required quorum.
    pub fn rechain_after_suspect(
        &self,
        suspect: Suspect,
        quorum: &QuorumPolicy,
    ) -> Result<RechainCertificate, RechainError> {
        self.validate_suspicion(&suspect)?;
        let expected_successor = self
            .successor_of(&suspect.accuser)
            .cloned()
            .ok_or(RechainError::AccuserHasNoCriticalSuccessor)?;
        if expected_successor != suspect.accused {
            return Err(RechainError::AccusedIsNotSuccessor {
                expected: expected_successor,
            });
        }

        let mut untainted = Vec::with_capacity(self.ordered_validators.len());
        let mut tainted_peers = Vec::with_capacity(2);
        for peer in &self.ordered_validators {
            if peer == &suspect.accuser || peer == &suspect.accused {
                tainted_peers.push(peer.clone());
            } else {
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
        reordered.extend(tainted_peers);
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

        Ok(RechainCertificate {
            slot: suspect.slot,
            previous_chain_order_hash: self.hash(),
            new_chain_order_hash: next.hash(),
            new_order: next,
            rechain_seq,
            tainted: vec![
                TaintedValidator {
                    peer_id: suspect.accuser.clone(),
                    reason: TaintReason::Accuser,
                },
                TaintedValidator {
                    peer_id: suspect.accused.clone(),
                    reason: TaintReason::Accused,
                },
            ],
            suspicions: vec![suspect],
            signer_bitmap: Vec::new(),
            aggregate_signature: Vec::new(),
        })
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
        /// Required stake weight.
        required: u64,
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
            Self::Stake { required, weights } => {
                let stake = validators
                    .iter()
                    .map(|validator| stake_weight(weights, validator))
                    .sum::<u64>();
                stake >= *required
            }
        }
    }
}

/// Stake weight assigned to one validator.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct StakeWeight {
    /// Validator peer id.
    pub peer_id: PeerId,
    /// Stake weight used for quorum checks.
    pub weight: u64,
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
    /// Build an unsigned suspicion for tests and local pre-signing assembly.
    #[must_use]
    pub fn unsigned(
        slot: SlotId,
        accuser: PeerId,
        accused: PeerId,
        obligation: MissedObligation,
        chain_order: &ChainOrder,
        observed_delay_ms: u64,
    ) -> Self {
        Self {
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

    /// Return the canonical vNext suspicion signing preimage.
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
        let body = SuspectSigningBody {
            slot: self.slot,
            accuser: self.accuser.clone(),
            accused: self.accused.clone(),
            obligation: self.obligation,
            chain_order_hash: self.chain_order_hash,
            rechain_seq: self.rechain_seq,
            observed_delay_ms: self.observed_delay_ms,
        };
        vnext_signing_preimage(chain_id, mode_tag, "Suspect", &body)
    }

    /// Sign this suspicion with the accuser's private key.
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
        self.signature = Signature::new(private_key, &preimage).payload().to_vec();
        Ok(())
    }

    /// Verify this suspicion against the embedded accuser public key.
    ///
    /// # Errors
    ///
    /// Returns a [`VNextSignatureError`] when the signature is absent, malformed,
    /// or does not match the canonical suspicion preimage.
    pub fn verify_signature(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
    ) -> Result<(), VNextSignatureError> {
        if self.signature.is_empty() {
            return Err(VNextSignatureError::MissingSignature);
        }
        let preimage = self.signing_preimage(chain_id, mode_tag)?;
        Signature::from_bytes(&self.signature)
            .verify(self.accuser.public_key(), &preimage)
            .map_err(|_| VNextSignatureError::BadSignature)
    }
}

/// Re-chain proposal issued by the current head.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RechainProposal {
    /// Slot being re-chained.
    pub slot: SlotId,
    /// Previous chain-order hash.
    pub previous_chain_order_hash: Hash,
    /// Proposed chain order.
    pub proposed_order: ChainOrder,
    /// Suspicion messages justifying the proposal.
    pub suspicions: Vec<Suspect>,
    /// Head signature over the proposal.
    pub head_signature: Vec<u8>,
}

impl RechainProposal {
    /// Return the canonical vNext re-chain proposal signing preimage.
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
        let body = RechainProposalSigningBody {
            slot: self.slot,
            previous_chain_order_hash: self.previous_chain_order_hash,
            proposed_order: self.proposed_order.clone(),
            suspicions: self.suspicions.clone(),
        };
        vnext_signing_preimage(chain_id, mode_tag, "RechainProposal", &body)
    }

    /// Sign this proposal with the current head's private key.
    ///
    /// # Errors
    ///
    /// Returns [`VNextSignatureError::CanonicalEncoding`] if the signing body
    /// cannot be encoded.
    pub fn sign_head(
        &mut self,
        chain_id: &ChainId,
        mode_tag: &str,
        private_key: &PrivateKey,
    ) -> Result<(), VNextSignatureError> {
        let preimage = self.signing_preimage(chain_id, mode_tag)?;
        self.head_signature = Signature::new(private_key, &preimage).payload().to_vec();
        Ok(())
    }

    /// Verify the proposal head signature against `head`.
    ///
    /// # Errors
    ///
    /// Returns a [`VNextSignatureError`] when the signature is absent, malformed,
    /// or does not match the canonical proposal preimage.
    pub fn verify_head_signature(
        &self,
        chain_id: &ChainId,
        mode_tag: &str,
        head: &PeerId,
    ) -> Result<(), VNextSignatureError> {
        if self.head_signature.is_empty() {
            return Err(VNextSignatureError::MissingSignature);
        }
        let preimage = self.signing_preimage(chain_id, mode_tag)?;
        Signature::from_bytes(&self.head_signature)
            .verify(head.public_key(), &preimage)
            .map_err(|_| VNextSignatureError::BadSignature)
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
        let body = RechainCertificateSigningBody {
            slot: self.slot,
            previous_chain_order_hash: self.previous_chain_order_hash,
            new_chain_order_hash: self.new_chain_order_hash,
            new_order: self.new_order.clone(),
            rechain_seq: self.rechain_seq,
            tainted: self.tainted.clone(),
            suspicions: self.suspicions.clone(),
        };
        vnext_signing_preimage(chain_id, mode_tag, "RechainCertificate", &body)
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
        let body = ViewChangeCertificateSigningBody {
            new_view: self.new_view,
            highest_slot: self.highest_slot,
            chain_order_hash: self.chain_order_hash,
        };
        vnext_signing_preimage(chain_id, mode_tag, "ViewChangeCertificate", &body)
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
    /// A single-signer signature field was empty.
    MissingSignature,
    /// A single-signer signature failed verification.
    BadSignature,
    /// An aggregate signature field was empty.
    MissingAggregateSignature,
    /// An aggregate signature failed BLS verification.
    BadAggregateSignature,
    /// The canonical signing body could not be encoded.
    CanonicalEncoding,
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
        let algorithm = signer.public_key().algorithm();
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
struct RechainProposalSigningBody {
    slot: SlotId,
    previous_chain_order_hash: Hash,
    proposed_order: ChainOrder,
    suspicions: Vec<Suspect>,
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

/// vNext consensus control message.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum ConsensusMessage {
    /// Successor-scoped performance suspicion.
    Suspect(Suspect),
    /// Proposal to install a new chain order.
    RechainProposal(RechainProposal),
    /// Certificate confirming a new chain order.
    RechainCertificate(RechainCertificate),
    /// Certificate moving to a new view.
    ViewChangeCertificate(ViewChangeCertificate),
}

/// Event consumed by the vNext reactor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactorEvent {
    /// A proposal was accepted for local tracking.
    ProposalAccepted {
        /// Slot being proposed.
        slot: SlotId,
        /// Data-availability payload hash.
        payload_hash: Hash,
    },
    /// DA/RBC completed for the slot.
    AvailabilityReady {
        /// Slot whose payload became available.
        slot: SlotId,
    },
    /// The slot needs stateful validation.
    ValidationNeeded {
        /// Slot requiring validation.
        slot: SlotId,
        /// Logical timestamp in milliseconds.
        now_ms: u64,
    },
    /// A validation worker accepted and started work.
    ValidationWorkerStarted {
        /// Slot being validated.
        slot: SlotId,
        /// Worker assignment id.
        id: u64,
        /// Reactor generation assigned to the work.
        generation: u64,
        /// Logical start timestamp in milliseconds.
        started_at_ms: u64,
    },
    /// The validation worker queue was full.
    ValidationQueueFull {
        /// Slot whose validation could not be queued.
        slot: SlotId,
        /// Worker assignment id whose queue attempt failed.
        id: u64,
        /// Reactor generation whose queue attempt failed.
        generation: u64,
        /// Logical timestamp in milliseconds.
        now_ms: u64,
    },
    /// A validation worker returned a result.
    ValidationResult {
        /// Slot the result belongs to.
        slot: SlotId,
        /// Worker result.
        result: ValidationWorkerResult,
    },
    /// Timer tick for timeout and recovery decisions.
    Tick {
        /// Logical timestamp in milliseconds.
        now_ms: u64,
    },
    /// Local successor obligation timed out.
    SuccessorObligationMissed {
        /// Slot whose obligation was missed.
        slot: SlotId,
        /// Validator raising suspicion.
        accuser: PeerId,
        /// Accused successor.
        accused: PeerId,
        /// Missed obligation.
        obligation: MissedObligation,
        /// Observed delay in milliseconds.
        observed_delay_ms: u64,
    },
    /// A suspicion was received from the network.
    SuspectReceived {
        /// Received suspicion.
        suspect: Suspect,
        /// Logical timestamp in milliseconds.
        now_ms: u64,
    },
}

/// Effect emitted by the vNext reactor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactorEffect {
    /// Dispatch stateful validation to an asynchronous worker.
    DispatchValidation {
        /// Slot to validate.
        slot: SlotId,
        /// Worker assignment id.
        id: u64,
        /// Reactor generation assigned to the work.
        generation: u64,
    },
    /// Validation finished successfully.
    AcceptValidated {
        /// Validated slot.
        slot: SlotId,
        /// Validated state roots.
        roots: ValidationRoots,
    },
    /// Validation rejected the slot.
    RejectValidation {
        /// Rejected slot.
        slot: SlotId,
        /// Validation failure summary.
        failure: ValidationFailure,
    },
    /// Broadcast a vNext consensus message.
    BroadcastVNext {
        /// Message to broadcast.
        message: ConsensusMessage,
    },
    /// Enter recovery without blocking the reactor.
    StartRecovery {
        /// Slot entering recovery.
        slot: SlotId,
        /// Recovery reason.
        reason: RecoveryReason,
    },
    /// Install a certified chain order.
    InstallRechain {
        /// Re-chain certificate.
        certificate: RechainCertificate,
    },
    /// A view change is required instead of local re-chain.
    RequireViewChange {
        /// Slot requiring view change.
        slot: SlotId,
        /// Stable reason label.
        reason_label: String,
    },
    /// Drop a stale worker event or result.
    DropStaleWorkerResult {
        /// Slot associated with the stale worker message.
        slot: SlotId,
        /// Stale worker id.
        id: u64,
        /// Stale generation.
        generation: u64,
    },
    /// Reject an invalid suspicion.
    RejectSuspicion {
        /// Slot associated with the suspicion.
        slot: SlotId,
        /// Stable reason label.
        reason_label: String,
    },
}

/// Recovery trigger emitted by the reactor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryReason {
    /// Validation exceeded the configured timeout.
    ValidationTimeout,
    /// Validation could not be queued within the configured timeout.
    ValidationBackpressure,
    /// A successor-scoped protocol obligation was missed.
    SuccessorObligation,
}

/// One slot tracked by the vNext reactor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactorSlot {
    /// Slot identifier.
    pub slot: SlotId,
    /// Consensus slot state.
    pub slot_state: SlotState,
    /// Validation ownership state.
    pub validation: ValidationState,
}

impl ReactorSlot {
    fn new(slot: SlotId) -> Self {
        Self {
            slot,
            slot_state: SlotState::Idle,
            validation: ValidationState::Unqueued,
        }
    }
}

/// Nonblocking vNext reactor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reactor {
    /// Current chain order.
    pub chain_order: ChainOrder,
    /// Quorum policy used for re-chain safety checks.
    pub quorum: QuorumPolicy,
    /// Performance-fault configuration.
    pub config: PerformanceFaultConfig,
    /// Slots tracked by block hash.
    pub slots: BTreeMap<HashOf<BlockHeader>, ReactorSlot>,
    next_validation_id: u64,
    generation: u64,
    last_rechain_ms: Option<u64>,
}

impl Reactor {
    /// Construct a new vNext reactor.
    #[must_use]
    pub fn new(
        chain_order: ChainOrder,
        quorum: QuorumPolicy,
        config: PerformanceFaultConfig,
    ) -> Self {
        Self {
            chain_order,
            quorum,
            config,
            slots: BTreeMap::new(),
            next_validation_id: 0,
            generation: 0,
            last_rechain_ms: None,
        }
    }

    /// Process one event and return side effects for the runtime shell.
    pub fn handle_event(&mut self, event: ReactorEvent) -> Vec<ReactorEffect> {
        match event {
            ReactorEvent::ProposalAccepted { slot, payload_hash } => {
                let reactor_slot = self.slot_mut(slot);
                reactor_slot.slot_state = SlotState::Proposed {
                    block_hash: slot.block_hash,
                    payload_hash,
                };
                Vec::new()
            }
            ReactorEvent::AvailabilityReady { slot } => {
                let reactor_slot = self.slot_mut(slot);
                reactor_slot.slot_state = SlotState::AwaitingValidation {
                    block_hash: slot.block_hash,
                };
                Vec::new()
            }
            ReactorEvent::ValidationNeeded { slot, now_ms } => {
                self.handle_validation_needed(slot, now_ms)
            }
            ReactorEvent::ValidationWorkerStarted {
                slot,
                id,
                generation,
                started_at_ms,
            } => {
                let reactor_slot = self.slot_mut(slot);
                if matches!(
                    &reactor_slot.validation,
                    ValidationState::Queued {
                        id: queued_id,
                        generation: queued_generation,
                        ..
                    } if *queued_id == id && *queued_generation == generation
                ) {
                    reactor_slot.validation = reactor_slot.validation.clone().worker_started(
                        id,
                        generation,
                        started_at_ms,
                    );
                    Vec::new()
                } else {
                    vec![ReactorEffect::DropStaleWorkerResult {
                        slot,
                        id,
                        generation,
                    }]
                }
            }
            ReactorEvent::ValidationQueueFull {
                slot,
                id,
                generation,
                now_ms,
            } => {
                let reactor_slot = self.slot_mut(slot);
                if matches!(
                    &reactor_slot.validation,
                    ValidationState::Queued {
                        id: queued_id,
                        generation: queued_generation,
                        ..
                    } if *queued_id == id && *queued_generation == generation
                ) {
                    reactor_slot.validation = ValidationState::Backpressured { since_ms: now_ms };
                    Vec::new()
                } else {
                    vec![ReactorEffect::DropStaleWorkerResult {
                        slot,
                        id,
                        generation,
                    }]
                }
            }
            ReactorEvent::ValidationResult { slot, result } => {
                self.handle_validation_result(slot, result)
            }
            ReactorEvent::Tick { now_ms } => self.handle_tick(now_ms),
            ReactorEvent::SuccessorObligationMissed {
                slot,
                accuser,
                accused,
                obligation,
                observed_delay_ms,
            } => self.handle_successor_obligation_missed(
                slot,
                accuser,
                accused,
                obligation,
                observed_delay_ms,
            ),
            ReactorEvent::SuspectReceived { suspect, now_ms } => {
                self.handle_suspect_received(suspect, now_ms)
            }
        }
    }

    /// Return a tracked slot by block hash.
    #[must_use]
    pub fn slot(&self, block_hash: HashOf<BlockHeader>) -> Option<&ReactorSlot> {
        self.slots.get(&block_hash)
    }

    fn slot_mut(&mut self, slot: SlotId) -> &mut ReactorSlot {
        self.slots
            .entry(slot.block_hash)
            .or_insert_with(|| ReactorSlot::new(slot))
    }

    fn handle_validation_needed(&mut self, slot: SlotId, now_ms: u64) -> Vec<ReactorEffect> {
        let config = self.config;
        let decision = self.slot_mut(slot).validation.decision_at(now_ms, &config);
        if !matches!(decision, ValidationDecision::DispatchWorker) {
            return Vec::new();
        }

        self.next_validation_id = self.next_validation_id.saturating_add(1);
        self.generation = self.generation.saturating_add(1);
        let id = self.next_validation_id;
        let generation = self.generation;
        self.slot_mut(slot).validation = ValidationState::Queued {
            id,
            generation,
            queued_at_ms: now_ms,
        };
        vec![ReactorEffect::DispatchValidation {
            slot,
            id,
            generation,
        }]
    }

    fn handle_validation_result(
        &mut self,
        slot: SlotId,
        result: ValidationWorkerResult,
    ) -> Vec<ReactorEffect> {
        let result_id = result.id;
        let result_generation = result.generation;
        let reactor_slot = self.slot_mut(slot);
        if reactor_slot.validation.apply_worker_result(result) == WorkerResultAction::IgnoredStale {
            return vec![ReactorEffect::DropStaleWorkerResult {
                slot,
                id: result_id,
                generation: result_generation,
            }];
        }

        match &reactor_slot.validation {
            ValidationState::Valid { roots } => {
                reactor_slot.slot_state = SlotState::Prepared {
                    block_hash: slot.block_hash,
                };
                vec![ReactorEffect::AcceptValidated {
                    slot,
                    roots: *roots,
                }]
            }
            ValidationState::Invalid { failure } => {
                reactor_slot.slot_state = SlotState::Aborted {
                    reason_label: failure.reason_label.clone(),
                };
                vec![ReactorEffect::RejectValidation {
                    slot,
                    failure: failure.clone(),
                }]
            }
            _ => Vec::new(),
        }
    }

    fn handle_tick(&mut self, now_ms: u64) -> Vec<ReactorEffect> {
        let config = self.config;
        let mut effects = Vec::new();
        for reactor_slot in self.slots.values_mut() {
            if matches!(
                reactor_slot.slot_state,
                SlotState::Recovering { .. } | SlotState::Aborted { .. }
            ) {
                continue;
            }
            let reason = match reactor_slot.validation.decision_at(now_ms, &config) {
                ValidationDecision::RaiseSuspicion => match reactor_slot.validation {
                    ValidationState::Backpressured { .. } => RecoveryReason::ValidationBackpressure,
                    _ => RecoveryReason::ValidationTimeout,
                },
                _ => continue,
            };
            reactor_slot.slot_state = SlotState::Recovering {
                block_hash: Some(reactor_slot.slot.block_hash),
            };
            effects.push(ReactorEffect::StartRecovery {
                slot: reactor_slot.slot,
                reason,
            });
        }
        effects
    }

    fn handle_successor_obligation_missed(
        &mut self,
        slot: SlotId,
        accuser: PeerId,
        accused: PeerId,
        obligation: MissedObligation,
        observed_delay_ms: u64,
    ) -> Vec<ReactorEffect> {
        let Some(expected_successor) = self.chain_order.successor_of(&accuser).cloned() else {
            return vec![ReactorEffect::RejectSuspicion {
                slot,
                reason_label: "accuser_has_no_critical_successor".to_owned(),
            }];
        };
        if expected_successor != accused {
            return vec![ReactorEffect::RejectSuspicion {
                slot,
                reason_label: "accused_is_not_successor".to_owned(),
            }];
        }

        let suspect = Suspect::unsigned(
            slot,
            accuser,
            accused,
            obligation,
            &self.chain_order,
            observed_delay_ms,
        );
        self.slot_mut(slot).slot_state = SlotState::Recovering {
            block_hash: Some(slot.block_hash),
        };
        vec![
            ReactorEffect::BroadcastVNext {
                message: ConsensusMessage::Suspect(suspect),
            },
            ReactorEffect::StartRecovery {
                slot,
                reason: RecoveryReason::SuccessorObligation,
            },
        ]
    }

    fn handle_suspect_received(&mut self, suspect: Suspect, now_ms: u64) -> Vec<ReactorEffect> {
        if self
            .last_rechain_ms
            .is_some_and(|last| now_ms.saturating_sub(last) < self.config.rechain_cooldown_ms)
        {
            return vec![ReactorEffect::RejectSuspicion {
                slot: suspect.slot,
                reason_label: "rechain_cooldown".to_owned(),
            }];
        }

        let slot = suspect.slot;
        match self
            .chain_order
            .rechain_after_suspect(suspect, &self.quorum)
        {
            Ok(certificate) => {
                if certificate.tainted.len() > usize::from(self.config.max_tainted_per_view) {
                    return vec![ReactorEffect::RequireViewChange {
                        slot,
                        reason_label: "max_tainted_per_view_exceeded".to_owned(),
                    }];
                }
                self.chain_order = certificate.new_order.clone();
                self.last_rechain_ms = Some(now_ms);
                vec![ReactorEffect::InstallRechain { certificate }]
            }
            Err(RechainError::InsufficientUntaintedValidators { .. })
            | Err(RechainError::InsufficientQuorumAfterQuarantine) => {
                vec![ReactorEffect::RequireViewChange {
                    slot,
                    reason_label: "rechain_would_weaken_quorum".to_owned(),
                }]
            }
            Err(err) => vec![ReactorEffect::RejectSuspicion {
                slot,
                reason_label: rechain_error_label(&err).to_owned(),
            }],
        }
    }
}

/// Re-chain validation error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RechainError {
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

fn stake_weight(weights: &[StakeWeight], validator: &PeerId) -> u64 {
    weights
        .iter()
        .find_map(|entry| (&entry.peer_id == validator).then_some(entry.weight))
        .unwrap_or(0)
}

fn rechain_error_label(err: &RechainError) -> &'static str {
    match err {
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
                Signature::new(key_pair.private_key(), preimage)
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

    fn validation_roots() -> ValidationRoots {
        ValidationRoots {
            parent_state_root: Hash::new(b"parent"),
            post_state_root: Hash::new(b"post"),
        }
    }

    fn reactor_with(validators: Vec<PeerId>, critical_prefix_len: u16, required: u16) -> Reactor {
        let order = ChainOrder::new(
            7,
            2,
            0,
            0,
            validators,
            critical_prefix_len,
            critical_prefix_len,
        )
        .expect("valid order");
        Reactor::new(
            order,
            QuorumPolicy::Count { required },
            PerformanceFaultConfig::default(),
        )
    }

    fn dispatch_validation(reactor: &mut Reactor, slot: SlotId, now_ms: u64) -> (u64, u64) {
        let effects = reactor.handle_event(ReactorEvent::ValidationNeeded { slot, now_ms });
        let [
            ReactorEffect::DispatchValidation {
                slot: dispatched_slot,
                id,
                generation,
            },
        ] = effects.as_slice()
        else {
            panic!("expected one dispatch effect, got {effects:?}");
        };
        assert_eq!(*dispatched_slot, slot);
        (*id, *generation)
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
    fn reactor_dispatches_validation_once_and_waits_while_queued() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(10);

        let (id, generation) = dispatch_validation(&mut reactor, slot, 10);
        assert_eq!((id, generation), (1, 1));
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .validation,
            ValidationState::Queued {
                id: 1,
                generation: 1,
                queued_at_ms: 10
            }
        ));

        assert!(
            reactor
                .handle_event(ReactorEvent::ValidationNeeded { slot, now_ms: 11 })
                .is_empty()
        );
    }

    #[test]
    fn reactor_rejects_stale_worker_start_without_hijacking_validation() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(11);
        dispatch_validation(&mut reactor, slot, 10);

        assert_eq!(
            reactor.handle_event(ReactorEvent::ValidationWorkerStarted {
                slot,
                id: 1,
                generation: 2,
                started_at_ms: 12,
            }),
            vec![ReactorEffect::DropStaleWorkerResult {
                slot,
                id: 1,
                generation: 2,
            }]
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .validation,
            ValidationState::Queued {
                id: 1,
                generation: 1,
                queued_at_ms: 10
            }
        ));
    }

    #[test]
    fn reactor_overdue_running_validation_enters_recovery_without_dispatching_inline() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(12);
        let (id, generation) = dispatch_validation(&mut reactor, slot, 10);
        reactor.handle_event(ReactorEvent::ValidationWorkerStarted {
            slot,
            id,
            generation,
            started_at_ms: 20,
        });

        assert_eq!(
            reactor.handle_event(ReactorEvent::Tick {
                now_ms: 20 + reactor.config.suspicion_timeout_ms,
            }),
            vec![ReactorEffect::StartRecovery {
                slot,
                reason: RecoveryReason::ValidationTimeout,
            }]
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .slot_state,
            SlotState::Recovering { .. }
        ));
    }

    #[test]
    fn reactor_queue_backpressure_enters_recovery_without_inline_validation() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(13);
        let (id, generation) = dispatch_validation(&mut reactor, slot, 10);

        assert!(
            reactor
                .handle_event(ReactorEvent::ValidationQueueFull {
                    slot,
                    id,
                    generation,
                    now_ms: 12,
                })
                .is_empty()
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .validation,
            ValidationState::Backpressured { since_ms: 12 }
        ));
        assert_eq!(
            reactor.handle_event(ReactorEvent::Tick {
                now_ms: 12 + reactor.config.suspicion_timeout_ms,
            }),
            vec![ReactorEffect::StartRecovery {
                slot,
                reason: RecoveryReason::ValidationBackpressure,
            }]
        );
    }

    #[test]
    fn reactor_applies_matching_worker_result() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(14);
        let roots = validation_roots();
        let (id, generation) = dispatch_validation(&mut reactor, slot, 10);
        reactor.handle_event(ReactorEvent::ValidationWorkerStarted {
            slot,
            id,
            generation,
            started_at_ms: 12,
        });

        assert_eq!(
            reactor.handle_event(ReactorEvent::ValidationResult {
                slot,
                result: ValidationWorkerResult {
                    id,
                    generation,
                    outcome: ValidationWorkerOutcome::Valid(roots),
                },
            }),
            vec![ReactorEffect::AcceptValidated { slot, roots }]
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .slot_state,
            SlotState::Prepared { .. }
        ));
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
    fn reactor_drops_stale_worker_result() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators, 3, 3);
        let slot = slot(15);
        let (id, generation) = dispatch_validation(&mut reactor, slot, 10);
        reactor.handle_event(ReactorEvent::ValidationWorkerStarted {
            slot,
            id,
            generation,
            started_at_ms: 12,
        });

        assert_eq!(
            reactor.handle_event(ReactorEvent::ValidationResult {
                slot,
                result: ValidationWorkerResult {
                    id,
                    generation: generation + 1,
                    outcome: ValidationWorkerOutcome::Valid(validation_roots()),
                },
            }),
            vec![ReactorEffect::DropStaleWorkerResult {
                slot,
                id,
                generation: generation + 1,
            }]
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .validation,
            ValidationState::Running { .. }
        ));
    }

    #[test]
    fn successor_scoped_suspicion_rechains_accuser_and_accused_to_tail() {
        let validators = peers(5);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        let cert = order
            .rechain_after_suspect(suspect, &QuorumPolicy::Count { required: 3 })
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
    fn reactor_broadcasts_successor_suspicion_and_enters_recovery() {
        let validators = peers(5);
        let mut reactor = reactor_with(validators.clone(), 3, 3);
        let slot = slot(16);
        let suspect = Suspect::unsigned(
            slot,
            validators[0].clone(),
            validators[1].clone(),
            MissedObligation::VoteRelay,
            &reactor.chain_order,
            900,
        );

        assert_eq!(
            reactor.handle_event(ReactorEvent::SuccessorObligationMissed {
                slot,
                accuser: validators[0].clone(),
                accused: validators[1].clone(),
                obligation: MissedObligation::VoteRelay,
                observed_delay_ms: 900,
            }),
            vec![
                ReactorEffect::BroadcastVNext {
                    message: ConsensusMessage::Suspect(suspect),
                },
                ReactorEffect::StartRecovery {
                    slot,
                    reason: RecoveryReason::SuccessorObligation,
                },
            ]
        );
        assert!(matches!(
            reactor
                .slot(slot.block_hash)
                .expect("tracked slot")
                .slot_state,
            SlotState::Recovering { .. }
        ));
    }

    #[test]
    fn reactor_installs_rechain_for_valid_received_suspicion() {
        let validators = peers(5);
        let mut reactor = reactor_with(validators.clone(), 3, 3);
        let slot = slot(17);
        let suspect = Suspect::unsigned(
            slot,
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &reactor.chain_order,
            900,
        );

        let effects = reactor.handle_event(ReactorEvent::SuspectReceived {
            suspect,
            now_ms: 1_000,
        });
        let [ReactorEffect::InstallRechain { certificate }] = effects.as_slice() else {
            panic!("expected install rechain effect, got {effects:?}");
        };

        assert_eq!(certificate.rechain_seq, 1);
        assert_eq!(reactor.chain_order.rechain_seq, 1);
        assert_eq!(reactor.chain_order, certificate.new_order);
        assert!(!reactor.chain_order.critical_path().contains(&validators[1]));
        assert!(!reactor.chain_order.critical_path().contains(&validators[2]));
    }

    #[test]
    fn reactor_requires_view_change_when_rechain_would_weaken_quorum() {
        let validators = peers(4);
        let mut reactor = reactor_with(validators.clone(), 3, 3);
        let slot = slot(18);
        let suspect = Suspect::unsigned(
            slot,
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &reactor.chain_order,
            900,
        );

        assert_eq!(
            reactor.handle_event(ReactorEvent::SuspectReceived {
                suspect,
                now_ms: 1_000,
            }),
            vec![ReactorEffect::RequireViewChange {
                slot,
                reason_label: "rechain_would_weaken_quorum".to_owned(),
            }]
        );
        assert_eq!(reactor.chain_order.rechain_seq, 0);
    }

    #[test]
    fn non_successor_suspicion_is_rejected() {
        let validators = peers(5);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[0].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        assert_eq!(
            order.rechain_after_suspect(suspect, &QuorumPolicy::Count { required: 3 }),
            Err(RechainError::AccusedIsNotSuccessor {
                expected: validators[1].clone(),
            })
        );
    }

    #[test]
    fn quarantine_rejects_when_count_quorum_would_drop() {
        let validators = peers(4);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );

        assert_eq!(
            order.rechain_after_suspect(suspect, &QuorumPolicy::Count { required: 3 }),
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
        let suspect = Suspect::unsigned(
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
            .map(|peer_id| StakeWeight { peer_id, weight: 1 })
            .collect();

        assert_eq!(
            order.rechain_after_suspect(
                suspect,
                &QuorumPolicy::Stake {
                    required: 4,
                    weights,
                },
            ),
            Err(RechainError::InsufficientQuorumAfterQuarantine)
        );
    }

    #[test]
    fn suspect_signature_is_chain_and_mode_bound() {
        let keypairs = bls_keypairs(3);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let mut suspect = Suspect::unsigned(
            slot(9),
            validators[0].clone(),
            validators[1].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let chain = chain_id();
        suspect
            .sign(
                &chain,
                crate::sumeragi::consensus::PERMISSIONED_TAG,
                keypairs[0].private_key(),
            )
            .expect("sign suspicion");

        assert!(
            suspect
                .verify_signature(&chain, crate::sumeragi::consensus::PERMISSIONED_TAG)
                .is_ok()
        );
        assert_eq!(
            suspect.verify_signature(
                &ChainId::from("iroha:test:other-chain"),
                crate::sumeragi::consensus::PERMISSIONED_TAG,
            ),
            Err(VNextSignatureError::BadSignature)
        );
        assert_eq!(
            suspect.verify_signature(&chain, crate::sumeragi::consensus::NPOS_TAG),
            Err(VNextSignatureError::BadSignature)
        );
    }

    #[test]
    fn rechain_proposal_head_signature_verifies() {
        let keypairs = bls_keypairs(5);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let cert = order
            .rechain_after_suspect(suspect.clone(), &QuorumPolicy::Count { required: 2 })
            .expect("rechain certificate");
        let mut proposal = RechainProposal {
            slot: suspect.slot,
            previous_chain_order_hash: cert.previous_chain_order_hash,
            proposed_order: cert.new_order,
            suspicions: vec![suspect],
            head_signature: Vec::new(),
        };
        let chain = chain_id();
        proposal
            .sign_head(
                &chain,
                crate::sumeragi::consensus::PERMISSIONED_TAG,
                keypairs[0].private_key(),
            )
            .expect("sign proposal");

        assert!(
            proposal
                .verify_head_signature(
                    &chain,
                    crate::sumeragi::consensus::PERMISSIONED_TAG,
                    &validators[0],
                )
                .is_ok()
        );
        assert_eq!(
            proposal.verify_head_signature(
                &chain,
                crate::sumeragi::consensus::PERMISSIONED_TAG,
                &validators[1],
            ),
            Err(VNextSignatureError::BadSignature)
        );
    }

    #[test]
    fn rechain_certificate_aggregate_verifies_bitmap_quorum_and_body() {
        let keypairs = bls_keypairs(5);
        let validators = peers_from_keypairs(&keypairs);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let mut cert = order
            .rechain_after_suspect(suspect, &QuorumPolicy::Count { required: 3 })
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
        let suspect = Suspect::unsigned(
            slot(9),
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let mut cert = order
            .rechain_after_suspect(suspect, &QuorumPolicy::Count { required: 3 })
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

    #[test]
    fn reactor_requires_view_change_when_tainted_budget_is_exceeded() {
        let validators = peers(5);
        let mut reactor = reactor_with(validators.clone(), 3, 3);
        reactor.config.max_tainted_per_view = 1;
        let slot = slot(19);
        let suspect = Suspect::unsigned(
            slot,
            validators[1].clone(),
            validators[2].clone(),
            MissedObligation::AckValidation,
            &reactor.chain_order,
            900,
        );

        assert_eq!(
            reactor.handle_event(ReactorEvent::SuspectReceived {
                suspect,
                now_ms: 1_000,
            }),
            vec![ReactorEffect::RequireViewChange {
                slot,
                reason_label: "max_tainted_per_view_exceeded".to_owned(),
            }]
        );
        assert_eq!(reactor.chain_order.rechain_seq, 0);
    }

    #[test]
    fn vnext_consensus_message_roundtrips_through_norito() {
        let validators = peers(3);
        let order = ChainOrder::new(7, 2, 0, 0, validators.clone(), 3, 3).expect("valid order");
        let suspect = Suspect::unsigned(
            slot(9),
            validators[0].clone(),
            validators[1].clone(),
            MissedObligation::AckValidation,
            &order,
            900,
        );
        let message = ConsensusMessage::Suspect(suspect);

        let bytes = norito::to_bytes(&message).expect("encode vNext message");
        let decoded =
            norito::decode_from_bytes::<ConsensusMessage>(&bytes).expect("decode vNext message");

        assert_eq!(decoded, message);
    }
}
