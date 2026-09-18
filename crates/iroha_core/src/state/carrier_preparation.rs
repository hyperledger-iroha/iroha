//! Consuming carrier preparation after a verified execution-prefix attachment.
//!
//! The owner retains the exact validated block, frozen context, actual output
//! seal, source inventory and witness while staging deterministic metadata. It
//! exposes no mutable State or publication operation. The existing commitment
//! remains the execution-prefix projection, not a complete prepared State root.
//! TODO: finish non-World/resource admission and consume the prepared journals
//! under exact QC and durable Kura/Native authorization. Keep the existing
//! execution commitment; complete ownership does not require a new wire root.

use super::{ApplyTopologyAuthority, EventBox, StateBlock};
use crate::{
    block::{ValidBlock, valid::ValidatedCarrierPreparationInput},
    sumeragi::exec,
};
use iroha_data_model::block::{
    SignedBlock,
    consensus_v2::{ExecutionCommitment, HeightContext},
};
use std::sync::Arc;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: connect retained journals to the consuming State publisher"
    )
)]
mod journals;

/// A prepared candidate with all execution ownership retained and no Apply API.
pub(crate) struct PreparedCarrier<'state> {
    valid: ValidBlock,
    state: Box<StateBlock<'state>>,
    // Retain the authenticated context used by execution and topology selection.
    // The next complete-State owner must consume this same object.
    context: Arc<HeightContext>,
    execution_prefix: ExecutionCommitment,
    native_amx_manifest: exec::NativeAmxApplicationManifestV1,
    // Retain the actual deferred cache records; future publication must consume
    // these, never re-prepare an overlay whose pin indexes are already staged.
    _world_effects: super::world_commit::PreparedWorldEffects,
    // Event admission happens before voting. Delivery requires consuming the
    // final durable publication owner; dropping a candidate drops its events.
    _publication_events: Vec<EventBox>,
    _tiered_snapshot: super::tiered_publication::PreparedTieredSnapshot,
}

impl<'state> PreparedCarrier<'state> {
    /// Consume the exact validator output; errors drop every staged journal.
    pub(crate) fn prepare(
        input: ValidatedCarrierPreparationInput<'state>,
    ) -> Result<Self, (Box<SignedBlock>, String)> {
        let (valid, mut state, context) = input.into_parts();
        let result = (|| {
            let block = valid.as_ref();
            // Authenticate the completed prefix before the permitted metadata
            // tail changes World. Rechecking that prefix delta afterward would
            // reject precisely the deterministic writes this owner now stages.
            state.verify_execution_output_seal(block)?;
            let inventory = state.verified_fastpq_source_inventory_for_capture()?;
            state.verify_cached_ordinary_witness_content(&inventory)?;
            let witness = state
                .exec_witness
                .as_ref()
                .ok_or("carrier preparation requires its retained execution witness")?;
            let native_amx_manifest =
                exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
                    block,
                    state.staged_merge_entry(),
                )?;
            let lanes = exec::LaneFinalityManifestV1::from_result_bearing_block(block)?;
            let execution_prefix = exec::execution_commitment_from_validated_block(
                witness,
                &native_amx_manifest,
                &lanes,
                block,
            )
            .map_err(str::to_owned)?;
            state
                .prepare_deterministic_carrier_metadata(
                    block,
                    context
                        .roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect(),
                    ApplyTopologyAuthority::V2Finality,
                )
                .map_err(|error| error.to_string())?;
            let world_effects = state.prepare_carrier_world_effects()?;
            let publication_events = state
                .prepare_carrier_publication_events(block.header())
                .map_err(|error| error.to_string())?;
            let tiered_snapshot = super::tiered_publication::PreparedTieredSnapshot::prepare(
                &state.world,
                &state.state_ref.tiered_snapshot_worker,
            );
            Ok((
                execution_prefix,
                native_amx_manifest,
                world_effects,
                publication_events,
                tiered_snapshot,
            ))
        })();
        match result {
            Ok((
                execution_prefix,
                native_amx_manifest,
                world_effects,
                publication_events,
                tiered_snapshot,
            )) => Ok(Self {
                valid,
                state,
                context,
                execution_prefix,
                native_amx_manifest,
                _world_effects: world_effects,
                _publication_events: publication_events,
                _tiered_snapshot: tiered_snapshot,
            }),
            Err(error) => {
                // No partially prepared State escapes, even when an error
                // follows membership, hash-log or World journal staging.
                drop(state);
                Err((Box::new(valid.into()), error))
            }
        }
    }

    /// Borrow the immutable candidate whose exact wire was sealed by execution.
    pub(crate) fn block(&self) -> &SignedBlock {
        self.valid.as_ref()
    }

    /// Inspect staged admission inputs without allowing mutation or publication.
    pub(crate) fn state(&self) -> &StateBlock<'state> {
        &self.state
    }

    /// Borrow the context retained from the actual candidate validator.
    pub(crate) fn context(&self) -> &HeightContext {
        &self.context
    }

    /// Return the unchanged execution-prefix commitment after metadata admission.
    pub(crate) fn execution_prefix_commitment(&self) -> ExecutionCommitment {
        self.execution_prefix
    }

    /// Borrow the exact manifest derived before deterministic metadata staging.
    pub(crate) fn native_amx_manifest(&self) -> &exec::NativeAmxApplicationManifestV1 {
        &self.native_amx_manifest
    }
}

#[cfg(test)]
#[path = "carrier_preparation_tests.rs"]
mod tests;
