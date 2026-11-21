//! Exact engine worklists and cursor scratch retain original World-pool charges.
//!
//! These owners fund only their actual fixed arrays. Native persistent map nodes,
//! replacement cursor graphs and callback codec controls remain separate owners.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
use iroha_allocation::{AllocatorRefusal, ChargedBuffer, ChargedBufferError};
use std::ops::Range;

pub(super) fn buffer_error(original: ChargedBufferError) -> ExecutionAttemptError<String> {
    let reason: ExecutionDeferred = match original {
        ChargedBufferError::Admission(original) => original.into(),
        original @ ChargedBufferError::Allocator { .. } => AllocatorRefusal::try_from(original)
            .expect("matched original scratch allocator refusal")
            .into(),
    };
    ExecutionAttemptError::Deferred(reason)
}

pub(super) struct StageEntry {
    pub(super) id: DefinitionId,
    pub(super) restriction: Option<RestrictionV1>,
}

pub(super) fn stage_census(
    world: &WorldBlock<'_>,
    height: u64,
) -> MigrationResult<ChargedBuffer<StageEntry>> {
    let selection = world.runtime_selection.get();
    let selected = || {
        world
            .runtime_migrations
            .iter()
            .filter(|(id, _)| selection.active.binary_search(id).is_ok())
    };
    // World acquisition checked this retained field against the original State
    // pool. This borrows its exact handle; no caller can supply substitute credit.
    let budget = world.sccp_committee_generations.allocation_budget();
    let mut entries = ChargedBuffer::new(selected().count(), budget).map_err(buffer_error)?;
    for (id, _) in selected() {
        entries.push_reserved(StageEntry {
            id: *id,
            restriction: crate::runtime::blocking_restriction(selection, id, height),
        });
    }
    Ok(entries)
}

pub(super) struct BarrierEntry {
    pub(super) id: DefinitionId,
    pub(super) status: MigrationStatusV1,
    pub(super) barrier_holds: bool,
    cursor: Range<usize>,
}

pub(super) struct BarrierCensus {
    entries: ChargedBuffer<BarrierEntry>,
    cursors: ChargedBuffer<u8>,
}
impl BarrierCensus {
    pub(super) fn entries(&self) -> &[BarrierEntry] {
        self.entries.as_slice()
    }
    pub(super) fn cursor(&self, entry: &BarrierEntry) -> &[u8] {
        &self.cursors.as_slice()[entry.cursor.clone()]
    }
}

pub(super) fn barrier_census(
    world: &WorldBlock<'_>,
    completed_through: Option<DefinitionId>,
) -> MigrationResult<BarrierCensus> {
    let selection = world.runtime_selection.get();
    let selected = || {
        world.runtime_migrations.iter().filter(|(id, _)| {
            completed_through.is_none_or(|done| **id > done)
                && selection.active.binary_search(id).is_ok()
        })
    };
    let mut count = 0_usize;
    let mut cursor_bytes = 0_usize;
    for (_, record) in selected() {
        count = count.checked_add(1).ok_or_else(|| {
            ExecutionAttemptError::Deferred(
                iroha_allocation::AllocationRefusal::DemandOverflow.into(),
            )
        })?;
        if record.barrier_holds() {
            cursor_bytes = cursor_bytes
                .checked_add(record.cursor.len())
                .ok_or_else(|| {
                    ExecutionAttemptError::Deferred(
                        iroha_allocation::AllocationRefusal::DemandOverflow.into(),
                    )
                })?;
        }
    }
    let budget = world.sccp_committee_generations.allocation_budget();
    let mut entries = ChargedBuffer::new(count, budget).map_err(buffer_error)?;
    let mut cursors = ChargedBuffer::new(cursor_bytes, budget).map_err(buffer_error)?;
    for (id, record) in selected() {
        let start = cursors.as_slice().len();
        let barrier_holds = record.barrier_holds();
        if barrier_holds {
            // The complete original cursor extent was checked before allocation;
            // pushes neither grow this backing nor invoke caller cloning code.
            for byte in &record.cursor {
                cursors.push_reserved(*byte);
            }
        }
        entries.push_reserved(BarrierEntry {
            id: *id,
            status: record.status,
            barrier_holds,
            cursor: start..cursors.as_slice().len(),
        });
    }
    Ok(BarrierCensus { entries, cursors })
}
