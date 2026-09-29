//! Exact Musubi universal resolver and directory checks for one World cut.
//!
//! The row's index revision is independent authority. Every other row field
//! is reconstructed from its canonical package, release, archive, and archive
//! availability sources before a semantic authority projection is admitted.

use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use mv::allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};

#[path = "deserialize_world_musubi_universal/accumulator.rs"]
mod accumulator;
use accumulator::PackageRevision;

fn invalid(
    cut: ProjectionCut,
    table: ProjectionTable,
    reason: &'static str,
) -> ProjectionRejection {
    ProjectionRejection::new(table, reason).with_cut(cut)
}

/// Verify exact row content and membership against authoritative source tables.
///
/// This is read-only so both finalized restore and World publication can use the
/// same predicate. A failed check cannot mutate the candidate or its predecessor.
pub(in crate::state) fn validate_musubi_universal_projection_cut(
    world: &impl WorldReadOnly,
    cut: ProjectionCut,
    execution_budget: &AllocationBudget,
) -> Result<(), ExecutionAttemptError<ProjectionRejection>> {
    const RESOLVER: ProjectionTable = ProjectionTable::ResolverIndex;
    const DIRECTORY: ProjectionTable = ProjectionTable::PublicDirectory;
    let packages = world.musubi_packages();
    let releases = world.musubi_releases();
    let archives = world.musubi_archives();
    let availability = world.musubi_archive_availability();
    let resolver = world.musubi_resolver_index();
    let directory = world.musubi_public_directory();
    let current_revision = world.musubi_resolver_index_revision();
    for (package_id, package) in packages.iter() {
        package
            .validate()
            .map_err(|error| invalid(cut, DIRECTORY, error.reason()))?;
        if package_id != &package.package {
            return Err(invalid(
                cut,
                DIRECTORY,
                "package lookup key differs from its canonical identity",
            )
            .into());
        }
    }
    // Validate all package identities before scratch admission, preserving the
    // semantic error order. Source keys and versions stay borrowed from this cut.
    let mut package_revisions =
        ChargedBuffer::new(packages.len(), execution_budget).map_err(|error| {
            ExecutionAttemptError::Deferred(match error {
                ChargedBufferError::Admission(refusal) => refusal.into(),
                ChargedBufferError::Allocator { .. } => {
                    ivm::error::ExecutionDeferral::AllocationUnavailable.into()
                }
            })
        })?;
    // MV storage iterates in canonical key order, so this array supports borrowed
    // binary searches without copying package identities or sorting another index.
    for (package_id, _) in packages.iter() {
        package_revisions.push_reserved(PackageRevision::new(package_id));
    }
    for (release_id, row) in resolver.iter() {
        row.validate()
            .map_err(|error| invalid(cut, RESOLVER, error.reason()))?;
        let release = releases
            .get(release_id)
            .ok_or_else(|| invalid(cut, RESOLVER, "resolver row references a missing release"))?;
        release
            .validate()
            .map_err(|error| invalid(cut, RESOLVER, error.reason()))?;
        if release_id != &release.manifest.release || packages.get(&release_id.package).is_none() {
            return Err(invalid(
                cut,
                RESOLVER,
                "resolver row's release identity or package source is missing",
            )
            .into());
        }
        let archive = archives
            .get(&release.manifest.archive_id)
            .ok_or_else(|| invalid(cut, RESOLVER, "resolver row references a missing archive"))?;
        let storage = availability
            .get(&release.manifest.archive_id)
            .ok_or_else(|| {
                invalid(
                    cut,
                    RESOLVER,
                    "resolver row is missing its authoritative storage projection",
                )
            })?;
        if release_id != &row.release
            || row.release_digest != release.release_digest
            || row.archive_id != release.manifest.archive_id
            || row.source_digest != archive.commitment.source_tree_digest
            || row.interface_digest != release.manifest.interface_digest
            || row.abi != release.manifest.abi
            || row.dependencies.as_slice() != release.manifest.dependencies.as_slice()
            || row.selection.yank != release.yank
            || row.selection.governance != release.artifact_governance
            || row.selection.storage != *storage
            || row.index_revision > current_revision
        {
            return Err(invalid(
                cut,
                RESOLVER,
                "resolver row diverges from authoritative release/archive projections",
            )
            .into());
        }
        let index = package_revisions
            .as_slice()
            .binary_search_by(|entry| entry.package.cmp(&release_id.package))
            .map_err(|_| invalid(cut, RESOLVER, "resolver row references a missing package"))?;
        package_revisions.as_mut_slice()[index].observe(release_id, row);
    }
    for (release_id, _) in releases.iter() {
        if resolver.get(release_id).is_none() {
            return Err(invalid(cut, RESOLVER, "release is missing its exact resolver row").into());
        }
    }
    for (selector, entry) in directory.iter() {
        entry
            .validate()
            .map_err(|error| invalid(cut, DIRECTORY, error.reason()))?;
        let package = packages.get(&entry.package).ok_or_else(|| {
            invalid(
                cut,
                DIRECTORY,
                "directory entry references a missing package",
            )
        })?;
        let index = package_revisions
            .as_slice()
            .binary_search_by(|revision| revision.package.cmp(&entry.package))
            .expect("validated directory package has a latest-version accumulator");
        let revision = &mut package_revisions.as_mut_slice()[index];
        if selector != &entry.selector
            || entry.selector.namespace != package.claimed_namespace
            || entry.selector.name != entry.package.name
            || entry.metadata_revision != package.revisions.metadata
            || entry.latest_selectable.as_ref() != revision.latest
            || entry.index_revision > current_revision
        {
            return Err(invalid(
                cut,
                DIRECTORY,
                "directory entry diverges from its package and resolver rows",
            )
            .into());
        }
        if let Some(maximum_row_revision) = revision.maximum {
            if entry.index_revision < maximum_row_revision {
                return Err(invalid(
                    cut,
                    DIRECTORY,
                    "directory entry predates its package resolver rows",
                )
                .into());
            }
        }
        // A validated row has exactly this package's canonical selector. Marking
        // its borrowed accumulator avoids allocating a selector for the final
        // completeness pass, including when two package IDs share a selector.
        revision.directory_present = true;
    }
    for revision in package_revisions.as_slice() {
        if !revision.directory_present {
            return Err(invalid(
                cut,
                DIRECTORY,
                "package is missing its exact public-directory entry",
            )
            .into());
        }
    }
    Ok(())
}

/// Check both the current and rollback-visible versions retained by one World.
pub(super) fn validate_musubi_universal_projection_cuts(
    world: &World,
    execution_budget: &AllocationBudget,
) -> Result<(), StateRestoreError> {
    validate_musubi_universal_projection_cut(
        &world.view(),
        ProjectionCut::Current,
        execution_budget,
    )
    .map_err(|error| error.map_rejection(ProjectionRejection::into_json))?;
    validate_musubi_universal_projection_cut(
        &world.try_block_and_revert(execution_budget)?,
        ProjectionCut::Predecessor,
        execution_budget,
    )
    .map_err(|error| error.map_rejection(ProjectionRejection::into_json))
    .map_err(Into::into)
}

#[cfg(test)]
#[path = "deserialize_world_musubi_universal/funding_tests.rs"]
mod funding_tests;
