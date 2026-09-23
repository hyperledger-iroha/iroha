//! Consume the actual decided component owners within one State visibility cut.
//!
//! Namespace transitions consume their original retryable storage owners before
//! State visibility while retaining the original Queue retirement cut. Native
//! Decisions use their original source, Kura custody and World application markers;
//! retired participant manifests remain refused. Actual source and execution
//! owners survive worker handoff; nested payload allocation remains outside the
//! concrete descriptor and shell admission policy.

use super::super::super::{PreparedCarrierJournals, RetainedCarrierEffects};
use super::*;
use crate::{block::CommittedBlock, state::EventBox};

/// An unmet publication obligation, never invalidity of a decided proposal.
#[derive(Debug, thiserror::Error)]
pub(in crate::state::carrier_preparation::journals) enum CarrierPublicationError {
    /// The original prepared execution no longer matches its immutable carrier.
    #[error("prepared execution no longer matches its immutable carrier")]
    Source,
    /// The retained transition differs from its original State, header or effects.
    #[error("retained geometry differs from its original State, header or effects")]
    Geometry,
    /// Retirement/replacement still needs the original service Queue custody.
    #[error("lane retirement requires the original service Queue custody")]
    QueueRetirementRequired,
    /// The retained original Queue latched a recovery fault before visibility.
    #[error("original Queue retirement custody is unavailable: {0:?}")]
    QueueRetirement(CarrierQueueRetirementError),
    /// The retained geometry storage attempt must retry or recover before visibility.
    #[error("retained geometry storage must complete before publication: {0}")]
    GeometryStorage(#[source] crate::state::LaneLifecycleError),
    /// Old participant evidence has not supplied complete durable application custody.
    #[error("participant evidence requires complete durable application custody")]
    ParticipantDurability,
    /// A prevalidation scratch owner cannot publish State.
    #[error("prevalidation cannot publish State")]
    Prevalidation,
}

/// Actual published State and its original block, events, source and reservations.
/// This token has no second publication or reexecution operation.
/// Move the complete owner across worker completion before borrowing Native Apply
/// authority; its original source and its retained shell reservation remain attached.
#[must_use = "retain published custody until original lane Apply completion is delivered"]
pub(crate) struct PublishedCarrier<A> {
    block: CommittedBlock,
    committed_event: iroha_data_model::events::pipeline::BlockEvent,
    events: Vec<EventBox>,
    checkpoint: KuraWsvCheckpointReceipt,
    finality: crate::block::VerifiedV2FinalityArtifact,
    // Original opaque State family, retained without a State borrow or pointer ABA.
    state_owner: crate::state::NativeLaneStateOwner,
    source: super::super::super::super::execution_prefix::ValidatedExecutionPrefix,
    // These outlive all values retained for completion delivery.
    _admission: A,
}

/// Borrowed proof of completed global publication of the original Native source.
///
/// Only the terminal publisher exposes this value. Its borrows retain the actual
/// carrier/source and their resource owners; wire evidence cannot construct it.
#[must_use]
pub(crate) struct PublishedNativeApply<'published> {
    state_owner: &'published crate::state::NativeLaneStateOwner,
    block: &'published iroha_data_model::block::SignedBlock,
    source: &'published crate::state::NativeExecutionCustody,
}

impl PublishedNativeApply<'_> {
    /// Enumerate only original authenticated instances from the published source.
    pub(crate) fn instance_ids(
        &self,
    ) -> impl Iterator<Item = iroha_data_model::block::consensus_v2::HeightContextId> + '_ {
        self.source
            .sources()
            .iter()
            .flat_map(|group| group.contexts().iter().map(|context| context.instance_id()))
    }

    // Authenticate the actual published group before using it for either Apply
    // settlement or terminal retirement. A global finality/QC by itself cannot
    // construct this borrowed proof or replace its original State/source owner.
    fn published_instance(
        &self,
        owner: &crate::state::NativeLaneStateOwner,
        instance: &crate::state::VerifiedLaneContext,
    ) -> Result<
        (
            &iroha_data_model::block::lane_consensus::LaneDecisionV1,
            crate::sumeragi::v2_core::Subject,
        ),
        String,
    > {
        if !self.state_owner.same_family(owner)
            || !self
                .source
                .retains_carrier(self.block, self.source.context().context())
        {
            return Err("Native application proof belongs to another State or source".into());
        }
        let auth = crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(instance);
        for group in self.source.sources() {
            for (context, published) in group.contexts().iter().zip(group.decisions()) {
                if context.instance_id() != instance.instance_id() {
                    continue;
                }
                if context.frozen() != instance.frozen() {
                    return Err(
                        "Native application proof differs from the original frozen context".into(),
                    );
                }
                let actual = auth
                    .decision_certificate(published)
                    .map_err(|error| error.to_string())?;
                return Ok((published, actual.subject()));
            }
        }
        Err("Native application proof does not contain this original instance/group".into())
    }

    /// Authenticate one original local Decision against its published group.
    /// Quorum signer subsets may differ; the entire immutable value may not.
    pub(crate) fn authorizes(
        &self,
        owner: &crate::state::NativeLaneStateOwner,
        instance: &crate::state::VerifiedLaneContext,
        original: &iroha_data_model::block::lane_consensus::LaneDecisionV1,
    ) -> Result<(), String> {
        let (published, published_subject) = self.published_instance(owner, instance)?;
        let auth = crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(instance);
        let local = auth
            .decision_certificate(original)
            .map_err(|error| error.to_string())?;
        if original.manifest != published.manifest {
            return Err(
                "Native application proof differs from the original immutable value".into(),
            );
        }
        if local.subject() != published_subject {
            return Err("Native application proof differs from the original subject".into());
        }
        Ok(())
    }

    /// Authorize terminal in-memory retirement for this exact original instance.
    /// Every actual fsynced or retained issued Decision must authenticate the published
    /// value, even before its reducer acknowledgement or body manifest exists.
    /// Earlier proposal, lock/vote and timeout intents need not name that value.
    pub(crate) fn authorizes_terminal<'qc, Decisions>(
        &self,
        owner: &crate::state::NativeLaneStateOwner,
        instance: &crate::state::VerifiedLaneContext,
        local_decisions: Decisions,
    ) -> Result<(), String>
    where
        Decisions: IntoIterator<Item = &'qc iroha_data_model::block::lane_consensus::LaneQcV1>,
    {
        let (published, published_subject) = self.published_instance(owner, instance)?;
        let auth = crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(instance);
        for original in local_decisions {
            // The actual published manifest supplies the complete value join.
            // This neither changes the original QC nor infers body readiness.
            let original = iroha_data_model::block::lane_consensus::LaneDecisionV1 {
                manifest: published.manifest,
                commit_qc: original.clone(),
            };
            let local = auth
                .decision_certificate(&original)
                .map_err(|error| error.to_string())?;
            if local.subject() != published_subject {
                return Err(
                    "Native terminal publication differs from a durable local Decision".into(),
                );
            }
        }
        Ok(())
    }
}

impl<A> PublishedCarrier<A> {
    /// Match the physical State family that consumed the original journals.
    pub(crate) fn matches_state(&self, state: &crate::state::State) -> bool {
        self.state_owner.matches_state(state)
    }

    /// Borrow the committed pipeline event emitted by the exact finality transition.
    pub(crate) fn committed_event(&self) -> &iroha_data_model::events::pipeline::BlockEvent {
        &self.committed_event
    }

    /// Borrow the exact durable receipt retained by the original checkpoint writer.
    pub(crate) fn receipt(&self) -> &crate::kura::KuraV2CommitReceipt {
        self.checkpoint.finality_receipt()
    }

    /// Borrow the authenticated finality artifact consumed by actual publication.
    pub(crate) fn artifact(
        &self,
    ) -> &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact {
        self.finality.artifact()
    }

    /// Borrow exact Native completion authority only from actual State publication.
    pub(crate) fn native_apply(&self) -> Option<PublishedNativeApply<'_>> {
        let source = self.source.native()?;
        source
            .retains_carrier(self.block(), source.context().context())
            .then_some(PublishedNativeApply {
                state_owner: &self.state_owner,
                block: self.block(),
                source,
            })
    }

    /// Borrow the exact result-bearing carrier that became visible.
    pub(crate) fn block(&self) -> &iroha_data_model::block::SignedBlock {
        self.block.as_ref()
    }

    /// Inspect events only after complete State publication and writer release.
    pub(crate) fn events(&self) -> &[EventBox] {
        &self.events
    }
}

impl<A> PhysicallyPreparedCarrier<'_, A> {
    /// Publish the original component journals once, then finish derived work
    /// outside their physical fences while retaining Apply serialization.
    /// No fallible/refusal branch exists after the first component is visible.
    pub(in crate::state::carrier_preparation::journals) fn publish(
        self,
    ) -> Result<
        PublishedCarrier<A>,
        (
            DecisionBoundCarrierJournals<A, DetachedCarrierComponents, KuraWsvCheckpointReceipt>,
            CarrierPublicationError,
        ),
    > {
        let mut publication_notice = self.target.state_view_publication();
        let mut this = self;
        let journals = &this.decision.journals;
        let error = if let Some(error) = journals
            .components
            ._fences
            ._queue
            .as_ref()
            .and_then(|queue| queue.ensure_available().err())
        {
            Some(CarrierPublicationError::QueueRetirement(error))
        } else if journals.effects.replay_prevalidation {
            Some(CarrierPublicationError::Prevalidation)
        } else if !journals
            .source_prefix
            .retains_carrier(journals.valid.as_ref(), &journals.context)
            || journals.effects.header != journals.valid.as_ref().header()
            || journals.staged_legacy_source()
        {
            Some(CarrierPublicationError::Source)
        } else if !journals
            .geometry
            .matches_publication_target(this.target, journals.effects.header)
            || journals.geometry.has_pending_lifecycle() != journals.effects.lifecycle.is_some()
        {
            Some(CarrierPublicationError::Geometry)
        } else if !journals.geometry.has_queue_custody(
            this.target,
            journals.effects.header,
            journals.components._fences._queue.as_ref(),
        ) {
            Some(CarrierPublicationError::QueueRetirementRequired)
        } else if !journals.native_amx_manifest.entries().is_empty() {
            Some(CarrierPublicationError::ParticipantDurability)
        } else {
            None
        };
        if let Some(error) = error {
            return Err((this.abort(), error));
        }

        // Only this terminal consumer may advance the exact storage operation,
        // after every source/retirement/participant refusal above. The held Kura
        // lease continues through State publication; a failed attempt returns
        // the same raw/tiered descriptors with all physical writers released.
        let update_da_mapping = match this.try_complete_geometry() {
            Ok(update) => update,
            Err(error) => {
                return Err((
                    this.abort(),
                    CarrierPublicationError::GeometryStorage(error),
                ));
            }
        };

        // Storage completion may take time while an independent writer latches
        // a sticky Queue recovery fault. Preserve the completed geometry owner
        // but refuse State visibility if that happened during this attempt.
        if let Some(error) = this
            .decision
            .journals
            .components
            ._fences
            ._queue
            .as_ref()
            .and_then(|queue| queue.ensure_available().err())
        {
            return Err((
                this.abort(),
                CarrierPublicationError::QueueRetirement(error),
            ));
        }

        // Reservations are declared before decomposition so even unwind drops
        // component writers/fences before returning their retained capacity.

        let admission;
        let Self { target, decision } = this;

        let DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
        } = decision;

        let PreparedCarrierJournals {
            valid,
            context: _context,
            execution_prefix: _execution_prefix,
            native_amx_manifest: _manifest,
            source_prefix,
            checkpoint: _state_hash,
            kura: _kura,
            components,
            world_effects,
            geometry: _geometry,
            provider_capture: _provider_capture,
            reputation_capture: _reputation_capture,
            mut publication_events,
            tiered_snapshot,
            mut effects,
            admission: retained_admission,
        } = journals;
        admission = retained_admission;
        let mut effect_cleanup;
        let hash_retirement;
        let membership_retirement;
        let world_retirement;
        let runtime_retirement;
        let AcquiredCarrierParticipants {
            world,
            runtime,
            transactions,
            block_hashes,
            effect_locks: original_effect_locks,
            _fences: fences,
        } = components.into_original();

        effect_cleanup = original_effect_locks;
        let mut effect_locks = effect_cleanup.physical_scope();
        let state_owner = block_hashes.state_owner();
        let generation = publication_notice.begin();
        membership_retirement = transactions.publish();
        runtime_retirement = runtime.publish();
        if update_da_mapping {
            effect_locks
                .da_shard_cursors
                .as_mut()
                .expect("prepared shard cursors")
                .sync_mapping(&effects.nexus.lane_config);
        }
        // Canonical resets precede this same carrier's DA observations.
        let mut lifecycle_post_publication = effects
            .lifecycle
            .take()
            .map(|effects| effects.publish(target, &mut effect_locks, &generation, true));
        let (_, mut extra_events, retirement, (), ()) = world.publish();
        world_retirement = retirement;
        world_effects.publish(
            target,
            effect_locks
                .da_pin_intents
                .as_mut()
                .expect("prepared pin cache"),
        );
        let mut da_post_publication = effects
            .da_commitments
            .take()
            .map(|effects| effects.publish(target, &mut effect_locks, &generation, true));
        effect_locks.install_sccp(std::sync::Arc::clone(&effects.sccp_registry));
        hash_retirement = block_hashes.publish();
        **effect_locks
            .latest_block_header
            .as_mut()
            .expect("prepared header") = Some(effects.header);
        drop(generation);
        if let Some(post) = da_post_publication.as_mut() {
            post.capture_snapshot(
                target,
                effect_locks
                    .da_shard_cursors
                    .as_ref()
                    .expect("prepared shard cursors"),
            );
        }
        if let Some(post) = lifecycle_post_publication.as_mut() {
            post.capture_snapshot(
                target,
                effect_locks
                    .da_shard_cursors
                    .as_ref()
                    .expect("prepared shard cursors"),
            );
        }
        effect_locks.release_writers();
        // Capture the final cursor projection under these same physical fences,
        // with the applying carrier's retained lane configuration.
        if let Some(post) = da_post_publication {
            post.publish(target);
        }
        if let Some(post) = lifecycle_post_publication {
            post.publish(target);
        }
        let commit = fences.release_for_completion();

        // Every authoritative component is now visible. Remaining operations
        // materialize derived indexes/persistence and cannot return a pre-write retry.
        effects.publish_observability(target);
        tiered_snapshot.publish(target, false);
        let height = effects.header.height().get();
        target.enforce_nexus_storage_budget(height);
        if effects.authenticated_replay_commit {
            target.set_query_index_status(height, Some(effects.header.hash()));
        } else {
            target.persist_query_index_status(height, Some(effects.header.hash()));
        }
        publication_events.append(&mut extra_events);
        drop(commit);
        if !effects.verified_lane_relay_records.is_empty() {
            target.hydrate_verified_lane_relay_records(effects.verified_lane_relay_records);
        }
        drop(membership_retirement);
        drop(runtime_retirement);
        drop(world_retirement);
        drop(hash_retirement);
        Ok(PublishedCarrier {
            finality,
            block: valid,
            committed_event,
            events: publication_events,
            checkpoint,
            state_owner,
            source: source_prefix,
            _admission: admission,
        })
    }
}

impl<A, Block, C> PreparedCarrierJournals<A, Block, C> {
    fn staged_legacy_source(&self) -> bool {
        self.effects.staged_merge_entry.is_some()
            || self
                .effects
                .canonical_wsv_merge_commit_authorization
                .is_some()
            || self
                .effects
                .canonical_carrier_commit_metadata_authorization
                .is_some()
    }
}

impl RetainedCarrierEffects {
    fn publish_observability(&self, target: &State) {
        if !self.authenticated_replay_commit {
            for slash in &self.pending_public_lane_slash_observability {
                crate::sumeragi::status::record_public_lane_bonded_delta(
                    slash.lane_id,
                    &slash.bonded_amount,
                    false,
                );
                if !slash.pending_unbond_amount.is_zero() {
                    crate::sumeragi::status::record_public_lane_pending_unbond_delta(
                        slash.lane_id,
                        &slash.pending_unbond_amount,
                        false,
                    );
                }
                crate::sumeragi::status::record_public_lane_slash(slash.lane_id);
                #[cfg(feature = "telemetry")]
                {
                    target.telemetry.record_public_lane_validator_status(
                        slash.lane_id,
                        Some(&slash.previous_status),
                        &slash.slashed_status,
                    );
                    target
                        .telemetry
                        .decrease_public_lane_bonded(slash.lane_id, &slash.bonded_amount);
                    if !slash.pending_unbond_amount.is_zero() {
                        target.telemetry.decrease_public_lane_pending_unbond(
                            slash.lane_id,
                            &slash.pending_unbond_amount,
                        );
                    }
                    target.telemetry.record_public_lane_slash(slash.lane_id);
                }
            }
            #[cfg(feature = "telemetry")]
            for (transition, no_result_kind) in &self.pending_parliament_telemetry_events {
                target
                    .telemetry
                    .record_committed_parliament_transition(*transition, *no_result_kind);
            }
        }
        #[cfg(feature = "telemetry")]
        {
            if let Some(counts) = self.committed_parliament_attempt_counts {
                let (statuses, stages) = counts.telemetry_counts();
                target
                    .telemetry
                    .set_parliament_attempt_counts(statuses, stages);
            }
            if let Some(total) = self.committed_citizens_total {
                target.telemetry.record_citizens_total(total);
            }
            target.telemetry.set_musubi_replication_shortfall_releases(
                self.committed_musubi_replication_shortfall_releases,
            );
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = target;
    }
}

#[cfg(test)]
#[path = "publication_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_publication_tests.rs"]
mod native_tests;
