//! Bind actual retained execution to a verified decision without publishing it.
//!
//! This step has no State, Queue, archive or Kura mutation. Its result deliberately
//! lacks the independent source/retirement/durability and installation authority.

use super::PreparedCarrierJournals;
use crate::block::{BlockValidationError, CommittedBlock, ValidBlock, VerifiedV2FinalityArtifact};
use iroha_data_model::{
    block::{
        SignedBlock,
        consensus_v2::{ExecutionCommitment, HeightContext, finality::V2FinalityArtifact},
    },
    events::pipeline::{BlockEvent, BlockStatus, PipelineEventBox},
};

/// Borrowed original inputs for admission of the finality-binding work.
///
/// Admission covers the current canonical-resultless helper's SignedBlock clone,
/// canonical proposal/result-bearing wire encoding overlap and validation temporaries. It is a local reservation, never consensus authority.
pub(crate) struct CarrierDecisionBindingInputs<'owner> {
    /// Exact result-bearing block retained from the sole execution.
    pub(crate) block: &'owner SignedBlock,
    /// Complete context used by that execution, not a newly resolved context.
    pub(crate) context: &'owner HeightContext,
    /// Execution-prefix commitment computed from its actual witness and outputs.
    pub(crate) execution_prefix: ExecutionCommitment,
    /// Already cryptographically verified decision to join to those inputs.
    pub(crate) finality: &'owner V2FinalityArtifact,
}

/// Refusal of decision binding; no variant authorizes publication or reexecution.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CarrierDecisionBindingError<E> {
    /// The verified decision belongs to another semantic frozen context identity.
    #[error("decision differs from the retained frozen height context")]
    Context,
    /// The verified decision commits different execution than the actual owner.
    #[error("decision differs from the retained execution-prefix commitment")]
    Execution,
    /// An internal carrier owner lost its exact proposal header.
    #[error("retained carrier metadata differs from its validated proposal")]
    Header,
    /// Local encoding/capture capacity was unavailable before binding work.
    #[error("decision-binding resource admission failed")]
    Admission(E),
    /// The canonical ValidBlock binding refused subject, wire or execution identity.
    #[error("verified decision cannot bind the retained block: {0}")]
    Block(Box<BlockValidationError>),
    /// The existing commit adapter changed its one exact committed-event contract.
    #[error("verified block commit did not yield its one exact committed event")]
    Event,
}

/// Full original custody returned on every decision-binding refusal.
///
/// The caller can retry an appropriate verified artifact or abandon the owner.
/// No original journal or artifact data is substituted or cloned. The binder
/// clones only the verified artifact's Arc capability so refusal retains it.
pub(crate) struct CarrierDecisionBindingRefusal<Admission, E> {
    /// Binding failure, distinct from an execution or consensus rejection.
    pub(crate) error: CarrierDecisionBindingError<E>,
    /// Original verified artifact, including on canonical block-binding refusal.
    pub(crate) finality: VerifiedV2FinalityArtifact,
    /// Every original journal, deferred effect and capture reservation.
    pub(crate) journals: PreparedCarrierJournals<Admission>,
}

/// Explicitly incomplete authority after global finality has been joined.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum PendingCarrierPublicationAuthorization {
    /// Exact source/economic, retirement, Kura/Native and installation ownership
    /// must still be joined; an empty old-merge authorization cannot complete it.
    SourceAndDurability,
}

/// Actual journals whose sole validated block now retains verified global finality.
///
/// This owner has no publication/reattach operation and does not implement
/// StateBlockCommitAuthorization. All source seals remain held unchanged.
/// TODO: consume this owner only after complete source/retirement/resource and
/// exact Kura/Native durability authorization is retained by the State publisher.
#[must_use = "a decided carrier must retain its journals until authorized publication or drop"]
pub(crate) struct DecisionBoundCarrierJournals<
    Admission,
    BindingAdmission,
    Components = super::DetachedCarrierComponents,
    Checkpoint = (),
> {
    checkpoint: Checkpoint,
    finality: VerifiedV2FinalityArtifact,
    committed_event: BlockEvent,
    journals: PreparedCarrierJournals<Admission, CommittedBlock, Components>,
    // The transient encoding reservation may be released by a later concrete
    // durability owner. Until then it outlives all binding/captured values.
    _binding_admission: BindingAdmission,
}

/// Join canonical consensus identity, not the byte representation of a parent QC.
/// The opaque artifact already proves its round context id equals its context's
/// semantic id. The captured context remains the original authenticated owner.
fn decision_has_retained_context_identity(
    context: &HeightContext,
    finality: &VerifiedV2FinalityArtifact,
) -> bool {
    finality.artifact().commit_qc.round.context_id == context.id()
}

impl<Admission> PreparedCarrierJournals<Admission> {
    /// Consume the actual validated lifecycle once under exact verified finality.
    ///
    /// The callback is mandatory; this API supplies no permissive production
    /// resource policy. It runs before any canonical wire recomputation. All
    /// original journals and their reservation are returned on refusal. No
    /// execution, current-State lookup, BLS re-verification or event delivery runs.
    pub(crate) fn bind_decision<BindingAdmission, E>(
        self,
        finality: VerifiedV2FinalityArtifact,
        admit_binding: impl FnOnce(CarrierDecisionBindingInputs<'_>) -> Result<BindingAdmission, E>,
    ) -> Result<
        DecisionBoundCarrierJournals<Admission, BindingAdmission>,
        CarrierDecisionBindingRefusal<Admission, E>,
    > {
        let fail = |journals, finality, error| CarrierDecisionBindingRefusal {
            error,
            finality,
            journals,
        };
        if finality.artifact().commit_qc.execution_commitment != self.execution_prefix {
            return Err(fail(self, finality, CarrierDecisionBindingError::Execution));
        }
        if self.effects.header != self.valid.as_ref().header() {
            return Err(fail(self, finality, CarrierDecisionBindingError::Header));
        }
        // Declare both reservations before decomposition: unwind must first drop
        // any partially transitioned block and every detached component.
        let binding_admission = match admit_binding(CarrierDecisionBindingInputs {
            block: self.valid.as_ref(),
            context: &self.context,
            execution_prefix: self.execution_prefix,
            finality: finality.artifact(),
        }) {
            Ok(admission) => admission,
            Err(error) => {
                return Err(fail(
                    self,
                    finality,
                    CarrierDecisionBindingError::Admission(error),
                ));
            }
        };
        // Context hashing also encodes the complete semantic policy, so it is
        // covered by binding admission. Parent CommitQC witnesses can differ
        // between honest peers without changing the authenticated decision.
        if !decision_has_retained_context_identity(&self.context, &finality) {
            drop(binding_admission);
            return Err(fail(self, finality, CarrierDecisionBindingError::Context));
        }
        let admission;
        let Self {
            valid,
            context,
            execution_prefix,
            native_amx_manifest,
            source_prefix,
            checkpoint,
            kura,
            components,
            world_effects,
            geometry,
            provider_capture,
            reputation_capture,
            publication_events,
            tiered_snapshot,
            effects,
            admission: original_admission,
        } = self;
        admission = original_admission;
        // One exhaustive original-field list, used in both return paths. There
        // is no State reconstruction, replacement journal or cloned ValidBlock.
        macro_rules! retain_journals {
            ($block:expr) => {
                PreparedCarrierJournals {
                    valid: $block,
                    context,
                    execution_prefix,
                    native_amx_manifest,
                    source_prefix,
                    checkpoint,
                    kura,
                    components,
                    world_effects,
                    geometry,
                    provider_capture,
                    reputation_capture,
                    publication_events,
                    tiered_snapshot,
                    effects,
                    admission,
                }
            };
        }
        let expected_header = valid.as_ref().header();
        let mut committed_event = None;
        let mut event_contract_changed = false;
        let transition = valid
            .commit_with_verified_v2_artifact(finality.clone(), execution_prefix)
            .unpack(|event| match event {
                PipelineEventBox::Block(event)
                    if event.header == expected_header
                        && event.status == BlockStatus::Committed =>
                {
                    if committed_event.replace(event).is_some() {
                        event_contract_changed = true;
                    }
                }
                _ => event_contract_changed = true,
            });
        match transition {
            Ok(committed) => {
                let event = match committed_event {
                    Some(event) if !event_contract_changed => event,
                    _ => {
                        // Even an internal adapter drift returns the exact block
                        // and journals. No committed event was delivered.
                        let journals = retain_journals!(ValidBlock::from(committed));
                        drop(binding_admission);
                        return Err(fail(journals, finality, CarrierDecisionBindingError::Event));
                    }
                };
                Ok(DecisionBoundCarrierJournals {
                    checkpoint: (),
                    finality,
                    committed_event: event,
                    journals: retain_journals!(committed),
                    _binding_admission: binding_admission,
                })
            }
            Err((valid, error)) => {
                let journals = retain_journals!(*valid);
                drop(binding_admission);
                Err(fail(
                    journals,
                    finality,
                    CarrierDecisionBindingError::Block(error),
                ))
            }
        }
    }
}

impl<Admission, BindingAdmission, Components, Checkpoint>
    DecisionBoundCarrierJournals<Admission, BindingAdmission, Components, Checkpoint>
{
    /// Inspect the exact result-bearing block without exposing a mutable owner.
    pub(crate) fn block(&self) -> &SignedBlock {
        self.journals.valid.as_ref()
    }

    /// Inspect the exact artifact retained by the actual CommittedBlock.
    pub(crate) fn finality(&self) -> &V2FinalityArtifact {
        self.finality.artifact()
    }

    /// State the deliberately unresolved authority; global finality alone is not Apply.
    pub(crate) fn publication_authorization(&self) -> PendingCarrierPublicationAuthorization {
        PendingCarrierPublicationAuthorization::SourceAndDurability
    }
}

impl<Admission, BindingAdmission> DecisionBoundCarrierJournals<Admission, BindingAdmission> {
    /// Retain the actual checkpoint writer receipt with the original decision.
    ///
    /// Attachment grants no authority. Its exact Kura, finality and captured
    /// State hash must be reauthenticated under the original publication lease
    /// before acquiring any State writer. The receipt remains owned on refusal.
    pub(crate) fn attach_checkpoint(
        self,
        checkpoint: crate::kura::KuraWsvCheckpointReceipt,
    ) -> DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        super::DetachedCarrierComponents,
        crate::kura::KuraWsvCheckpointReceipt,
    > {
        let Self {
            checkpoint: (),
            finality,
            committed_event,
            journals,
            _binding_admission,
        } = self;
        DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
            _binding_admission,
        }
    }
}

#[path = "physical_publication.rs"]
mod physical_publication;
pub(crate) use physical_publication::PublishedNativeApply;

#[path = "archive_publication.rs"]
pub(crate) mod archive_publication;

#[path = "execution_witness_publication.rs"]
pub(crate) mod execution_witness_publication;

#[cfg(test)]
#[path = "decision_binding_tests.rs"]
mod tests;
