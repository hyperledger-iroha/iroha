//! Consuming carrier preparation after a verified execution-prefix attachment.
//!
//! The owner retains the exact validated block, frozen context, actual output
//! seal, source inventory and witness while staging deterministic metadata. It
//! exposes no mutable State or publication operation. The existing commitment
//! remains the execution-prefix projection, not a complete prepared State root.
//! TODO: finish non-World/resource admission and consume the prepared journals
//! under exact QC and durable Kura/Native authorization. Keep the existing
//! execution commitment; complete ownership does not require a new wire root.

use super::{EventBox, StateBlock};
use crate::{
    block::{ValidBlock, valid::ValidatedCarrierPreparationInput},
    sumeragi::exec,
};
use iroha_data_model::block::{
    SignedBlock,
    consensus_v2::{ExecutionCommitment, HeightContext},
};
use std::sync::Arc;

mod execution_prefix;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: connect retained journals to the consuming State publisher"
    )
)]
mod journals;
pub(crate) use journals::PreparedCarrierJournals;
pub(crate) use journals::PublishedNativeApply;
pub(crate) use journals::RetainedCarrier;

/// A prepared candidate with all execution ownership retained and no Apply API.
pub(crate) struct PreparedCarrier<'state> {
    valid: ValidBlock,
    state: Box<StateBlock<'state>>,
    source_prefix: execution_prefix::ValidatedExecutionPrefix,
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
}

impl<'state> PreparedCarrier<'state> {
    /// Inspect the exact retained Native custody without source reconstruction.
    #[cfg(test)]
    pub(in crate::state) fn native_source_for_test(
        &self,
    ) -> Option<&super::NativeExecutionCustody> {
        self.source_prefix.native_for_test()
    }

    /// Exercise the unchanged raw publication refusal after preparation.
    #[cfg(test)]
    pub(in crate::state) fn into_state_for_test(self) -> Box<StateBlock<'state>> {
        self.state
    }

    /// Consume the exact validator output; errors drop every staged journal.
    pub(crate) fn prepare(
        input: ValidatedCarrierPreparationInput<'state>,
    ) -> Result<Self, (Box<SignedBlock>, String)> {
        execution_prefix::prepare(input)
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
