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

/// Refusal of decision binding; no variant authorizes publication or reexecution.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CarrierDecisionBindingError {
    /// The verified decision belongs to another semantic frozen context identity.
    #[error("decision differs from the retained frozen height context")]
    Context,
    /// The verified decision commits different execution than the actual owner.
    #[error("decision differs from the retained execution-prefix commitment")]
    Execution,
    /// An internal carrier owner lost its exact proposal header.
    #[error("retained carrier metadata differs from its validated proposal")]
    Header,
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
pub(crate) struct CarrierDecisionBindingRefusal<Admission> {
    /// Binding failure, distinct from an execution or consensus rejection.
    pub(crate) error: CarrierDecisionBindingError,
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
/// All source seals remain held unchanged. The consuming publisher joins the
/// original State, Queue and durable Kura receipt before acquiring writers.
/// Its retained admission covers concrete shells; nested payload allocation
/// remains governed by the component's existing allocation policy.
#[must_use = "a decided carrier must retain its journals until authorized publication or drop"]
pub(crate) struct DecisionBoundCarrierJournals<
    Admission,
    Components = super::DetachedCarrierComponents,
    Checkpoint = (),
> {
    checkpoint: Checkpoint,
    finality: VerifiedV2FinalityArtifact,
    committed_event: BlockEvent,
    journals: PreparedCarrierJournals<Admission, CommittedBlock, Components>,
}

/// The one original detached carrier through decision and durability attachment.
///
/// Each phase owns the previous phase's journals and admissions by move. A local
/// publication refusal returns the current phase to the same preallocated
/// candidate slot; it must never reconstruct a ValidBlock or execute again.
/// Checkpoint attachment still grants no publication or retirement authority.
#[must_use = "retain the current carrier phase until authorized publication or drop"]
pub(crate) enum RetainedCarrier<Admission> {
    /// Original detached execution awaiting its original archive capture owners.
    Capturing(Box<super::StagedCarrierCapture<Admission>>),
    /// Actual detached execution, before an exact verified decision is joined.
    Validated(PreparedCarrierJournals<Admission>),
    /// The same journals after consuming their ValidBlock under verified finality.
    Decided(DecisionBoundCarrierJournals<Admission>),
    /// The same decided carrier with its actual checkpoint writer receipt.
    Checkpointed(
        DecisionBoundCarrierJournals<
            Admission,
            super::DetachedCarrierComponents,
            crate::kura::KuraWsvCheckpointReceipt,
        >,
    ),
}

impl<Admission> RetainedCarrier<Admission> {
    /// Borrow the manifest captured from original execution without State rereads.
    pub(crate) fn native_amx_manifest(
        &self,
    ) -> &crate::sumeragi::exec::NativeAmxApplicationManifestV1 {
        match self {
            Self::Capturing(carrier) => &carrier.journals.native_amx_manifest,
            Self::Validated(journals) => &journals.native_amx_manifest,
            Self::Decided(carrier) => &carrier.journals.native_amx_manifest,
            Self::Checkpointed(carrier) => &carrier.journals.native_amx_manifest,
        }
    }

    /// Compare the original context and proposal in every retained phase.
    pub(crate) fn matches_validation_candidate(
        &self,
        context: &HeightContext,
        proposal: &SignedBlock,
    ) -> bool {
        match self {
            Self::Capturing(carrier) => carrier.matches_candidate(context, proposal),
            Self::Validated(journals) => journals.matches_validation_candidate(context, proposal),
            Self::Decided(carrier) => carrier
                .journals
                .matches_validation_candidate(context, proposal),
            Self::Checkpointed(carrier) => carrier
                .journals
                .matches_validation_candidate(context, proposal),
        }
    }

    /// Expose the original prefix only after all original captures are complete.
    pub(crate) fn ready_commitment(&self) -> Option<ExecutionCommitment> {
        match self {
            Self::Capturing(_) => None,
            Self::Validated(journals) => Some(journals.execution_prefix_commitment()),
            Self::Decided(carrier) => Some(carrier.journals.execution_prefix_commitment()),
            Self::Checkpointed(carrier) => Some(carrier.journals.execution_prefix_commitment()),
        }
    }

    /// Resume only the original capture; every refusal retains the current phase.
    pub(crate) fn resume_capture(
        self,
    ) -> Result<Self, (Self, super::CarrierArchivePreparationError)> {
        match self {
            Self::Capturing(carrier) => carrier
                .try_complete()
                .map(Self::Validated)
                .map_err(|(carrier, error)| (Self::Capturing(carrier), error)),
            ready => Ok(ready),
        }
    }
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
    /// Every original journal and its retained shell reservation is returned on
    /// refusal. Canonical block/wire temporaries use standard allocation and are
    /// not included in that reservation. No execution, current-State lookup,
    /// BLS re-verification or event delivery runs.
    pub(crate) fn bind_decision(
        self,
        finality: VerifiedV2FinalityArtifact,
    ) -> Result<DecisionBoundCarrierJournals<Admission>, CarrierDecisionBindingRefusal<Admission>>
    {
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
        // Canonical binding uses standard transient allocations. The one retained
        // carrier admission covers only its explicitly named shells and effects.
        if !decision_has_retained_context_identity(&self.context, &finality) {
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

                        return Err(fail(journals, finality, CarrierDecisionBindingError::Event));
                    }
                };
                Ok(DecisionBoundCarrierJournals {
                    checkpoint: (),
                    finality,
                    committed_event: event,
                    journals: retain_journals!(committed),
                })
            }
            Err((valid, error)) => {
                let journals = retain_journals!(*valid);

                Err(fail(
                    journals,
                    finality,
                    CarrierDecisionBindingError::Block(error),
                ))
            }
        }
    }
}

impl<Admission, Components, Checkpoint>
    DecisionBoundCarrierJournals<Admission, Components, Checkpoint>
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

impl<Admission> DecisionBoundCarrierJournals<Admission> {
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
        super::DetachedCarrierComponents,
        crate::kura::KuraWsvCheckpointReceipt,
    > {
        let Self {
            checkpoint: (),
            finality,
            committed_event,
            journals,
        } = self;
        DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
        }
    }
}

#[path = "service_publication.rs"]
mod service_publication;

#[path = "physical_publication.rs"]
mod physical_publication;
pub(crate) use physical_publication::PublishedCarrier;
pub(crate) use physical_publication::PublishedNativeApply;

#[path = "archive_publication.rs"]
pub(crate) mod archive_publication;

#[path = "execution_witness_publication.rs"]
pub(crate) mod execution_witness_publication;

#[cfg(test)]
#[path = "decision_binding_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use physical_publication::publish_governance_fixture;
