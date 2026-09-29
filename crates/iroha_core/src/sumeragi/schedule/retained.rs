//! Immutable funded runtime custody of the canonical schedule graph.
//!
//! Cell current/undo clones share the same original graph allocation. No normal decoder can
//! manufacture this owner: snapshot values remain canonical DTOs until explicit original-pool
//! admission. EBR generations and their publication controls are separate resource obligations.

use super::{ConsensusSchedule, ScheduleError};
use mv::allocation::{
    AllocationBudget, AllocationReservation, ChargedShared, PrepaidSharedError, RetainedPayload,
};
use norito::{
    core::{Encoder, SerializePayload},
    json::{self, JsonSerialize},
};
use std::{alloc::Layout, fmt, ops::Deref};

/// The only heap-bearing World schedule owner. Empty pregenesis state has no allocation.
#[derive(Clone, Default)]
pub struct RetainedConsensusSchedule {
    owner: Option<ChargedShared<RetainedPayload<ConsensusSchedule>>>,
}
impl RetainedConsensusSchedule {
    /// Exact shell layout, distinct from every nested allocation in the canonical graph.
    pub(crate) fn shared_layout() -> Layout {
        ChargedShared::<RetainedPayload<ConsensusSchedule>>::allocation_layout()
    }
    /// Wrap the exact admitted graph without copying it or reacquiring pool capacity.
    pub(crate) fn from_retained(
        original: RetainedPayload<ConsensusSchedule>,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, (RetainedPayload<ConsensusSchedule>, PrepaidSharedError)> {
        ChargedShared::from_reservation(original, reservation)
            .map(|owner| Self { owner: Some(owner) })
    }
    /// Admit a genuinely separate World graph from the original configured execution pool.
    /// All deep allocations plus the shared shell are reserved before materialization.
    pub(crate) fn admit(
        source: &ConsensusSchedule,
        budget: &AllocationBudget,
    ) -> Result<Self, ScheduleError> {
        crate::sumeragi::epoch_election::retain_schedule(source, budget).map_err(Into::into)
    }
    /// Read the canonical value; callers cannot mutate, extract or replace its allocations.
    pub(crate) fn canonical(&self) -> &ConsensusSchedule {
        static EMPTY: ConsensusSchedule = ConsensusSchedule::empty();
        self.owner.as_ref().map_or(&EMPTY, |owner| owner.get())
    }
}
impl Deref for RetainedConsensusSchedule {
    type Target = ConsensusSchedule;
    fn deref(&self) -> &Self::Target {
        self.canonical()
    }
}
impl fmt::Debug for RetainedConsensusSchedule {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.canonical().fmt(out)
    }
}
impl PartialEq for RetainedConsensusSchedule {
    fn eq(&self, other: &Self) -> bool {
        self.canonical() == other.canonical()
    }
}
impl Eq for RetainedConsensusSchedule {}
impl norito::NoritoSchema for RetainedConsensusSchedule {
    fn nominal_name() -> String {
        <ConsensusSchedule as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <ConsensusSchedule as norito::NoritoSchema>::frame_name()
    }
}
impl SerializePayload for RetainedConsensusSchedule {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.canonical().serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.canonical().encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.canonical().encoded_len_exact()
    }
}
impl JsonSerialize for RetainedConsensusSchedule {
    fn json_serialize(&self, out: &mut String) {
        self.canonical().json_serialize(out)
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.canonical().json_serialize_to(out)
    }
}
