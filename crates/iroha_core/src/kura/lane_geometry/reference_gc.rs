//! Collection of immutable instances after reference publication and snapshot proof.
//!
//! Apply never moves these objects. A bounded, durable GC intent receives an
//! exact instance only after all original retirement/release checks pass. The
//! current catalog, its actual MV predecessor, and retained journal images pin
//! their instances independently of aliases and current LaneId reuse.

use super::*;

impl Kura {
    /// Caller owns prune -> canonical-chain -> geometry; release checks take
    /// sidecar last. No lock or descriptor is retained across a Kura write.
    pub(super) fn collect_released_lane_instances_locked(
        &self,
        journal: &mut LaneGeometryJournal,
    ) -> Result<(LaneGeometryGcSummary, Option<usize>)> {
        let checkpoint = journal
            .checkpoint
            .as_ref()
            .expect("validated GC checkpoint");
        let independently_owned = checkpoint
            .bindings
            .iter()
            .chain(journal.records.iter().flat_map(|record| {
                record
                    .previous_bindings
                    .iter()
                    .chain(record.updated_bindings.iter())
            }))
            .map(LaneGeometryBinding::identity)
            .collect::<BTreeSet<_>>();
        let predecessor_owned = checkpoint
            .recovery_bindings
            .iter()
            .map(LaneGeometryBinding::identity)
            .collect::<BTreeSet<_>>();
        let merge_releases = checkpoint.merge_releases.clone();
        // The last exact occurrence owns collection. Repeated catalog references
        // must not introduce two physical deletion owners for the same instance.
        let mut owners = BTreeMap::new();
        for (index, pending) in journal.pending_archive_gc.iter().enumerate() {
            for operation in &pending.intent.operations {
                for (previous, binding) in [
                    (true, operation.previous.as_ref()),
                    (false, operation.updated.as_ref()),
                ] {
                    if let Some(binding) = binding {
                        owners.insert(binding.identity(), (index, previous, binding.clone()));
                    }
                }
            }
        }
        let retain_from = owners
            .iter()
            .filter_map(|(identity, (index, _, _))| {
                (predecessor_owned.contains(identity) && !independently_owned.contains(identity))
                    .then_some(*index)
            })
            .min();
        let mut summary = LaneGeometryGcSummary::default();
        for index in 0..journal.pending_archive_gc.len() {
            let mut archive = journal.pending_archive_gc[index].clone();
            let transition = hex::encode(archive.intent.transition_id.as_ref());
            // Archive paths are GC-local derivations, never catalog identities.
            for operation in &mut archive.intent.operations {
                let root = format!(
                    "retired/lane_geometry/{transition}/lane_{:010}",
                    operation.lane_id.as_u32()
                );
                operation.archived_blocks_path = format!("{root}/previous_blocks");
                operation.archived_merge_path = format!("{root}/previous_merge.log");
                operation.unpublished_blocks_path = format!("{root}/unpublished_blocks");
                operation.unpublished_merge_path = format!("{root}/unpublished_merge.log");
            }
            let quarantine = self.resolve_relative_path(&format!(
                "retired/lane_geometry/{GC_QUARANTINE_PREFIX}{transition}"
            ))?;
            let quarantined = self.validate_path_kind(&quarantine, true)?;
            for (identity, (owner, previous, binding)) in &owners {
                if *owner != index
                    || independently_owned.contains(identity)
                    || predecessor_owned.contains(identity)
                {
                    continue;
                }
                let operation = archive
                    .intent
                    .operations
                    .iter()
                    .find(|operation| operation.lane_id == binding.lane_id)
                    .expect("candidate came from exact operation");
                let (blocks_relative, merge_relative) = if *previous {
                    (
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )
                } else {
                    (
                        &operation.unpublished_blocks_path,
                        &operation.unpublished_merge_path,
                    )
                };
                let blocks = self.binding_blocks_path(binding);
                let merge = self.binding_merge_path(binding);
                let archived_blocks = self.resolve_relative_path(blocks_relative)?;
                let archived_merge = self.resolve_relative_path(merge_relative)?;
                let admitted = journal.pending_archive_gc[index]
                    .collecting
                    .contains(binding);
                if !admitted {
                    if quarantined {
                        return Err(self.geometry_error(
                            ErrorKind::InvalidData,
                            "geometry GC quarantine predates exact instance release admission",
                        ));
                    }
                    self.require_complete_geometry_binding_at(binding, &blocks, &merge)?;
                    self.ensure_archived_lane_work_released(&blocks, binding, &merge_releases)?;
                    journal.pending_archive_gc[index]
                        .collecting
                        .push(binding.clone());
                    journal.pending_archive_gc[index]
                        .collecting
                        .sort_by_key(LaneGeometryBinding::identity);
                    let root = geometry_pending_archive_gc_root(&journal.pending_archive_gc);
                    let checkpoint = journal
                        .checkpoint
                        .as_mut()
                        .expect("validated GC checkpoint");
                    checkpoint.pending_archive_gc_root = Some(root);
                    checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
                    self.write_lane_geometry_journal(journal)?;
                }
                // Absence is a retry of this admitted deletion, never permission
                // to adopt a substitute object or manufacture an empty pair.
                if !quarantined
                    && (self.validate_path_kind(&blocks, true)?
                        || self.validate_path_kind(&merge, false)?
                        || self.validate_path_kind(&archived_blocks, true)?
                        || self.validate_path_kind(&archived_merge, false)?)
                {
                    self.archive_geometry_binding(binding, blocks_relative, merge_relative)?;
                }
            }
            archive.collecting = journal.pending_archive_gc[index].collecting.clone();
            let (bytes, existed) =
                self.remove_authenticated_geometry_archive(&archive, &merge_releases)?;
            summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
            summary.removed_archive_roots = summary
                .removed_archive_roots
                .saturating_add(usize::from(existed));
        }
        Ok((summary, retain_from))
    }
}
