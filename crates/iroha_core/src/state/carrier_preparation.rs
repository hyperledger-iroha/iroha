//! Consuming carrier preparation after a verified execution-prefix attachment.
//!
//! The owner retains the exact validated block, frozen context, actual output
//! seal, source inventory and witness while staging deterministic metadata. It
//! exposes no mutable State or publication operation. The existing commitment
//! remains the execution-prefix projection, not a complete prepared State root.
//! Publication consumes these journals under exact finality and original
//! State/Queue/Kura custody. The execution commitment remains unchanged.
//! TODO: extend concrete shell admission to aggregate nested payload accounting.

use super::{DataSpaceId, EventBox, Hash, LaneId, LaneLifecycleError, StateBlock};
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
pub(super) mod queue_retirement;
pub(crate) use journals::{
    CarrierArchivePreparationError, CarrierJournalPreparationError, RetainedCarrier,
};
pub(crate) use journals::{PublishedCarrier, PublishedNativeApply};

/// A prepared candidate with all execution ownership retained and no Apply API.
pub(crate) struct PreparedCarrier<'state> {
    parts: Option<PreparedCarrierFields<'state>>,
}

/// Exact candidate fields; the enclosing owner retires every writer first.
pub(crate) struct PreparedCarrierFields<'state> {
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

impl<'state> std::ops::Deref for PreparedCarrier<'state> {
    type Target = PreparedCarrierFields<'state>;

    fn deref(&self) -> &Self::Target {
        self.parts.as_ref().expect("original prepared carrier")
    }
}

impl Drop for PreparedCarrier<'_> {
    fn drop(&mut self) {
        use mv::BlockRetirement as _;
        if let Some(parts) = self.parts.as_mut() {
            parts.state.release_writers();
        }
    }
}

impl<'state> PreparedCarrier<'state> {
    fn new(parts: PreparedCarrierFields<'state>) -> Self {
        Self { parts: Some(parts) }
    }

    fn parts_mut(&mut self) -> &mut PreparedCarrierFields<'state> {
        self.parts.as_mut().expect("original prepared carrier")
    }

    fn into_parts(mut self) -> PreparedCarrierFields<'state> {
        self.parts.take().expect("original prepared carrier")
    }

    /// Plan World journal wrapper allocations before acquiring execution writers.
    /// These capture/installation shells coexist through retry. Their checked
    /// requested bytes are only one part of complete candidate admission; MV
    /// payloads, runtime, archives and publication resources remain separate.
    pub(crate) fn world_journal_shell_bytes() -> Result<usize, mv::allocation::AllocationRefusal> {
        super::world_journals::resources::WorldJournalShellDemand::plan()
            .map(|demand| demand.total_bytes())
    }

    /// Exact inline allocation for the deferred effects owner captured by journals.
    /// Nested payloads use their existing standard allocations and are excluded.
    pub(crate) fn retained_effects_layout() -> std::alloc::Layout {
        std::alloc::Layout::new::<journals::RetainedCarrierEffects>()
    }

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
        self.into_parts().state
    }

    /// Consume the exact validator output; errors drop every staged journal.
    pub(crate) fn prepare(
        input: ValidatedCarrierPreparationInput<'state>,
    ) -> Result<Self, (Box<SignedBlock>, super::MergeLedgerCommitError)> {
        execution_prefix::prepare(input)
    }

    /// Borrow the immutable candidate whose exact wire was sealed by execution.
    #[cfg(test)]
    pub(crate) fn block(&self) -> &SignedBlock {
        self.valid.as_ref()
    }

    /// Inspect staged admission inputs without allowing mutation or publication.
    #[cfg(test)]
    pub(crate) fn state(&self) -> &StateBlock<'state> {
        &self.state
    }

    /// Observe the exact pending/prospective retirement without exposing mutable State.
    /// Any actual reader releases remain in this original candidate's State owner.
    pub(crate) fn autoscale_retirement_binding(
        &mut self,
    ) -> Result<Option<(LaneId, DataSpaceId, Hash)>, LaneLifecycleError> {
        let parts = self.parts_mut();
        if let Some(binding) = parts.state.pending_autoscale_retirement_binding()? {
            return Ok(Some(binding));
        }
        parts
            .state
            .prospective_autoscale_retirement_binding(parts.valid.as_ref())
    }

    /// Borrow the context retained from the actual candidate validator.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "TODO: wire native state publication")
    )]
    pub(crate) fn context(&self) -> &HeightContext {
        &self.context
    }

    /// Return the unchanged execution-prefix commitment after metadata admission.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "TODO: wire native state publication")
    )]
    pub(crate) fn execution_prefix_commitment(&self) -> ExecutionCommitment {
        self.execution_prefix
    }

    /// Borrow the exact manifest derived before deterministic metadata staging.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "TODO: wire native state publication")
    )]
    pub(crate) fn native_amx_manifest(&self) -> &exec::NativeAmxApplicationManifestV1 {
        &self.native_amx_manifest
    }
}

#[cfg(test)]
#[path = "carrier_preparation_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use journals::publish_governance_fixture;

#[cfg(test)]
pub(in crate::state) use journals::archive_fixture_instructions;
