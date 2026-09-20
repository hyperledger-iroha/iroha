//! Consume the actual decided component owners within one State visibility cut.
//!
//! Namespace transitions and old participant evidence still require their real
//! storage owner; refusal returns the original detached decision before any write.
//! TODO: join pending geometry/retirement and aggregate production capacity to
//! this consumer before changing the production Validate/Apply handoff.

use super::super::super::{PreparedCarrierJournals, RetainedCarrierEffects};
use super::*;
use crate::{block::CommittedBlock, state::EventBox};

/// An unmet publication obligation, never invalidity of a decided proposal.
#[derive(Debug)]
pub(in crate::state::carrier_preparation::journals) enum CarrierPublicationError {
    /// The original prepared execution no longer matches its immutable carrier.
    Source,
    /// The retained transition still needs its real geometry/Queue storage owner.
    Geometry,
    /// Old participant evidence has not supplied complete durable application custody.
    ParticipantDurability,
    /// A prevalidation scratch owner cannot publish State.
    Prevalidation,
}

/// Actual published State and its original block, events, source and reservations.
/// This token has no second publication or reexecution operation.
pub(in crate::state::carrier_preparation::journals) struct PublishedCarrier<A, B, I> {
    block: CommittedBlock,
    committed_event: iroha_data_model::events::pipeline::BlockEvent,
    events: Vec<EventBox>,
    checkpoint: KuraWsvCheckpointReceipt,
    source: super::super::super::super::execution_prefix::ValidatedExecutionPrefix,
    // These outlive all values retained for completion delivery.
    _admission: A,
    _binding: B,
    _installation: I,
}

impl<A, B, I> PublishedCarrier<A, B, I> {
    /// Borrow the exact result-bearing carrier that became visible.
    pub(in crate::state::carrier_preparation::journals) fn block(
        &self,
    ) -> &iroha_data_model::block::SignedBlock {
        self.block.as_ref()
    }

    /// Inspect events only after complete State publication and writer release.
    pub(in crate::state::carrier_preparation::journals) fn events(&self) -> &[EventBox] {
        &self.events
    }
}

impl<A, B, I> PhysicallyPreparedCarrier<'_, A, B, I> {
    /// Publish the original component journals once, then finish derived work
    /// outside their physical fences while retaining Apply serialization.
    /// No fallible/refusal branch exists after the first component is visible.
    pub(in crate::state::carrier_preparation::journals) fn publish(
        self,
    ) -> Result<
        PublishedCarrier<A, B, I>,
        (
            DecisionBoundCarrierJournals<A, B, DetachedCarrierComponents, KuraWsvCheckpointReceipt>,
            CarrierPublicationError,
        ),
    > {
        let journals = &self.decision.journals;
        let error = if journals.effects.replay_prevalidation {
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
            .is_identity_transition(journals.effects.header)
            || journals.effects.pending_autoscale_lifecycle.is_some()
        {
            Some(CarrierPublicationError::Geometry)
        } else if !journals.native_amx_manifest.entries().is_empty() {
            Some(CarrierPublicationError::ParticipantDurability)
        } else {
            None
        };
        if let Some(error) = error {
            return Err((self.abort(), error));
        }

        // Reservations are declared before decomposition so even unwind drops
        // component writers/fences before returning their retained capacity.
        let installation;
        let binding;
        let admission;
        let Self {
            target,
            decision,
            installation: retained_installation,
        } = self;
        installation = retained_installation;
        let DecisionBoundCarrierJournals {
            checkpoint,
            finality: _finality,
            committed_event,
            journals,
            _binding_admission: retained_binding,
        } = decision;
        binding = retained_binding;
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
        let AcquiredCarrierComponents {
            world,
            runtime,
            transactions,
            block_hashes,
            _fences: fences,
        } = components;

        let generation = target.begin_state_view_write();
        transactions.publish();
        runtime.publish();
        let (_, mut extra_events, (), ()) = world.publish();
        world_effects.publish(target);
        let da_post_publication = effects
            .da_commitments
            .take()
            .map(|effects| effects.publish(target, &generation, true));
        target.install_sccp_registry_cache(std::sync::Arc::clone(&effects.sccp_registry));
        block_hashes.publish();
        target.update_latest_block_header_cache(effects.header);
        drop(generation);
        // Capture the final cursor projection under these same physical fences,
        // with the applying carrier's retained lane configuration.
        if let Some(post) = da_post_publication {
            post.publish(target);
        }
        let commit = fences.release_for_completion();

        // Every authoritative component is now visible. Remaining operations
        // materialize derived indexes/persistence and cannot return a pre-write retry.
        effects.publish_observability(target);
        if !effects.verified_lane_relay_records.is_empty() {
            target.hydrate_verified_lane_relay_records(effects.verified_lane_relay_records);
        }
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
        Ok(PublishedCarrier {
            block: valid,
            committed_event,
            events: publication_events,
            checkpoint,
            source: source_prefix,
            _admission: admission,
            _binding: binding,
            _installation: installation,
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
