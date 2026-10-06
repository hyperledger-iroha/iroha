//! Shared wallet control around the stock-phone `Advance` custody authority.
//!
//! Frozen G1 inputs are durable before Advance. Only its retained completion authorizes a
//! completed operation; an archive, checkpoint or proof never does. Reconciliation follows
//! the provider-selected capsule chain and indexes permanent credit identities. Folding
//! consumes that chain in order, persists every sub-proof, self-verifies Ω including both
//! curve decides through [`NativeProofs`], and records exactly one Ω per head.
//!
//! The native transition/recursive relation implementation is an injected, mandatory
//! dependency. This module does not substitute a structural G1 check for a proof verifier.
//! SDKs should own I/O and call this shared state machine rather than reproduce its rules.
//!
//! Archive storage must share the provider's non-backup custody root and exclusive lifetime.
//! [`ProviderArchive`] and [`AdvanceHandle`] share one exclusive provider. Every archive call
//! uses its protected-data bracket; fixed manifests bind index roots in source-selected
//! metadata generations and the existing iOS rollback anchor. [`FsArchive`] is a low-level
//! standalone store for tests/diagnostics and does not itself provide those brackets.
//! No archive record grants monetary authority independently of Advance.
//!
//! The current slice conservatively retains all frozen inputs and completions. Collection
//! after a covering fold and fee-claim acknowledgement is deliberately not enabled here.
// TODO(G3/G4): connect the real native operation/Λ/Ω artifact provider and mobile bridge;
// qualify this complete custody/proof lifetime on stock phones. Add multi-step run relations
// and acknowledged collection without changing Advance.

use std::collections::BTreeMap;

use iroha_data_model::kagemusha::*;

use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletAdvanceOutcomeV1 as AdvanceOutcome,
    KagemushaWalletAdvanceRequestV1 as AdvanceRequest,
    KagemushaWalletExpectedHeadV1 as ExpectedHead, KagemushaWalletLookupV1 as Lookup,
    KagemushaWalletNotPerformedV1 as NotPerformed, KagemushaWalletProviderErrorV1 as ProviderError,
    KagemushaWalletRetainedV1 as Retained, KagemushaWalletSlotStatusV1 as SlotStatus,
};

mod archive;
mod credit_tree;
mod custody;
mod folding;
mod index;
mod manifest;
mod scheduling;

pub use archive::{ArchiveKey, ArchiveStore, FsArchive};
pub use custody::{AdvanceHandle, Custody, ProviderArchive, TransitionOwner};
pub use folding::{FoldStatus, LineageCache};
pub use index::{IndexRoot, ObjectStore};
pub use scheduling::{Cancellation, PaymentGuard, Scheduler};

/// Errors distinguish uncertain custody from invalid input and missing retained witnesses.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The source custody authority could not complete the request.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// A G1 object failed its canonical or native validation.
    #[error("invalid wallet object: {0}")]
    Invalid(&'static str),
    /// An I/O failure gives no evidence of absence or failure to commit.
    #[error("wallet archive unavailable: {0}")]
    Storage(#[from] std::io::Error),
    /// Required source-bound input is absent or corrupt; never wait silently for it.
    #[error("wallet fold witness custody lost: {0}")]
    WitnessLost(&'static str),
    /// A previously used credit identity names different canonical Payment bytes.
    #[error("conflicting Payment for an already consumed credit")]
    CreditConflict,
    /// The operation needs Ω of the current head.
    #[error("current head is not folded")]
    FoldRequired,
    /// The selected operation has not durably completed yet.
    #[error("selected operation is pending")]
    Pending,
    /// No active or released head is available.
    #[error("wallet has no usable head")]
    NoHead,
    /// Payment work cancelled the current sub-proof.
    #[error("fold cancelled at a cooperative boundary")]
    Cancelled,
    /// A required native proof failed or its implementation is unavailable.
    #[error("native proof rejected: {0}")]
    Proof(&'static str),
}

fn valid<T>(result: Result<T, KagemushaWalletValidationErrorV1>) -> Result<T, Error> {
    result.map_err(|_| Error::Invalid("G1 validation"))
}

/// Immutable pre-signing inputs, including the credential needed to resume assembly.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::FrozenTransition")]
pub struct FrozenTransition {
    /// Credential of this successor head; issuer authentication belongs to `NativeProofs`.
    pub credential: KagemushaWalletCredentialV1,
    /// Marker-bound actual state, σ, map openings and retained canonical input bytes.
    pub capsule: KagemushaWalletRecoveryCapsuleV1,
}

impl FrozenTransition {
    /// Check canonical bindings; this is additional to the mandatory native verifier.
    ///
    /// # Errors
    /// Rejects malformed objects, another wallet/credential, or a mismatching statement.
    pub fn validate(&self) -> Result<(), Error> {
        valid(self.capsule.to_canonical_bytes())?;
        valid(self.credential.validate())?;
        valid(
            self.capsule
                .statement
                .validate_for_credential(&self.credential),
        )?;
        if self.credential.body.wallet_id != self.capsule.wallet_id {
            return Err(Error::Invalid("frozen wallet"));
        }
        Ok(())
    }

    fn expected(&self) -> Result<ExpectedHead, Error> {
        let c = &self.capsule;
        if let KagemushaWalletEffectV1::Bootstrap {
            enrollment_marker, ..
        } = c.statement.effect
        {
            Ok(ExpectedHead::Enrollment {
                marker_digest: enrollment_marker,
            })
        } else {
            Ok(ExpectedHead::Released {
                sequence: c
                    .statement
                    .sequence
                    .checked_sub(1)
                    .ok_or(Error::Invalid("sequence"))?,
                head: c.statement.predecessor,
                capsule_digest: c.predecessor_capsule_digest,
            })
        }
    }

    fn request(&self) -> Result<AdvanceRequest<KagemushaWalletRecoveryCapsuleV1>, Error> {
        Ok(AdvanceRequest {
            expected: self.expected()?,
            operation_id: self.capsule.operation_id,
            new_head: self.capsule.statement.successor,
            proof_digest: valid(self.capsule.proof_digest())?,
            capsule: self.capsule.clone(),
            // The archive has already made its growth durable. G2 reserves its own copies.
            growth_bytes: 0,
        })
    }
}

/// Mandatory native relation boundary. Implementations use the frozen artifact allowlist.
///
/// There is deliberately no default implementation. G1 structural validation is insufficient
/// for any method. A production implementation must verify σ, all consumed objects and map
/// roots before Advance, and both Pasta accumulator decides when verifying Ω.
pub trait NativeProofs {
    /// Ordered intermediate proof layouts from the authenticated artifact schedule for this
    /// operation. The final Ω follows these checkpoints and keeps its separate wire bound.
    ///
    /// # Errors
    /// Reject unavailable artifacts or a schedule not bound to this transition's relation.
    fn fold_schedule(
        &self,
        witness: &ReleasedStep,
        predecessor: Option<&KagemushaWalletFoldRecordV1>,
    ) -> Result<Vec<CheckpointLayout>, Error>;
    /// Verify the transition, inputs and σ against the actual selected predecessor state.
    /// `folded` is the exact Ω already verified and recorded for that predecessor.
    ///
    /// # Errors
    /// Reject invalid/missing proofs, credentials, historical controls, amounts or map roots.
    fn verify_transition(
        &self,
        next: &FrozenTransition,
        predecessor: Option<&FrozenTransition>,
        folded: Option<&KagemushaWalletFoldRecordV1>,
    ) -> Result<(), Error>;

    /// Verify Ω in full, including its deferred values and both curve accumulator decides.
    ///
    /// # Errors
    /// Reject any invalid proof, binding, artifact or decide.
    fn verify_lineage(&self, lineage: &KagemushaWalletLineageV1) -> Result<(), Error>;

    /// Compute exactly the next sub-proof of one released transition. Poll cancellation at
    /// every parallel task boundary and release proof workspaces before returning.
    ///
    /// # Errors
    /// Return `Cancelled` on preemption; reject unavailable/invalid relation inputs.
    fn fold_next(
        &self,
        witness: &ReleasedStep,
        predecessor: Option<&KagemushaWalletFoldRecordV1>,
        checkpoint: Option<&[u8]>,
        cancellation: &Cancellation,
    ) -> Result<FoldProgress, Error>;
}

/// One authenticated released transition; inputs and output remain byte-exact.
#[derive(Debug, Clone)]
pub struct ReleasedStep {
    /// Frozen capsule and credential selected by Advance.
    pub frozen: FrozenTransition,
    /// Original retained completion; never assembled again after release.
    pub retained: Retained<KagemushaWalletCompletionRecordV1>,
}

/// Exact intermediate output layout from the authenticated native proof schedule.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::CheckpointLayout")]
pub struct CheckpointLayout {
    /// Digest binding the proof type, curve, stage and exact circuit/descriptor.
    pub artifact_digest: [u8; 32],
    /// Exact checkpoint payload length, derived from that descriptor's proof layout.
    pub payload_bytes: u32,
}

impl CheckpointLayout {
    fn record_limit(self) -> Result<usize, Error> {
        if self.artifact_digest == [0; 32] || self.payload_bytes == 0 {
            return Err(Error::Proof("checkpoint layout"));
        }
        usize::try_from(self.payload_bytes)
            .ok()
            .and_then(|n| n.checked_add(archive::METADATA_BOUND))
            .ok_or(Error::Proof("checkpoint layout size"))
    }
}

/// One sub-proof result, checked against the authenticated native stage's exact layout.
#[derive(Debug, Clone)]
pub enum FoldProgress {
    /// Persist this result before asking for the next sub-proof.
    Checkpoint(Vec<u8>),
    /// Final Ω. A Receive's burn result determines its first credit-digest leaf.
    Complete {
        /// Candidate lineage, verified natively before it becomes folded.
        lineage: KagemushaWalletLineageV1,
        /// Must be false for a non-Receive operation.
        burned: bool,
    },
}

/// What the caller may display after a serialized transition attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {
    /// Original released output bytes. This is the only successful completion result.
    Complete(Vec<u8>),
    /// A selected or uncertain operation needs reconciliation, never a new debit.
    Pending,
    /// The custody authority definitively did not select the operation.
    NotPerformed(NotPerformed),
    /// A retained result was pruned after its permanent tombstone.
    Archived,
    /// Released bytes were lost; the debit remains committed.
    DeliveryDataLoss,
}

/// Permanent native replay entry. Payment digest is outside the monetary σ state leaf.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::ConsumedCredit")]
pub struct ConsumedCredit {
    /// First canonical Payment digest, never replaced on duplicate delivery.
    pub payment_digest: [u8; 32],
    /// Amount of the first credit.
    pub amount: u128,
    /// Sequence of the crediting Receive.
    pub sequence: u128,
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::Checkpoint")]
struct Checkpoint {
    layout: CheckpointLayout,
    capsule_digest: [u8; 32],
    predecessor_fold: [u8; 32],
    previous: [u8; 32],
    ordinal: u32,
    proof: Vec<u8>,
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::RecordedFold")]
struct RecordedFold {
    record: KagemushaWalletFoldRecordV1,
    burned: bool,
}

/// Shared Rust coordinator. The owned custody handle serializes all monetary transitions.
///
/// Permanent replay/step/fold indexes and the credit-digest tree have source-selected roots.
/// Normal operations traverse bounded paths; interrupted indexing streams one tail capsule
/// at a time. Witness collection is not enabled, so unresolved Payments and fee-claim inputs
/// remain retained until a complete acknowledgement/covering-fold collection policy exists.
pub struct Coordinator<C, A, N> {
    custody: C,
    archive: A,
    proofs: N,
    scheme_id: [u8; 32],
    wallet_id: [u8; 32],
    scheduler: Scheduler,
    verified_folds: BTreeMap<u128, Vec<u8>>,
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    /// Bind a coordinator to one incarnation and a matching archive.
    ///
    /// # Errors
    /// Reject a mismatched archive or a provider marker for another wallet.
    pub fn new(
        custody: C,
        archive: A,
        proofs: N,
        scheme_id: [u8; 32],
        wallet_id: [u8; 32],
    ) -> Result<Self, Error> {
        if archive.binding() != (scheme_id, wallet_id) {
            return Err(Error::Invalid("archive incarnation"));
        }
        let mut this = Self {
            custody,
            archive,
            proofs,
            scheme_id,
            wallet_id,
            scheduler: Scheduler::new(),
            verified_folds: BTreeMap::new(),
        };
        // Verify durable Ω once during open, before the payment critical path.
        if !matches!(this.status()?, SlotStatus::Pending(_)) {
            let (_, manifest) = this.sync_manifest()?;
            if let Some(sequence) = manifest.folded {
                let step = this.indexed_step(&manifest, sequence)?;
                this.read_fold(&step)?;
            }
        }
        Ok(this)
    }

    /// Share the scheduler with the UI/transport so payment arrival can preempt proving.
    #[must_use]
    pub fn scheduler(&self) -> Scheduler {
        self.scheduler.clone()
    }

    fn status(&mut self) -> Result<SlotStatus, Error> {
        let status = self.custody.status()?;
        if let Some(record) = status.marker() {
            let marker = record.marker();
            if marker.scheme_id != self.scheme_id || marker.wallet_id != self.wallet_id {
                return Err(Error::Invalid("provider incarnation"));
            }
        }
        Ok(status)
    }

    fn frozen(&mut self, digest: [u8; 32]) -> Result<FrozenTransition, Error> {
        let bytes = self
            .archive
            .get(
                ArchiveKey::Capsule(digest),
                KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1
                    + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                    + archive::METADATA_BOUND,
            )?
            .ok_or(Error::WitnessLost("selected capsule"))?;
        let frozen: FrozenTransition = archive::decode(&bytes)?;
        frozen.validate()?;
        if frozen.capsule.scheme_id != self.scheme_id
            || frozen.capsule.wallet_id != self.wallet_id
            || valid(frozen.capsule.capsule_digest())? != digest
        {
            return Err(Error::WitnessLost("capsule binding"));
        }
        Ok(frozen)
    }

    /// Explicit diagnostic export of the released chain, never inferred from filenames.
    /// This opt-in export allocates the full history; no normal coordinator operation uses it.
    ///
    /// # Errors
    /// Pending custody is `Pending`; missing/corrupt retained witnesses are custody loss.
    pub fn released_steps(&mut self) -> Result<Vec<ReleasedStep>, Error> {
        let status = self.status()?;
        let marker = match status {
            SlotStatus::Released(record) => record,
            SlotStatus::Enrollment(_) => return Ok(Vec::new()),
            SlotStatus::Pending(_) => return Err(Error::Pending),
            _ => return Err(Error::NoHead),
        };
        let (_, _, mut digest) = marker.head().ok_or(Error::NoHead)?;
        let mut steps = Vec::new();
        let mut expected_head = match marker.marker().state {
            KagemushaWalletMarkerStateV1::Head { head, .. } => head,
            _ => return Err(Error::NoHead),
        };
        let mut expected_sequence = marker.head().ok_or(Error::NoHead)?.0;
        loop {
            let frozen = self.frozen(digest)?;
            let c = &frozen.capsule;
            if c.statement.sequence != expected_sequence || c.statement.successor != expected_head {
                return Err(Error::WitnessLost("capsule chain"));
            }
            let Lookup::Retained(retained) = self.custody.lookup(&c.operation_id)? else {
                return Err(Error::WitnessLost("released completion"));
            };
            if retained.capsule_digest != digest
                || retained.frame != valid(retained.record.to_canonical_bytes())?
                || retained.completion_digest != valid(retained.record.completion_digest())?
            {
                return Err(Error::WitnessLost("completion binding"));
            }
            valid(retained.record.verify(&frozen.credential, c))?;
            digest = c.predecessor_capsule_digest;
            expected_head = c.statement.predecessor;
            steps.push(ReleasedStep {
                frozen,
                retained: *retained,
            });
            if expected_sequence == 0 {
                break;
            }
            expected_sequence -= 1;
        }
        if digest != [0; 32] || !expected_head.is_zero() {
            return Err(Error::WitnessLost("bootstrap chain"));
        }
        // A protected-storage error never turns the chain into an absence result.
        self.status()?;
        steps.reverse();
        Ok(steps)
    }

    /// Find a permanent Receive, including after unrelated operations or a restart.
    ///
    /// # Errors
    /// Reject conflicting Payment bytes and propagate custody/reconciliation failures.
    pub fn consumed_credit(
        &mut self,
        credit_id: &[u8; 32],
        payment_digest: &[u8; 32],
    ) -> Result<Option<ConsumedCredit>, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let credit = self.indexed_credit(&manifest, credit_id)?;
        if credit.is_some_and(|entry| entry.payment_digest != *payment_digest) {
            return Err(Error::CreditConflict);
        }
        Ok(credit)
    }

    /// Return an operation's exact source-bound output; never construct another Payment.
    ///
    /// # Errors
    /// Provider unavailability is retriable and does not imply an unknown operation.
    pub fn retry(&mut self, operation_id: &[u8; 32]) -> Result<Option<Completion>, Error> {
        // Lookup owns reconciliation and both storage probes, including delivery-loss
        // classification. A separate status call could hide that authoritative classification.
        Ok(match self.custody.lookup(operation_id)? {
            Lookup::Retained(retained) => Some(Completion::Complete(retained.record.output)),
            Lookup::SelectedUnsigned { .. } => Some(Completion::Pending),
            Lookup::Archived(_) => Some(Completion::Archived),
            Lookup::DeliveryDataLoss => Some(Completion::DeliveryDataLoss),
            Lookup::Unknown => None,
        })
    }

    /// Validate and durably freeze a transition before dispatching it to Advance.
    ///
    /// # Errors
    /// Invalid proofs/inputs, conflicting replay, missing fold or unavailable custody reject
    /// before mutation. Once Advance selects a head its own Pending/Released rule applies.
    pub fn commit(&mut self, frozen: FrozenTransition) -> Result<Completion, Error> {
        let _payment = self.scheduler.payment();
        frozen.validate()?;
        let c = &frozen.capsule;
        if c.scheme_id != self.scheme_id || c.wallet_id != self.wallet_id {
            return Err(Error::Invalid("transition incarnation"));
        }
        let request = frozen.request()?;
        let owner = TransitionOwner::new(frozen.credential.clone());
        // Let Advance enforce changed-input conflicts for an already selected operation.
        if self.custody.lookup(&c.operation_id)? != Lookup::Unknown {
            return self
                .custody
                .advance(&owner, &request)
                .map(map_outcome)
                .map_err(Error::from);
        }
        let (_, manifest) = self.sync_manifest()?;
        if let KagemushaWalletEffectV1::Receive { credit_id, .. } = c.statement.effect {
            if let Some(credit) = self.indexed_credit(&manifest, &credit_id)? {
                if credit.payment_digest != c.payment_digest {
                    return Err(Error::CreditConflict);
                }
                return Ok(Completion::Complete(
                    self.indexed_step(&manifest, credit.sequence)?
                        .retained
                        .record
                        .output,
                ));
            }
        }
        let predecessor = manifest
            .indexed
            .map(|sequence| self.indexed_step(&manifest, sequence))
            .transpose()?;
        let folded = match predecessor.as_ref() {
            Some(step) => self.read_fold(step)?.map(|fold| fold.record),
            None => None,
        };
        if send_class(c.kind) {
            let fold = folded.as_ref().ok_or(Error::FoldRequired)?;
            if c.predecessor_lineage() != Some(&fold.lineage) {
                return Err(Error::Invalid("recorded predecessor Ω"));
            }
        }
        self.proofs.verify_transition(
            &frozen,
            predecessor.as_ref().map(|s| &s.frozen),
            folded.as_ref(),
        )?;
        let digest = valid(c.capsule_digest())?;
        self.archive
            .put(ArchiveKey::Capsule(digest), &archive::encode(&frozen)?)?;
        self.status()?;
        let outcome = map_outcome(self.custody.advance(&owner, &request)?);
        if matches!(outcome, Completion::Complete(_)) {
            self.sync_manifest()?;
        }
        Ok(outcome)
    }

    /// Resume the source-selected operation using its frozen capsule; no new proof or inputs.
    ///
    /// # Errors
    /// Missing frozen inputs are custody loss. Provider failures retain pending semantics.
    pub fn resume(&mut self) -> Result<Option<Completion>, Error> {
        let _payment = self.scheduler.payment();
        let status = self.status()?;
        let SlotStatus::Pending(marker) = status else {
            return Ok(None);
        };
        let (_, _, digest) = marker.head().ok_or(Error::NoHead)?;
        let frozen = self.frozen(digest)?;
        self.status()?;
        let owner = TransitionOwner::new(frozen.credential.clone());
        let outcome = map_outcome(self.custody.advance(&owner, &frozen.request()?)?);
        if matches!(outcome, Completion::Complete(_)) {
            self.sync_manifest()?;
        }
        Ok(Some(outcome))
    }
}

fn send_class(kind: KagemushaWalletOperationKindV1) -> bool {
    matches!(
        kind,
        KagemushaWalletOperationKindV1::Send
            | KagemushaWalletOperationKindV1::Unload
            | KagemushaWalletOperationKindV1::Retiring
    )
}

fn map_outcome(outcome: AdvanceOutcome<KagemushaWalletCompletionRecordV1>) -> Completion {
    match outcome {
        AdvanceOutcome::Released { retained, .. } => Completion::Complete(retained.record.output),
        AdvanceOutcome::Pending { .. } => Completion::Pending,
        AdvanceOutcome::NotPerformed(reason) => Completion::NotPerformed(reason),
        AdvanceOutcome::Archived(_) => Completion::Archived,
        AdvanceOutcome::DeliveryDataLoss { .. } => Completion::DeliveryDataLoss,
    }
}

#[cfg(test)]
mod tests;
