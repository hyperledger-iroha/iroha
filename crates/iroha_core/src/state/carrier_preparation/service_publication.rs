//! Drive the original retained carrier to visibility without repeating execution.
//!
//! Durable retries keep their latest owned phase. The caller retains its real
//! descriptor and shell reservation; standard block/wire/COW allocations are not
//! represented as completely prepaid process memory.

use super::physical_publication::{
    CarrierPhysicalPreparationError, publication::CarrierPublicationError,
};
use super::{PublishedCarrier, RetainedCarrier};
use crate::{
    block::VerifiedV2FinalityArtifact,
    kura::CommitManifest,
    state::State,
    sumeragi::{
        v2_apply::carrier_queue_retirement::OriginalCarrierQueue,
        v2_body_store::{BodyValidationBusy, LocalValidationRefusal},
    },
};
use std::task::Waker;

fn busy(
    field: &'static str,
    wait: &concread::release::ReleaseWait,
    wake: &Waker,
) -> LocalValidationRefusal {
    LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(field, wait.clone(), wake.clone()))
}

fn archive_refusal(
    error: &super::super::CarrierArchivePreparationError,
    wake: &Waker,
) -> LocalValidationRefusal {
    use super::super::CarrierArchivePreparationError as E;
    use crate::query::{
        provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1 as P,
        reputation_finalized::ReputationFinalizedArchiveError as R,
    };
    match error {
        E::Provider(error) if matches!(error.as_ref(), P::IndexBusy { .. }) => {
            let P::IndexBusy { wait } = error.as_ref() else {
                unreachable!()
            };
            busy("provider_archive_index", wait, wake)
        }
        E::Reputation(error) if matches!(error.as_ref(), R::IndexBusy { .. }) => {
            let R::IndexBusy { wait } = error.as_ref() else {
                unreachable!()
            };
            busy("reputation_archive_index", wait, wake)
        }
        E::Provider(error) if matches!(error.as_ref(), P::CaptureReserved { .. }) => {
            let P::CaptureReserved { wait } = error.as_ref() else {
                unreachable!()
            };
            busy("provider_archive_capture", wait.release_wait(), wake)
        }
        E::Reputation(error) if matches!(error.as_ref(), R::CaptureReserved { .. }) => {
            let R::CaptureReserved { wait } = error.as_ref() else {
                unreachable!()
            };
            busy("reputation_archive_capture", wait.release_wait(), wake)
        }
        _ => LocalValidationRefusal::RecoveryRequired(error.to_string()),
    }
}

fn physical_refusal(
    error: &CarrierPhysicalPreparationError,
    wake: &Waker,
) -> LocalValidationRefusal {
    use super::super::runtime_journals::RuntimePublicationError as R;
    use super::archive_publication::CarrierArchivePublicationError as A;
    use super::physical_publication::CarrierPhysicalPreparationError as E;
    use crate::kura::KuraPublicationPreparationError as K;
    use crate::query::{
        provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1 as P,
        reputation_finalized::ReputationFinalizedArchiveError as RArch,
    };
    use crate::state::carrier_preparation::queue_retirement::CarrierQueueRetirementError as Q;
    use crate::state::world_journals::publication::WorldPublicationError as W;
    use mv::PublicationPreparationError as M;
    match error {
        E::Fence { field, wait }
        | E::Queue(Q::Busy { field, wait })
        | E::Kura(K::Busy { field, wait })
        | E::Component {
            field,
            cause: M::Busy(wait),
        }
        | E::Runtime(R::Component {
            field,
            cause: M::Busy(wait),
        }) => busy(field, wait, wake),
        E::Queue(Q::Pending { wait, .. }) => busy("retiring_lane_queue", wait, wake),
        E::World(W::Field(refusal)) => match &refusal.cause {
            M::Busy(wait) => busy(refusal.field, wait, wake),
            M::Admission(mv::storage::AdmittedStorageError::Busy { release, .. })
            | M::Admission(mv::storage::AdmittedStorageError::Allocation(
                mv::allocation::AllocationRefusal::Capacity { release, .. },
            )) => busy(refusal.field, release, wake),
            _ => {
                LocalValidationRefusal::RecoveryRequired(format!("carrier publication: {error:?}"))
            }
        },
        E::Provider(P::IndexBusy { wait }) | E::Archive(A::Provider(P::IndexBusy { wait })) => {
            busy("provider_archive_index", wait, wake)
        }
        E::Reputation(RArch::IndexBusy { wait })
        | E::Archive(A::Reputation(RArch::IndexBusy { wait })) => {
            busy("reputation_archive_index", wait, wake)
        }
        _ => LocalValidationRefusal::RecoveryRequired(format!("carrier publication: {error:?}")),
    }
}

impl<A> RetainedCarrier<A> {
    /// Complete capture, exact decision binding, durability and actual publication.
    /// Every refusal returns the latest phase with all original source/journal
    /// ownership. No caller can pass a replacement block or execution commitment.
    pub(crate) fn try_publish(
        self,
        target: &State,
        queue: &OriginalCarrierQueue<'_>,
        finality: VerifiedV2FinalityArtifact,
        wake: Waker,
    ) -> Result<PublishedCarrier<A>, (Self, LocalValidationRefusal)> {
        let original = match self.resume_capture() {
            Ok(owner) => owner,
            Err((owner, error)) => return Err((owner, archive_refusal(&error, &wake))),
        };
        let matches_target = match &original {
            Self::Capturing(_) => unreachable!("capture completed above"),
            Self::Validated(journals) => {
                target.matches_kura_instance(&journals.kura)
                    && journals
                        .geometry
                        .matches_publication_target(target, journals.valid.as_ref().header())
            }
            Self::Decided(decision) => {
                target.matches_kura_instance(&decision.journals.kura)
                    && decision
                        .journals
                        .geometry
                        .matches_publication_target(target, decision.block().header())
            }
            Self::Checkpointed(decision) => {
                target.matches_kura_instance(&decision.journals.kura)
                    && decision
                        .journals
                        .geometry
                        .matches_publication_target(target, decision.block().header())
            }
        };
        if !matches_target || !queue.belongs_to(target) {
            return Err((
                original,
                LocalValidationRefusal::RecoveryRequired(
                    "carrier publication belongs to another original State, Kura or Queue"
                        .to_owned(),
                ),
            ));
        }
        let original = match original {
            Self::Validated(journals) => match journals.bind_decision(finality.clone()) {
                Ok(decision) => Self::Decided(decision),
                Err(refusal) => {
                    return Err((
                        Self::Validated(refusal.journals),
                        LocalValidationRefusal::RecoveryRequired(refusal.error.to_string()),
                    ));
                }
            },
            other => other,
        };
        let same_finality = match &original {
            Self::Decided(decision) => decision.finality() == finality.artifact(),
            Self::Checkpointed(decision) => decision.finality() == finality.artifact(),
            _ => unreachable!("original exact decision bound above"),
        };
        if !same_finality {
            return Err((
                original,
                LocalValidationRefusal::RecoveryRequired(
                    "retry finality differs from the original retained decision".to_owned(),
                ),
            ));
        }
        let original = match original {
            Self::Decided(decision) => {
                // Each writer is idempotent for these exact original bytes. A
                // failed later writer leaves this same decided owner for retry.
                let kura = &decision.journals.kura;
                let durable = (|| {
                    kura.store_block(decision.block().clone())?;
                    // Finality is the restart commit marker. Publish the exact
                    // captured checkpoint and its authenticated manifest first;
                    // every earlier crash cut remains one recoverable pending tip.
                    // Never derive this checkpoint from a later live State.
                    let artifact = decision.finality();
                    let checkpoint = decision.journals.checkpoint;
                    kura.store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)?;
                    kura.store_commit_manifest(
                        CommitManifest::new(
                            artifact.height,
                            artifact.block_hash,
                            None,
                            None,
                            checkpoint,
                            None,
                        )
                        .with_authenticated_v2_commit_authority(artifact),
                    )?;
                    let receipt = kura.store_v2_finality_artifact(artifact)?;
                    kura.persist_wsv_checkpoint_for_v2_commit(
                        &receipt,
                        decision.journals.checkpoint,
                    )
                })();
                match durable {
                    Ok(checkpoint) => Self::Checkpointed(decision.attach_checkpoint(checkpoint)),
                    Err(error) => {
                        return Err((
                            Self::Decided(decision),
                            LocalValidationRefusal::RecoveryRequired(format!(
                                "carrier durability: {error}"
                            )),
                        ));
                    }
                }
            }
            other => other,
        };
        let Self::Checkpointed(decision) = original else {
            unreachable!("exact checkpoint attached above")
        };
        let prepared = match decision.try_prepare_physical(target, Some(queue)) {
            Ok(prepared) => prepared,
            Err((decision, error)) => {
                return Err((
                    Self::Checkpointed(decision),
                    physical_refusal(&error, &wake),
                ));
            }
        };
        prepared.publish().map_err(|(decision, error)| {
            let refusal = match &error {
                CarrierPublicationError::GeometryStorage(
                    crate::state::LaneLifecycleError::PublicationBusy { field, wait },
                ) => busy(field, wait, &wake),
                _ => LocalValidationRefusal::RecoveryRequired(error.to_string()),
            };
            (Self::Checkpointed(decision), refusal)
        })
    }
}
