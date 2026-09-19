//! Catalog publication for structural storage fixtures after owner surrender.
//!
//! Production transitions retain one `RawGeometryAttempt` through both file
//! application and catalog publication. This test-only helper lets structural
//! fixtures inspect and recover a complete durable phase independently.

use super::*;
use crate::kura::publication_lease::KuraPublicationLease;

/// Exact catalog and original GC-reconciled journal to publish under held fences.
pub(super) struct PreparedLaneGeometryCatalog {
    pub(super) bindings: Vec<LaneGeometryBinding>,
    pub(super) fingerprint: Hash,
    pub(super) lineage_root: Hash,
    pub(super) configured_baseline: Option<Hash>,
    pub(super) journal: LaneGeometryJournal,
}

impl KuraPublicationLease<'_> {
    /// Publish the exact catalog phase with original-journal restoration on failure.
    pub(in crate::kura::lane_geometry) fn publish_prepared_lane_geometry_catalog(
        &self,
        prepared: PreparedLaneGeometryCatalog,
    ) -> Result<()> {
        let kura = self.original_kura();
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
