//! Exact lifecycle dispatch joined to the sole retained execution and publisher.
//!
//! The published owner completes durable repair without reexecuting its body,
//! then lends its original Native proof to the sole reducer owner. TODO: connect
//! this path only with complete resource admission and retire scalar execution
//! in the same production cutover.

use super::{
    ExactApplyTaskRef, LifecycleDecisionApplyCompletionV1, LifecycleDecisionApplyTaskV1,
    LifecycleDecisionApplyWorkerResultV1, V2ApplyError, V2ApplyService,
    finalize_committed_block_merge_reservations,
    validation_custody::{
        CarrierCustodyError, CarrierValidator, RetainedBodyValidationService,
        RetainedValidationOwner,
    },
};
use crate::{
    block::VerifiedV2FinalityArtifact,
    state::{PublishedCarrier, PublishedNativeApply},
    sumeragi::{
        v2_body_store::V2BodyStore,
        v2_lane_instance::{LaneApplySettlement, NativeLaneRetirementReceipt},
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::{SignedBlock, consensus_v2 as wire},
    events::{EventBox, pipeline::PipelineEventBox},
};
use std::{collections::BTreeSet, num::NonZeroUsize};

/// Read-only authentication shared by scalar and retained Apply adapters.
pub(super) struct AuthenticatedApplyBody {
    pub(super) body: SignedBlock,
    pub(super) verified_artifact: VerifiedV2FinalityArtifact,
    pub(super) canonical_proposal_wire_hash: Hash,
}

/// Refusal before physical publication; never a consensus rejection receipt.
pub(in crate::sumeragi) enum RetainedLifecycleApplyError<E> {
    /// Frozen task, exact body or genuine CommitQC authentication failed.
    Authentication(V2ApplyError),
    /// The original store/marker/execution owner is unavailable or different.
    Custody(CarrierCustodyError),
    /// The publisher retained its original current execution phase for retry.
    Publication(E),
}

/// The original dispatch is returned on every refusal before publication.
#[must_use = "a refused dispatch and its retained execution must remain owned"]
pub(in crate::sumeragi) struct RetainedLifecycleApplyRefusal<E> {
    /// Unchanged opaque registry dispatch, never reconstructed from a queue key.
    pub(in crate::sumeragi) task: LifecycleDecisionApplyTaskV1,
    /// Local authentication, custody or publication failure.
    pub(in crate::sumeragi) error: RetainedLifecycleApplyError<E>,
}

/// Full postpublication custody awaiting the consuming completion/recovery path.
///
/// No scalar receipt can construct this owner or detach its execution source.
#[must_use = "published execution must survive through completion or recovery"]
pub(in crate::sumeragi) struct PublishedLifecycleDecisionApply<A, B, I> {
    task: LifecycleDecisionApplyTaskV1,
    carrier: PublishedCarrier<A, B, I>,
    finality: VerifiedV2FinalityArtifact,
}

impl<A, B, I> PublishedLifecycleDecisionApply<A, B, I> {
    /// Inspect the exact registry dispatch still attached to publication.
    pub(in crate::sumeragi) fn task(&self) -> &LifecycleDecisionApplyTaskV1 {
        &self.task
    }

    /// Borrow the actual terminal publisher's complete execution/source owner.
    pub(in crate::sumeragi) fn carrier(&self) -> &PublishedCarrier<A, B, I> {
        &self.carrier
    }

    /// Borrow the genuine decision authenticated before any publication work.
    pub(in crate::sumeragi) fn finality(&self) -> &VerifiedV2FinalityArtifact {
        &self.finality
    }
}

/// Postpublication refusal retains the original State/source owner for repair.
#[must_use = "a published refusal must retain its original carrier for recovery"]
pub(in crate::sumeragi) struct PublishedLifecycleCompletionRefusal<A, B, I> {
    /// The same complete owner returned without reconstructing its execution.
    pub(in crate::sumeragi) owner: PublishedLifecycleDecisionApply<A, B, I>,
    /// Durable repair or exact association failure after State publication.
    pub(in crate::sumeragi) error: V2ApplyError,
}

/// Durable global completion still retaining every original Native Apply debt.
#[must_use = "the original Native Apply effects must be settled before worker completion"]
pub(in crate::sumeragi) struct CompletedPublishedLifecycleDecisionApply<A, B, I> {
    completion: LifecycleDecisionApplyCompletionV1,
    carrier: PublishedCarrier<A, B, I>,
}

/// Reducer Apply is settled but the same carrier still owes closed retirement.
#[must_use = "every original closed Native instance must retire before worker completion"]
pub(in crate::sumeragi) struct NativeAppliedPublishedLifecycleDecisionApply<A, B, I> {
    completion: LifecycleDecisionApplyCompletionV1,
    carrier: PublishedCarrier<A, B, I>,
    retired: BTreeSet<wire::HeightContextId>,
}

/// Why the original Native Apply effects cannot yet be acknowledged.
pub(in crate::sumeragi) enum NativeApplyDeliveryError<E> {
    /// The published source lost its exact bounded instance inventory.
    Custody(String),
    /// The same original reducer owner is still preparing or backpressured.
    Pending(wire::HeightContextId),
    /// The original Native driver returned its local settlement refusal.
    Local(E),
}

/// Why at least one original closed instance still owns terminal custody.
pub(in crate::sumeragi) enum NativeRetirementDeliveryError<E> {
    /// The published source or returned receipt changed its exact identity.
    Custody(String),
    /// Physical drain has not yet yielded the original closed owner.
    Pending(wire::HeightContextId),
    /// The sole runtime retained its original owner after a local refusal.
    Local(E),
}

impl<A, B, I> CompletedPublishedLifecycleDecisionApply<A, B, I> {
    /// Inspect exact durable completion without detaching its Native source.
    pub(in crate::sumeragi) fn completion(&self) -> &LifecycleDecisionApplyCompletionV1 {
        &self.completion
    }

    /// Borrow actual published custody while local reducer work drains.
    pub(in crate::sumeragi) fn carrier(&self) -> &PublishedCarrier<A, B, I> {
        &self.carrier
    }

    /// Settle every original Native Apply using the published source proof.
    ///
    /// A partial success is retryable: already-applied local instances stutter,
    /// and all remaining instances still receive the very same published owner.
    /// Even full Apply settlement returns that owner. Terminal retirement of
    /// each original closed instance still needs this proof, so no worker
    /// completion may be emitted at this boundary. The separate retirement
    /// operation must consume every closed owner before registry completion.
    pub(in crate::sumeragi) fn settle_original_native_apply<E>(
        self,
        mut settle: impl FnMut(
            wire::HeightContextId,
            &PublishedNativeApply<'_>,
        ) -> Result<Option<LaneApplySettlement>, E>,
    ) -> Result<
        NativeAppliedPublishedLifecycleDecisionApply<A, B, I>,
        (Self, NativeApplyDeliveryError<E>),
    > {
        let attempt = (|| {
            let published = self.carrier.native_apply().ok_or_else(|| {
                NativeApplyDeliveryError::Custody("published Native source is unavailable".into())
            })?;
            let instances = published
                .original_instance_ids()
                .map_err(NativeApplyDeliveryError::Custody)?;
            for id in instances {
                match settle(id, &published).map_err(NativeApplyDeliveryError::Local)? {
                    Some(LaneApplySettlement::Applied(_) | LaneApplySettlement::AlreadyApplied) => {
                    }
                    Some(LaneApplySettlement::NotReady | LaneApplySettlement::Backpressured)
                    | None => return Err(NativeApplyDeliveryError::Pending(id)),
                }
            }
            Ok(())
        })();
        if let Err(error) = attempt {
            return Err((self, error));
        }
        Ok(NativeAppliedPublishedLifecycleDecisionApply {
            completion: self.completion,
            carrier: self.carrier,
            retired: BTreeSet::new(),
        })
    }
}

impl<A, B, I> NativeAppliedPublishedLifecycleDecisionApply<A, B, I> {
    /// Retire each original closed instance through its consuming owner.
    ///
    /// A receipt is accepted only when the real closed-owner operation minted
    /// it for this exact borrowed proof and instance. Completed IDs remain
    /// attached to this carrier across partial local refusals, while the sole
    /// runtime retains every refused closed owner for a later retry.
    pub(in crate::sumeragi) fn retire_original_native<E>(
        mut self,
        mut retire: impl for<'proof, 'carrier> FnMut(
            wire::HeightContextId,
            &'proof PublishedNativeApply<'carrier>,
        ) -> Result<
            Option<NativeLaneRetirementReceipt<'proof, 'carrier>>,
            E,
        >,
    ) -> Result<LifecycleDecisionApplyWorkerResultV1, (Self, NativeRetirementDeliveryError<E>)>
    {
        let attempt = (|| {
            let published = self.carrier.native_apply().ok_or_else(|| {
                NativeRetirementDeliveryError::Custody(
                    "published Native source is unavailable".into(),
                )
            })?;
            let instances = published
                .original_instance_ids()
                .map_err(NativeRetirementDeliveryError::Custody)?;
            for id in instances {
                if self.retired.contains(&id) {
                    continue;
                }
                let Some(receipt) =
                    retire(id, &published).map_err(NativeRetirementDeliveryError::Local)?
                else {
                    return Err(NativeRetirementDeliveryError::Pending(id));
                };
                if !receipt.matches(id, &published) {
                    return Err(NativeRetirementDeliveryError::Custody(
                        "Native retirement receipt differs from original instance or publication"
                            .into(),
                    ));
                }
                self.retired.insert(id);
            }
            Ok(())
        })();
        if let Err(error) = attempt {
            return Err((self, error));
        }
        Ok(LifecycleDecisionApplyWorkerResultV1::Applied(
            self.completion,
        ))
    }
}

/// Publication has happened; neither branch permits candidate reexecution.
#[must_use = "both outcomes retain actual published custody for completion or recovery"]
pub(in crate::sumeragi) enum RetainedLifecycleApplyPublication<A, B, I> {
    /// Exact publication may enter the still-required durable completion path.
    PendingCompletion(PublishedLifecycleDecisionApply<A, B, I>),
    /// Preserve all published custody even if its final association is invalid.
    RecoveryRequired {
        /// Original dispatch and actual published owner, never a replacement task.
        owner: PublishedLifecycleDecisionApply<A, B, I>,
        /// Postpublication association failure requiring recovery.
        error: V2ApplyError,
    },
}

impl V2ApplyService {
    /// Finish an already-published retained decision or retry its interrupted repair.
    ///
    /// This consumes the original terminal carrier and never enters candidate
    /// validation or `validate_and_apply`. Every postpublication refusal returns
    /// that carrier unchanged so a later repair can resume without execution.
    pub(in crate::sumeragi) fn complete_published_lifecycle_decision_apply<A, B, I>(
        &self,
        mut owner: PublishedLifecycleDecisionApply<A, B, I>,
    ) -> Result<
        CompletedPublishedLifecycleDecisionApply<A, B, I>,
        PublishedLifecycleCompletionRefusal<A, B, I>,
    > {
        let complete = (|| -> Result<crate::kura::KuraV2CommitReceipt, V2ApplyError> {
            let task = &owner.task;
            let artifact = owner.finality.artifact();
            let context = &artifact.height_context;
            if !task.dispatch_identity.matches_height_context(context)
                || task.exact_lineage().is_none()
                || task.dispatch_key().lifecycle_ordinal() == 0
                || task.subject() != artifact.subject
                || task.certificate() != &artifact.commit_qc
                || task.certificate().execution_commitment
                    != task.validated_receipt().execution_commitment()
                || task.validated_receipt().durable().context_id() != context.id()
                || task.validated_receipt().durable().subject() != task.subject()
            {
                return Err(V2ApplyError::committed_recovery_required(
                    "published lifecycle association",
                    &"original registry task differs from its authenticated publication",
                ));
            }
            let published = owner.carrier.block();
            let published_wire = published
                .encode_wire()
                .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
            if published.hash() != task.subject().block_hash
                || published
                    .canonical_proposal_wire_hash()
                    .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?
                    != task.subject().payload_hash
                || u64::try_from(published_wire.len()).ok()
                    != Some(
                        task.certificate()
                            .execution_commitment
                            .executed_block_wire_len,
                    )
                || Hash::new(&published_wire)
                    != task
                        .certificate()
                        .execution_commitment
                        .executed_block_wire_hash
            {
                return Err(V2ApplyError::committed_recovery_required(
                    "published execution association",
                    &"original published block differs from the exact CommitQC execution",
                ));
            }
            let receipt = owner
                .carrier
                .reauthenticate_exact_publication(self.state.as_ref(), &self.kura, &owner.finality)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "published State and checkpoint custody",
                        &error,
                    )
                })?;
            let height = usize::try_from(context.height)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or(V2ApplyError::HeightOverflow)?;
            let stored = self
                .kura
                .read_block_body_with_verified_finality(height, &owner.finality)
                .map_err(V2ApplyError::CanonicalStorageRead)?
                .ok_or(V2ApplyError::StateAheadOfKura)?;
            if stored
                .encode_wire()
                .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?
                != published_wire
            {
                return Err(V2ApplyError::committed_recovery_required(
                    "published Kura body readback",
                    &"canonical body differs from the original published execution",
                ));
            }

            // The original checkpoint crossed the WSV boundary inside the
            // publisher. All following operations are idempotent restart repair.
            self.publish_finalized_lane_relays(published, artifact)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "lane-finality relay publication",
                        &error,
                    )
                })?;
            self.persist_post_apply_metadata(context, task.subject(), artifact)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required("post-apply metadata", &error)
                })?;
            self.kura
                .repair_native_amx_participant_application_evidence(published)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "Native AMX participant evidence repair",
                        &error,
                    )
                })?;
            self.publish_committed_block_merge_entry(published)?;
            self.kura
                .promote_kagemusha_finality_sidecar(artifact, &receipt)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "Kagemusha V1 finality sidecar promotion",
                        &error,
                    )
                })?;
            self.publish_kagemusha_mint_outbox_v1(artifact)?;
            finalize_committed_block_merge_reservations(
                self.state.as_ref(),
                self.queue.as_ref(),
                self.kura.as_ref(),
                published,
                self.network_id,
            )
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("merge reservation finalization", &error)
            })?;
            self.queue
                .remove_state_committed_replay_owners_preserving_globally_bound(
                    &self.state.view(),
                    None,
                )
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "replay-terminal committed Queue cleanup",
                        &error,
                    )
                })?;
            let nexus = self.state.nexus_snapshot();
            let compliance = self.queue.lane_compliance_engine();
            self.queue
                .reconfigure_nexus_with_state(&nexus, self.state.as_ref(), compliance);
            Ok(receipt)
        })();
        let receipt = match complete {
            Ok(receipt) => receipt,
            Err(error) => return Err(PublishedLifecycleCompletionRefusal { owner, error }),
        };
        let (committed_event, events) = owner.carrier.take_completion_events();
        let _ = self
            .events_sender
            .send(EventBox::Pipeline(PipelineEventBox::Block(committed_event)));
        for event in events {
            let _ = self.events_sender.send(event);
        }
        let task = owner.task;
        Ok(CompletedPublishedLifecycleDecisionApply {
            completion: LifecycleDecisionApplyCompletionV1 {
                dispatch_identity: task.dispatch_identity,
                subject: task.subject,
                certificate: task.certificate,
                validated_receipt: task.validated_receipt,
                receipt,
                artifact: owner.finality.artifact().clone(),
            },
            carrier: owner.carrier,
        })
    }

    /// Authenticate without modifying Kura, State, Queue or retained execution.
    pub(super) fn authenticate_exact_apply_body(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        task: ExactApplyTaskRef<'_>,
    ) -> Result<AuthenticatedApplyBody, V2ApplyError> {
        context.validate()?;
        let durable = task.validated_receipt().durable();
        if task.subject() != task.certificate().subject
            || task.certificate().phase != wire::GlobalPhase::Commit
            || task.certificate().round.context_id != context.id()
            || task.certificate().round.height != context.height
            || durable.context_id() != context.id()
            || durable.round() != task.certificate().proposal_round
            || durable.subject() != task.subject()
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        task.certificate().execution_commitment.validate()?;
        if task.certificate().execution_commitment
            != task.validated_receipt().execution_commitment()
        {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        let body = body_store.load(durable)?;
        let canonical_proposal_wire_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
        if !body.is_resultless_proposal()
            || body.hash() != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
            || canonical_proposal_wire_hash != task.subject().payload_hash
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        let verified_artifact =
            VerifiedV2FinalityArtifact::verify(wire::finality::V2FinalityArtifact::new(
                context.clone(),
                task.subject(),
                task.certificate().clone(),
                self.validator_set_pops.clone(),
            ))
            .map_err(V2ApplyError::FinalityCryptography)?;
        verified_artifact
            .artifact()
            .validate_for_header(&body.header())?;
        Ok(AuthenticatedApplyBody {
            body,
            verified_artifact,
            canonical_proposal_wire_hash,
        })
    }

    /// Consume only the original validation owner under the exact registry task.
    ///
    /// The callback must perform the real consuming State publisher with its own
    /// exact source, resource, Decision and durability authority. Every local
    /// refusal returns that same current phase (including newly attached Decision
    /// or checkpoint) to its original cache slot. Successful return accepts only
    /// the publisher's opaque terminal owner, never a scalar Apply result.
    pub(in crate::sumeragi) fn publish_retained_lifecycle_decision_apply<P, A, B, I, E>(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        service: &mut RetainedBodyValidationService<P>,
        task: LifecycleDecisionApplyTaskV1,
        publish: impl FnOnce(
            &P,
            P::Owner,
            &VerifiedV2FinalityArtifact,
        ) -> Result<PublishedCarrier<A, B, I>, (P::Owner, E)>,
    ) -> Result<RetainedLifecycleApplyPublication<A, B, I>, RetainedLifecycleApplyRefusal<E>>
    where
        P: CarrierValidator,
    {
        use super::LifecycleDecisionApplyLineageV1;

        let mut authenticate = || {
            if !task.dispatch_identity.matches_height_context(context)
                || task.dispatch_key().lifecycle_ordinal() == 0
                || task.exact_tag().height() != context.height
            {
                return Err(RetainedLifecycleApplyError::Authentication(
                    V2ApplyError::TaskMismatch,
                ));
            }
            if !service.matches_store(&body_store.instance_identity()) {
                return Err(RetainedLifecycleApplyError::Custody(
                    CarrierCustodyError::Identity,
                ));
            }
            let exact = match task.exact_lineage() {
                Some(LifecycleDecisionApplyLineageV1::Live) => {
                    ExactApplyTaskRef::LifecycleLive(&task)
                }
                Some(LifecycleDecisionApplyLineageV1::Recovered) => {
                    ExactApplyTaskRef::LifecycleRecovered(&task)
                }
                None => {
                    return Err(RetainedLifecycleApplyError::Authentication(
                        V2ApplyError::TaskMismatch,
                    ));
                }
            };
            self.authenticate_exact_apply_body(context, body_store, exact)
                .map_err(RetainedLifecycleApplyError::Authentication)
        };
        let authenticated = match authenticate() {
            Ok(authenticated) => authenticated,
            Err(error) => return Err(RetainedLifecycleApplyRefusal { task, error }),
        };
        let selected = match service.select(task.validated_receipt()) {
            Ok(selected) => selected,
            Err(error) => {
                return Err(RetainedLifecycleApplyRefusal {
                    task,
                    error: RetainedLifecycleApplyError::Custody(error),
                });
            }
        };
        let carrier = match selected.try_consume(|producer, owner| {
            if !owner.matches_candidate(context, &authenticated.body)
                || owner.ready_commitment() != Some(task.certificate().execution_commitment)
            {
                return Err((
                    owner,
                    RetainedLifecycleApplyError::Custody(CarrierCustodyError::Identity),
                ));
            }
            publish(producer, owner, &authenticated.verified_artifact)
                .map_err(|(owner, error)| (owner, RetainedLifecycleApplyError::Publication(error)))
        }) {
            Ok(carrier) => carrier,
            Err(error) => return Err(RetainedLifecycleApplyRefusal { task, error }),
        };
        let owner = PublishedLifecycleDecisionApply {
            task,
            carrier,
            finality: authenticated.verified_artifact,
        };
        // Publication is irreversible here. Preserve its full owner even if a
        // wrong consuming publisher returned another terminal carrier.
        let association = owner
            .finality
            .artifact()
            .validate_for_header(&owner.carrier.block().header())
            .map_err(V2ApplyError::from)
            .and_then(|()| {
                let actual = owner.carrier.block().executed_block_wire_hash().map_err(
                    |error: norito::core::Error| V2ApplyError::CanonicalBlock(error.to_string()),
                )?;
                if actual
                    != owner
                        .task
                        .certificate()
                        .execution_commitment
                        .executed_block_wire_hash
                {
                    return Err(V2ApplyError::ExecutionCommitmentMismatch);
                }
                Ok(())
            });
        Ok(match association {
            Ok(()) => RetainedLifecycleApplyPublication::PendingCompletion(owner),
            Err(error) => RetainedLifecycleApplyPublication::RecoveryRequired { owner, error },
        })
    }
}
