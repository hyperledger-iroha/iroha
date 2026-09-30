//! Same-cut directory revision checks with exact, caller-funded reference scratch.
//!
//! Directory keys order namespace/name, while resolver keys order package/version.
//! Sorting only borrowed payload references makes their package merge exact even
//! for malformed duplicate directory payloads; it never introduces a second row
//! authority. All prior semantic checks run before scratch admission.

use super::*;
use iroha_allocation::{ChargedBuffer, ChargedBufferError};

pub(super) fn validate_directory_revisions(
    world: &impl WorldReadOnly,
    execution_budget: &AllocationBudget,
) -> Result<(), ExecutionAttemptError<ProjectionRejection>> {
    let directory = world.musubi_public_directory();
    let mut entries = ChargedBuffer::new(directory.len(), execution_budget).map_err(|error| {
        ExecutionAttemptError::Deferred(match error {
            ChargedBufferError::Admission(refusal) => refusal.into(),
            ChargedBufferError::Allocator { .. } => {
                ivm::error::ExecutionDeferral::AllocationUnavailable.into()
            }
        })
    })?;
    for (_, entry) in directory.iter() {
        entries.push_reserved(entry);
    }
    entries
        .as_mut_slice()
        .sort_unstable_by(|left, right| left.package.cmp(&right.package));
    let mut entries = entries.as_slice().iter().copied().peekable();
    let mut rows = world.musubi_resolver_index().iter().peekable();
    while let Some(entry) = entries.next() {
        while rows
            .next_if(|(release, _)| release.package < entry.package)
            .is_some()
        {}
        let mut maximum = None::<u64>;
        while let Some((_, row)) = rows.next_if(|(release, _)| release.package == entry.package) {
            maximum =
                Some(maximum.map_or(row.index_revision, |value| value.max(row.index_revision)));
        }
        let stale = |entry: &iroha_data_model::musubi::MusubiOrderedPackageEntryV1| {
            maximum.is_some_and(|revision| entry.index_revision < revision)
        };
        if stale(entry) {
            return Err(ProjectionRejection::new(
                ProjectionTable::PublicDirectory,
                "directory entry predates its package resolver rows",
            )
            .into());
        }
        // Duplicate payload package identities are invalid elsewhere, but this
        // validator must preserve its own prior result without rescanning rows.
        while let Some(duplicate) = entries.next_if(|other| other.package == entry.package) {
            if stale(duplicate) {
                return Err(ProjectionRejection::new(
                    ProjectionTable::PublicDirectory,
                    "directory entry predates its package resolver rows",
                )
                .into());
            }
        }
    }
    Ok(())
}
