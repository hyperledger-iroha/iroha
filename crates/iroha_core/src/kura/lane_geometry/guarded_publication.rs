//! One journal mutation implementation under the original four Kura fences.
//!
//! Canonical recovery and capacity reads run before geometry/sidecar acquisition;
//! pending archive GC runs before sidecar acquisition. The existing entry points
//! transfer those exact guards and observations here. Neither durable phase calls
//! a public Kura locking wrapper or releases the publication boundary.
//!
//! TODO: connect this guarded owner to the complete carrier geometry preparation,
//! with admitted prelude work, authenticated drain-history custody and Queue/State
//! ownership. These storage primitives alone never authorize State publication.

use super::*;
use crate::kura::publication_lease::KuraPublicationLease;

/// Original transition inputs and journal, observed before the final sidecar fence.
pub(super) struct PreparedLaneGeometryTransition<'input> {
    pub(super) previous: &'input LaneConfig,
    pub(super) updated: &'input LaneConfig,
    pub(super) previous_incarnations: &'input BTreeMap<LaneId, Hash>,
    pub(super) updated_incarnations: &'input BTreeMap<LaneId, Hash>,
    pub(super) previous_activation_heights: &'input BTreeMap<LaneId, u64>,
    pub(super) updated_activation_heights: &'input BTreeMap<LaneId, u64>,
    pub(super) previous_lineage_root: Hash,
    pub(super) updated_lineage_root: Hash,
    pub(super) replaced_lane_ids: &'input BTreeSet<LaneId>,
    pub(super) certified_retirements: BTreeSet<LaneRetirementIdentity>,
    pub(super) transition_height: Option<u64>,
    pub(super) namespace_receipts: Option<&'input mut Vec<StartupReplayNamespaceCreation>>,
    pub(super) pending_canonical_bytes: u64,
    pub(super) previous_bindings: Vec<LaneGeometryBinding>,
    pub(super) updated_bindings: Vec<LaneGeometryBinding>,
    pub(super) previous_catalog: Hash,
    pub(super) updated_catalog: Hash,
    pub(super) journal_was_present: bool,
    pub(super) journal: LaneGeometryJournal,
}

/// Exact catalog and original GC-reconciled journal to publish under held fences.
pub(super) struct PreparedLaneGeometryCatalog {
    pub(super) bindings: Vec<LaneGeometryBinding>,
    pub(super) fingerprint: Hash,
    pub(super) lineage_root: Hash,
    pub(super) configured_baseline: Option<Hash>,
    pub(super) journal: LaneGeometryJournal,
}

impl KuraPublicationLease<'_> {
    /// Apply one exact transition without reacquiring any publication fence.
    pub(in crate::kura::lane_geometry) fn apply_prepared_lane_geometry(
        &self,
        prepared: PreparedLaneGeometryTransition<'_>,
    ) -> Result<()> {
        let kura = self.kura_under_publication_guards();
        let PreparedLaneGeometryTransition {
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            previous_lineage_root,
            updated_lineage_root,
            replaced_lane_ids,
            certified_retirements,
            transition_height,
            mut namespace_receipts,
            pending_canonical_bytes,
            previous_bindings,
            updated_bindings,
            previous_catalog,
            updated_catalog,
            journal_was_present,
            mut journal,
        } = prepared;
        let current_applied_count = journal
            .records
            .iter()
            .position(|record| record.phase == LaneGeometryPhase::RolledBack)
            .unwrap_or(journal.records.len());
        let uncertain_index = journal.records.iter().position(|record| {
            matches!(
                record.phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied
            )
        });
        let requested_transition_height = transition_height;
        let record_matches = |index: usize, height: Option<u64>| {
            journal.records.get(index).is_some_and(|record| {
                height.is_none_or(|height| record.transition_height == height)
                    && record.previous_catalog == previous_catalog
                    && record.previous_lineage_root == previous_lineage_root
                    && record.updated_catalog == updated_catalog
                    && record.updated_lineage_root == updated_lineage_root
            })
        };
        let frontier_retry = uncertain_index
            .filter(|index| record_matches(*index, requested_transition_height))
            .or_else(|| {
                (current_applied_count < journal.records.len()
                    && record_matches(current_applied_count, requested_transition_height))
                .then_some(current_applied_count)
            });
        let published_retry = current_applied_count.checked_sub(1).filter(|index| {
            let record = &journal.records[*index];
            record.phase == LaneGeometryPhase::CatalogPublished
                && record_matches(*index, requested_transition_height)
        });
        let retained_retry = frontier_retry.or(published_retry).or_else(|| {
            let mut matches = journal
                .records
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    (requested_transition_height.is_some_and(|height| {
                        record.transition_height == height
                            && record.previous_catalog == previous_catalog
                            && record.previous_lineage_root == previous_lineage_root
                            && record.updated_catalog == updated_catalog
                            && record.updated_lineage_root == updated_lineage_root
                    }))
                    .then_some(index)
                });
            let candidate = matches.next()?;
            matches.next().is_none().then_some(candidate)
        });
        let transition_height = match requested_transition_height {
            Some(height) => height,
            None => {
                if let Some(index) = retained_retry {
                    journal.records[index].transition_height
                } else if let Some(last) = journal.records.last() {
                    last.transition_height.checked_add(1).ok_or_else(|| {
                        kura.geometry_error(
                            ErrorKind::InvalidData,
                            "lane geometry transition height overflow",
                        )
                    })?
                } else if let Some(checkpoint) = journal.checkpoint.as_ref() {
                    checkpoint.snapshot_height.checked_add(1).ok_or_else(|| {
                        kura.geometry_error(
                            ErrorKind::InvalidData,
                            "lane geometry transition height overflow after checkpoint",
                        )
                    })?
                } else {
                    0
                }
            }
        };
        let existing_index = retained_retry
            .filter(|index| journal.records[*index].transition_height == transition_height);
        if previous_catalog == updated_catalog
            && previous_lineage_root == updated_lineage_root
            && existing_index.is_none()
        {
            if requested_transition_height.is_none() {
                kura.reconcile_lane_geometry_history(
                    &mut journal,
                    previous_catalog,
                    previous_lineage_root,
                )?;
            } else {
                kura.reconcile_lane_geometry_history_to_count(
                    &mut journal,
                    previous_catalog,
                    previous_lineage_root,
                    current_applied_count,
                )?;
            }
            kura.ensure_authoritative_lane_markers_with_receipts(
                previous,
                previous_incarnations,
                previous_activation_heights,
                namespace_receipts.as_deref_mut(),
            )?;
            *kura.lane_storage_entries.lock() = kura.lane_storage_entries_from_geometry(
                updated,
                updated_incarnations,
                updated_activation_heights,
            )?;
            return if journal_was_present || journal != LaneGeometryJournal::default() {
                kura.write_lane_geometry_journal(&journal)
            } else {
                Ok(())
            };
        }
        if let Some(published_index) = published_retry
            && existing_index == Some(published_index)
            && published_index + 1 == current_applied_count
        {
            kura.apply_geometry_operations_forward(
                &journal.records[published_index].operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            kura.ensure_authoritative_lane_markers_with_receipts(
                updated,
                updated_incarnations,
                updated_activation_heights,
                namespace_receipts.as_deref_mut(),
            )?;
            *kura.lane_storage_entries.lock() = kura.lane_storage_entries_from_geometry(
                updated,
                updated_incarnations,
                updated_activation_heights,
            )?;
            return Ok(());
        }
        let desired_previous_count = existing_index.unwrap_or(current_applied_count);
        if existing_index.is_none() && current_applied_count != journal.records.len() {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry cannot branch across a retained rolled-back transition",
            ));
        }
        kura.reconcile_lane_geometry_history_to_count(
            &mut journal,
            previous_catalog,
            previous_lineage_root,
            desired_previous_count,
        )?;
        kura.ensure_authoritative_lane_markers_with_receipts(
            previous,
            previous_incarnations,
            previous_activation_heights,
            namespace_receipts.as_deref_mut(),
        )?;
        if let Some(existing_index) = existing_index {
            let existing = &journal.records[existing_index];
            if existing.previous_catalog != previous_catalog
                || existing.previous_lineage_root != previous_lineage_root
                || existing.updated_catalog != updated_catalog
                || existing.updated_lineage_root != updated_lineage_root
                || existing.previous_bindings != previous_bindings
                || existing.updated_bindings != updated_bindings
            {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry transition id collides with a different exact identity",
                ));
            }
            let operations = journal.records[existing_index].operations.clone();
            let retiring = kura.geometry_retirement_identities(previous, &operations)?;
            kura.ensure_lane_retirement_admissible_locked(
                pending_canonical_bytes,
                &retiring,
                &certified_retirements,
            )?;
            let mut prepared =
                PreparedGeometryJournalTransition::prepare(kura, journal, existing_index)?;
            // Keep the retained terminal phase until the replay finishes. Downgrading a
            // `RolledBack` record to `Intent` would let a crash erase the fact that subsequent
            // recovery must authenticate existing storage rather than provision an empty pair.
            kura.apply_geometry_operations_forward(
                prepared.operations(),
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            prepared.persist(kura, LaneGeometryPhase::FilesApplied)?;
            kura.ensure_authoritative_lane_markers_with_receipts(
                updated,
                updated_incarnations,
                updated_activation_heights,
                namespace_receipts.as_deref_mut(),
            )?;
            *kura.lane_storage_entries.lock() = kura.lane_storage_entries_from_geometry(
                updated,
                updated_incarnations,
                updated_activation_heights,
            )?;
            return Ok(());
        }
        let last_sequence = journal
            .records
            .iter()
            .map(|record| record.transition_sequence)
            .chain(
                journal
                    .pending_archive_gc
                    .iter()
                    .map(|pending| pending.intent.transition_sequence),
            )
            .chain(
                journal
                    .checkpoint
                    .iter()
                    .filter_map(|checkpoint| checkpoint.transition_sequence),
            )
            .max();
        let transition_sequence = match last_sequence {
            Some(sequence) => sequence.checked_add(1).ok_or_else(|| {
                kura.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry transition sequence overflow",
                )
            })?,
            None => 0,
        };
        let transition_id = geometry_transition_id(
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );
        let operations = kura.build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            replaced_lane_ids,
        )?;
        let retiring = kura.geometry_retirement_identities(previous, &operations)?;
        kura.ensure_lane_retirement_admissible_locked(
            pending_canonical_bytes,
            &retiring,
            &certified_retirements,
        )?;
        let intent = LaneGeometryIntent {
            transition_id,
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        };
        journal.records.push(intent);
        let record_index = journal.records.len() - 1;
        let mut prepared = PreparedGeometryJournalTransition::prepare(kura, journal, record_index)?;
        prepared.persist(kura, LaneGeometryPhase::Intent)?;
        if let Err(error) = kura.apply_geometry_operations_forward(
            prepared.operations(),
            GeometryEvidencePolicy::FreshJournalIntent,
        ) {
            if let Err(rollback_error) = kura.apply_geometry_operations_rollback(
                prepared.operations(),
                GeometryEvidencePolicy::AllowJournalIntentProvisioning,
            ) {
                let ambiguous = Error::IO(
                    std::io::Error::other(format!(
                        "lane geometry apply failed ({error}); rollback failed ({rollback_error})"
                    )),
                    kura.lane_geometry_journal_path(),
                );
                kura.poison_canonical_storage("lane geometry apply rollback", &ambiguous);
                return Err(Error::CanonicalStoragePoisoned);
            }
            prepared.persist(kura, LaneGeometryPhase::RolledBack)?;
            return Err(error);
        }
        prepared.persist(kura, LaneGeometryPhase::FilesApplied)?;
        kura.ensure_authoritative_lane_markers_with_receipts(
            updated,
            updated_incarnations,
            updated_activation_heights,
            namespace_receipts.as_deref_mut(),
        )?;
        *kura.lane_storage_entries.lock() = kura.lane_storage_entries_from_geometry(
            updated,
            updated_incarnations,
            updated_activation_heights,
        )?;
        Ok(())
    }

    /// Publish the exact catalog phase with original-journal restoration on failure.
    pub(in crate::kura::lane_geometry) fn publish_prepared_lane_geometry_catalog(
        &self,
        prepared: PreparedLaneGeometryCatalog,
    ) -> Result<()> {
        let kura = self.kura_under_publication_guards();
        let PreparedLaneGeometryCatalog {
            bindings,
            fingerprint,
            lineage_root,
            configured_baseline,
            mut journal,
        } = prepared;
        let journal_path = kura.lane_geometry_journal_path();
        let publication_temp = kura.store_root.join(JOURNAL_TEMP_FILE_NAME);
        let prior_journal_bytes = kura.read_geometry_file_bytes(&journal_path)?;
        let publication_temp_preexisted = kura.validate_path_kind(&publication_temp, false)?;
        let uncertain = journal.records.iter().position(|record| {
            matches!(
                record.phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied
            )
        });
        if let Some(index) = uncertain {
            let record = &journal.records[index];
            if record.updated_catalog != fingerprint
                || record.updated_lineage_root != lineage_root
                || record.updated_bindings != bindings
            {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "catalog publication does not match the uncertain geometry identity",
                ));
            }
            journal.records[index].phase = LaneGeometryPhase::CatalogPublished;
        } else if !journal.records.is_empty() {
            let applied_count = journal
                .records
                .iter()
                .position(|record| record.phase == LaneGeometryPhase::RolledBack)
                .unwrap_or(journal.records.len());
            let current_matches = if applied_count == 0 {
                let record = &journal.records[0];
                record.previous_catalog == fingerprint
                    && record.previous_lineage_root == lineage_root
                    && record.previous_bindings == bindings
            } else {
                let record = &journal.records[applied_count - 1];
                record.updated_catalog == fingerprint
                    && record.updated_lineage_root == lineage_root
                    && record.updated_bindings == bindings
            };
            if !current_matches {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "catalog publication does not match the durable geometry frontier identity",
                ));
            }
        } else if journal.checkpoint.as_ref().is_some_and(|checkpoint| {
            checkpoint.catalog != fingerprint
                || checkpoint.lineage_root != lineage_root
                || checkpoint.bindings != bindings
        }) {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "catalog publication does not match the compacted geometry identity",
            ));
        }
        if let Some(attempted) = configured_baseline {
            match journal.configured_catalog_hash {
                Some(expected) if expected == attempted => {}
                None => {
                    return Err(kura.geometry_error(
                        ErrorKind::InvalidData,
                        "configured catalog publication has no authenticated startup baseline",
                    ));
                }
                Some(expected) => {
                    return Err(kura.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!(
                            "configured lane catalog baseline mismatch: expected {expected}, attempted {attempted}"
                        ),
                    ));
                }
            }
            let primary_binding = bindings.first().ok_or_else(|| {
                kura.geometry_error(
                    ErrorKind::InvalidData,
                    "configured catalog publication has no primary geometry binding",
                )
            })?;
            if primary_binding.lane_id != LaneId::SINGLE || primary_binding.activation_height != 0 {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "configured primary geometry binding is not lane zero at activation zero",
                ));
            }
            match journal.configured_primary_binding.as_ref() {
                Some(expected) if expected == primary_binding => {}
                None => {
                    return Err(kura.geometry_error(
                        ErrorKind::InvalidData,
                        "configured catalog publication has no authenticated primary geometry anchor",
                    ));
                }
                Some(_) => {
                    return Err(kura.geometry_error(
                        ErrorKind::InvalidData,
                        "configured primary geometry binding differs from its durable anchor",
                    ));
                }
            }
            kura.require_lane_marker(primary_binding)?;
        }
        kura.validate_lane_geometry_journal(&journal)?;
        let published_journal_bytes = journal.encode();
        // Use the same encoded bytes for the target replacement and rollback comparison. This
        // makes the exact value whose publication was attempted explicit even if the encoder is
        // changed in the future.
        let publication_result = kura.atomic_write_geometry_file(
            &journal_path,
            &publication_temp,
            &published_journal_bytes,
        );
        #[cfg(test)]
        let publication_result = publication_result.and_then(|()| {
            if kura
                .fail_next_lane_geometry_publication_after_write
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err(kura.geometry_error(
                    ErrorKind::Other,
                    "lane geometry publication failed after journal replacement for test injection",
                ));
            }
            Ok(())
        });
        if let Err(publication_error) = publication_result {
            if let Err(restore_error) = kura.restore_lane_geometry_journal_file(
                prior_journal_bytes.as_deref(),
                &published_journal_bytes,
                publication_temp_preexisted,
            ) {
                return Err(Error::LaneGeometryPublicationRestoreFailed {
                    publication: publication_error.to_string(),
                    restoration: restore_error.to_string(),
                });
            }
            return Err(publication_error);
        }
        Ok(())
    }
}
