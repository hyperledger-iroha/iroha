//! Owned runtime journals admitted together before their original writers release.
//!
//! The caller supplies the actual retained-memory and installation reservation.
//! No encoded-length estimate or permissive production admission is provided.

use super::*;

#[path = "runtime_capture.rs"]
mod capture;
pub(super) use capture::RuntimeCapture;

#[path = "runtime_publication.rs"]
mod publication;
pub(super) use publication::{
    PreparedRuntimeJournals, RuntimePublicationError, RuntimePublicationSlot,
};

#[cfg(test)]
#[path = "runtime_publication_tests.rs"]
mod publication_tests;

/// Read-only original inputs for one complete runtime-journal admission.
pub(crate) struct RuntimeJournalInputs<'capture, 'state> {
    canonical_runtime: &'capture CellBlock<'state, SnapshotNexusRuntime>,
    commit_topology: &'capture CellBlock<'state, Vec<PeerId>>,
    prev_commit_topology: &'capture CellBlock<'state, Vec<PeerId>>,
    lane_consensus_contexts: &'capture CellBlock<'state, LaneConsensusContextsV1>,
}

impl<'state> RuntimeJournalInputs<'_, 'state> {
    /// Original runtime policy journal, including its actual undo and touches.
    pub(crate) fn canonical_runtime(&self) -> &CellBlock<'state, SnapshotNexusRuntime> {
        self.canonical_runtime
    }

    /// Original current committee journal.
    pub(crate) fn commit_topology(&self) -> &CellBlock<'state, Vec<PeerId>> {
        self.commit_topology
    }

    /// Original previous committee journal.
    pub(crate) fn prev_commit_topology(&self) -> &CellBlock<'state, Vec<PeerId>> {
        self.prev_commit_topology
    }

    /// Original frozen lane-context journal.
    pub(crate) fn lane_consensus_contexts(&self) -> &CellBlock<'state, LaneConsensusContextsV1> {
        self.lane_consensus_contexts
    }
}

/// Exact original runtime deltas, with no State or Cell borrow.
/// Publication preparation requires reacquiring all four original owners.
pub(super) struct RuntimeJournals<Admission> {
    pub(super) canonical_runtime: mv::cell::Detached<SnapshotNexusRuntime, ()>,
    pub(super) commit_topology: mv::cell::Detached<Vec<PeerId>, ()>,
    pub(super) prev_commit_topology: mv::cell::Detached<Vec<PeerId>, ()>,
    pub(super) lane_consensus_contexts: mv::cell::Detached<LaneConsensusContextsV1, ()>,
    // Rust drops fields in declaration order: release accounting after values.
    admission: Admission,
}

impl<Admission> RuntimeJournals<Admission> {
    /// Admit retention before detaching the original current/undo allocations.
    /// Payloads move without cloning; their earlier execution allocations need
    /// admission before block acquisition and mutation, not at this boundary.
    /// Refusal drops every original writer; successful custody retains the guard.
    pub(super) fn capture<'state, E>(
        canonical_runtime: CellBlock<'state, SnapshotNexusRuntime>,
        commit_topology: CellBlock<'state, Vec<PeerId>>,
        prev_commit_topology: CellBlock<'state, Vec<PeerId>>,
        lane_consensus_contexts: CellBlock<'state, LaneConsensusContextsV1>,
        admit: impl FnOnce(RuntimeJournalInputs<'_, 'state>) -> Result<Admission, E>,
    ) -> Result<Self, E> {
        let mut pending = RuntimeCapture::new(
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
        );
        pending.try_capture(admit)?;
        Ok(pending.into_journals())
    }

    /// Momentarily inspect all original identities; this grants no publication lease.
    pub(super) fn matches_current(&self, state: &State) -> bool {
        self.canonical_runtime
            .matches_current(&state.canonical_runtime)
            && self.commit_topology.matches_current(&state.commit_topology)
            && self
                .prev_commit_topology
                .matches_current(&state.prev_commit_topology)
            && self
                .lane_consensus_contexts
                .matches_current(&state.lane_consensus_contexts)
    }

    /// Borrow the caller's retained reservation without releasing its ownership.
    pub(super) fn admission(&self) -> &Admission {
        &self.admission
    }
}
