// Lane-geometry journal structure validation and recovery guards.
fn lane_geometry_journal_structure_error(
    store_root: &Path,
    kind: ErrorKind,
    message: &'static str,
) -> Error {
    Error::IO(
        std::io::Error::new(kind, message),
        store_root.join(JOURNAL_FILE_NAME),
    )
}
fn validate_lane_geometry_phase_frontier(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    let mut saw_uncertain_boundary = false;
    let mut saw_rolled_back = false;
    for record in &journal.records {
        match record.phase {
            LaneGeometryPhase::CatalogPublished => {
                if saw_uncertain_boundary || saw_rolled_back {
                    return Err(lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "lane geometry journal phases do not form a durable applied frontier",
                    ));
                }
            }
            LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied => {
                if saw_uncertain_boundary || saw_rolled_back {
                    return Err(lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "lane geometry journal has more than one uncertain transition boundary",
                    ));
                }
                saw_uncertain_boundary = true;
            }
            LaneGeometryPhase::RolledBack => {
                saw_rolled_back = true;
            }
        }
    }
    Ok(())
}
fn validate_lane_geometry_journal_structure(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    if journal.version != JOURNAL_VERSION
        || journal.records.len() > MAX_GEOMETRY_TRANSITIONS
        || journal.pending_archive_gc.len() > MAX_GEOMETRY_TRANSITIONS
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal has an unsupported version or too many transitions",
        ));
    }
    if journal.configured_primary_binding.is_some() && journal.configured_catalog_hash.is_none() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "configured primary geometry binding has no configured-catalog baseline",
        ));
    }
    if let Some(primary) = journal.configured_primary_binding.as_ref() {
        if primary.lane_id != LaneId::SINGLE || primary.activation_height != 0 {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "configured primary geometry binding is not lane zero at activation zero",
            ));
        }
        validate_geometry_binding_structure(store_root, primary)?;
    }
    if let Some(checkpoint) = journal.checkpoint.as_ref() {
        validate_lane_geometry_checkpoint_structure(store_root, checkpoint)?;
        if journal.records.first().is_some_and(|record| {
            record.previous_catalog != checkpoint.catalog
                || record.previous_lineage_root != checkpoint.lineage_root
        }) {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal retained history does not start at its checkpoint catalog",
            ));
        }
        if let (Some(checkpoint), Some(first)) =
            (journal.checkpoint.as_ref(), journal.records.first())
            && (checkpoint
                .transition_sequence
                .is_some_and(|sequence| first.transition_sequence <= sequence)
                || first.transition_height <= checkpoint.snapshot_height)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "retained lane geometry history does not advance beyond its checkpoint cursor",
            ));
        }
    } else if !journal.pending_archive_gc.is_empty() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal has pending archive GC without a durable checkpoint",
        ));
    }
    validate_pending_lane_geometry_gc_structure(store_root, journal)?;
    validate_lane_geometry_phase_frontier(store_root, journal)?;
    let mut transition_ids = BTreeSet::new();
    let mut retained_paths = BTreeSet::new();
    if journal.records.windows(2).any(|pair| {
        pair[0].transition_sequence >= pair[1].transition_sequence
            || pair[0].transition_height > pair[1].transition_height
    }) {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal transition cursor is not monotonic",
        ));
    }
    for (record_index, record) in journal.records.iter().enumerate() {
        if record.transition_id
            != geometry_transition_id(
                record.transition_sequence,
                record.transition_height,
                record.previous_catalog,
                record.previous_lineage_root,
                record.updated_catalog,
                record.updated_lineage_root,
            )
            || record.previous_catalog == record.updated_catalog
                && record.previous_lineage_root == record.updated_lineage_root
            || lineage_root_is_zero(record.previous_lineage_root)
            || lineage_root_is_zero(record.updated_lineage_root)
            || !transition_ids.insert(record.transition_id)
            || record.operations.len() > MAX_GEOMETRY_BINDINGS.saturating_mul(2)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal contains an invalid or duplicate transition",
            ));
        }
        for bindings in [&record.previous_bindings, &record.updated_bindings] {
            validate_geometry_binding_set_structure(store_root, bindings)?;
        }
        if geometry_catalog_fingerprint(&record.previous_bindings) != record.previous_catalog
            || geometry_catalog_fingerprint(&record.updated_bindings) != record.updated_catalog
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal catalog fingerprint does not match its bindings",
            ));
        }
        if record_index > 0
            && (journal.records[record_index - 1].updated_catalog != record.previous_catalog
                || journal.records[record_index - 1].updated_lineage_root
                    != record.previous_lineage_root)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal transition chain is not contiguous",
            ));
        }
        let transition_hex = hex::encode(record.transition_id.as_ref());
        let previous_by_lane = record
            .previous_bindings
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let updated_by_lane = record
            .updated_bindings
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        if record
            .operations
            .windows(2)
            .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal operations are duplicated or unsorted",
            ));
        }
        let mut lane_ids = BTreeSet::new();
        for operation in &record.operations {
            if !lane_ids.insert(operation.lane_id)
                || operation
                    .previous
                    .as_ref()
                    .is_some_and(|binding| binding.lane_id != operation.lane_id)
                || operation
                    .updated
                    .as_ref()
                    .is_some_and(|binding| binding.lane_id != operation.lane_id)
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains duplicate or mismatched lane operations",
                ));
            }
            let expected_root = format!(
                "retired/lane_geometry/{transition_hex}/lane_{:010}",
                operation.lane_id.as_u32()
            );
            let expected_paths = [
                format!("{expected_root}/previous_blocks"),
                format!("{expected_root}/previous_merge.log"),
                format!("{expected_root}/unpublished_blocks"),
                format!("{expected_root}/unpublished_merge.log"),
            ];
            let actual_paths = [
                &operation.archived_blocks_path,
                &operation.archived_merge_path,
                &operation.unpublished_blocks_path,
                &operation.unpublished_merge_path,
            ];
            if actual_paths
                .iter()
                .zip(expected_paths.iter())
                .any(|(actual, expected)| *actual != expected)
                || actual_paths
                    .iter()
                    .any(|path| !retained_paths.insert((*path).clone()))
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains forged or colliding archive paths",
                ));
            }
            for (path, directory) in actual_paths.iter().zip([true, false, true, false]) {
                validate_geometry_journal_relative_path(store_root, path, directory)?;
            }
            for binding in operation.previous.iter().chain(operation.updated.iter()) {
                validate_geometry_binding_structure(store_root, binding)?;
            }
            if operation.previous.as_ref() != previous_by_lane.get(&operation.lane_id).copied()
                || operation.updated.as_ref() != updated_by_lane.get(&operation.lane_id).copied()
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry operation does not match its authenticated catalog bindings",
                ));
            }
            let shape_is_valid = match operation.kind {
                LaneGeometryOperationKind::Create => {
                    operation.previous.is_none() && operation.updated.is_some()
                }
                LaneGeometryOperationKind::Retire => {
                    operation.previous.is_some() && operation.updated.is_none()
                }
                LaneGeometryOperationKind::Replace => operation
                    .previous
                    .as_ref()
                    .zip(operation.updated.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation != updated.incarnation
                            || previous.activation_height != updated.activation_height
                    }),
                LaneGeometryOperationKind::Relabel => operation
                    .previous
                    .as_ref()
                    .zip(operation.updated.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation == updated.incarnation
                            && previous.activation_height == updated.activation_height
                            && (previous.blocks_path != updated.blocks_path
                                || previous.merge_path != updated.merge_path)
                    }),
            };
            if !shape_is_valid {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains an invalid operation shape",
                ));
            }
        }
        let expected_changed_lanes = previous_by_lane
            .keys()
            .chain(updated_by_lane.keys())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|lane_id| {
                previous_by_lane.get(lane_id).copied() != updated_by_lane.get(lane_id).copied()
            })
            .count();
        if record.operations.len() != expected_changed_lanes {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal omits or invents a catalog binding operation",
            ));
        }
    }
    Ok(())
}
fn validate_lane_geometry_checkpoint_structure(
    store_root: &Path,
    checkpoint: &LaneGeometrySnapshotCheckpoint,
) -> Result<()> {
    validate_geometry_binding_set_structure(store_root, &checkpoint.bindings)?;
    validate_geometry_merge_release_structure(
        store_root,
        &checkpoint.merge_releases,
        checkpoint.snapshot_height,
    )?;
    if checkpoint.version != CHECKPOINT_VERSION
        || checkpoint
            .snapshot_state_hash
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        || checkpoint.catalog != geometry_catalog_fingerprint(&checkpoint.bindings)
        || lineage_root_is_zero(checkpoint.lineage_root)
        || checkpoint.commitment != geometry_checkpoint_commitment(checkpoint)
        || checkpoint.snapshot_height == 0
        || checkpoint.snapshot_block_hash.is_none()
        || checkpoint
            .snapshot_block_hash
            .is_some_and(|hash| hash.as_ref().iter().all(|byte| *byte == 0))
        || checkpoint
            .bindings
            .iter()
            .any(|binding| binding.activation_height > checkpoint.snapshot_height)
        || checkpoint
            .transition_height
            .is_some_and(|height| height > checkpoint.snapshot_height)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint commitment, catalog, height, block hash, or activation is invalid",
        ));
    }
    match (
        checkpoint.transition_sequence,
        checkpoint.transition_height,
        checkpoint.transition_previous_catalog,
        checkpoint.transition_previous_lineage_root,
        checkpoint.transition_id,
    ) {
        (None, None, None, None, None) => Ok(()),
        (
            Some(sequence),
            Some(height),
            Some(previous_catalog),
            Some(previous_lineage_root),
            Some(transition_id),
        ) if !lineage_root_is_zero(previous_lineage_root)
            && transition_id
                == geometry_transition_id(
                    sequence,
                    height,
                    previous_catalog,
                    previous_lineage_root,
                    checkpoint.catalog,
                    checkpoint.lineage_root,
                ) =>
        {
            Ok(())
        }
        _ => Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint transition binding is invalid",
        )),
    }
}
fn validate_geometry_merge_release_structure(
    store_root: &Path,
    releases: &[LaneGeometryMergeRelease],
    snapshot_height: u64,
) -> Result<()> {
    if releases.len() > MAX_GEOMETRY_MERGE_RELEASES
        || releases.windows(2).any(|pair| pair[0] >= pair[1])
        || releases.iter().any(|release| {
            release.lane_block_height == 0
                || release.application_block_height == 0
                || release.application_block_height > snapshot_height
                || release
                    .lane_incarnation
                    .as_ref()
                    .iter()
                    .all(|byte| *byte == 0)
        })
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "geometry checkpoint merge releases are invalid, duplicated, unsorted, or oversized",
        ));
    }
    Ok(())
}
fn validate_pending_lane_geometry_gc_structure(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    if journal.pending_archive_gc.is_empty() {
        if journal
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.pending_archive_gc_root.is_some())
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry checkpoint commits a missing pending archive GC set",
            ));
        }
        return Ok(());
    }
    let checkpoint = journal.checkpoint.as_ref().ok_or_else(|| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "pending lane geometry GC has no checkpoint",
        )
    })?;
    let retained_ids = journal
        .records
        .iter()
        .map(|record| record.transition_id)
        .collect::<BTreeSet<_>>();
    let mut pending_ids = BTreeSet::new();
    for (index, pending) in journal.pending_archive_gc.iter().enumerate() {
        let intent = &pending.intent;
        let standalone = LaneGeometryJournal {
            version: JOURNAL_VERSION,
            configured_catalog_hash: None,
            configured_primary_binding: None,
            checkpoint: None,
            pending_archive_gc: Vec::new(),
            records: vec![intent.clone()],
        };
        validate_lane_geometry_journal_structure(store_root, &standalone)?;
        if intent.phase != LaneGeometryPhase::CatalogPublished
            || !pending_ids.insert(intent.transition_id)
            || retained_ids.contains(&intent.transition_id)
            || index > 0
                && (journal.pending_archive_gc[index - 1].intent.updated_catalog
                    != intent.previous_catalog
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .updated_lineage_root
                        != intent.previous_lineage_root
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .transition_sequence
                        >= intent.transition_sequence
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .transition_height
                        > intent.transition_height)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal has forged or non-contiguous pending archive GC",
            ));
        }
    }
    if checkpoint.pending_archive_gc_root
        != Some(geometry_pending_archive_gc_root(
            &journal.pending_archive_gc,
        ))
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint does not bind its exact pending archive GC set",
        ));
    }
    let last = journal
        .pending_archive_gc
        .last()
        .expect("non-empty pending archive GC");
    if last.intent.updated_catalog != checkpoint.catalog
        || last.intent.updated_lineage_root != checkpoint.lineage_root
        || checkpoint.transition_sequence != Some(last.intent.transition_sequence)
        || checkpoint.transition_height != Some(last.intent.transition_height)
        || checkpoint.transition_previous_catalog != Some(last.intent.previous_catalog)
        || checkpoint.transition_previous_lineage_root != Some(last.intent.previous_lineage_root)
        || checkpoint.transition_id != Some(last.intent.transition_id)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry pending archive GC does not terminate at its checkpoint",
        ));
    }
    Ok(())
}
// Lane-geometry catalog validation and deterministic commitment helpers.
fn validate_geometry_binding_structure(
    store_root: &Path,
    binding: &LaneGeometryBinding,
) -> Result<()> {
    if binding.incarnation.as_ref().iter().all(|byte| *byte == 0) {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal contains a zero incarnation",
        ));
    }
    validate_geometry_journal_relative_path(store_root, &binding.blocks_path, true)?;
    validate_geometry_journal_relative_path(store_root, &binding.merge_path, false)
}
fn validate_geometry_binding_set_structure(
    store_root: &Path,
    bindings: &[LaneGeometryBinding],
) -> Result<()> {
    if bindings.is_empty()
        || bindings.len() > MAX_GEOMETRY_BINDINGS
        || bindings
            .windows(2)
            .any(|pair| pair[0].lane_id >= pair[1].lane_id)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry catalog bindings are empty, duplicated, or unsorted",
        ));
    }
    let mut incarnations = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for binding in bindings {
        validate_geometry_binding_structure(store_root, binding)?;
        if !incarnations.insert(binding.incarnation)
            || !paths.insert(binding.blocks_path.clone())
            || !paths.insert(binding.merge_path.clone())
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry catalog contains duplicate incarnations or storage paths",
            ));
        }
    }
    Ok(())
}
fn validate_geometry_journal_relative_path(
    store_root: &Path,
    relative: &str,
    directory: bool,
) -> Result<()> {
    let relative = Path::new(relative);
    validate_relative_path(relative)?;
    let root_metadata = fs::symlink_metadata(store_root)
        .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "Kura geometry store root must remain a non-symlink directory",
        ));
    }
    let components = relative.components().collect::<Vec<_>>();
    let mut cursor = store_root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        cursor.push(component.as_os_str());
        let is_target = index + 1 == components.len();
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path traverses or targets a symlink",
                    ),
                    cursor,
                ));
            }
            Ok(metadata) if !is_target && !metadata.file_type().is_dir() => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path traverses a non-directory",
                    ),
                    cursor,
                ));
            }
            Ok(metadata)
                if is_target
                    && ((directory && !metadata.file_type().is_dir())
                        || (!directory && !metadata.file_type().is_file())) =>
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path target has the wrong file type",
                    ),
                    cursor,
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(Error::IO(error, cursor)),
        }
    }
    Ok(())
}
fn native_amx_receipt_targets_retirement(
    receipt: &iroha_data_model::block::consensus::NativeAmxReceipt,
    retiring: &BTreeSet<LaneRetirementIdentity>,
) -> std::result::Result<bool, &'static str> {
    let mut targets_retirement = false;
    for identity in retiring {
        targets_retirement |=
            crate::native_amx::native_amx_receipt_requires_separate_participant_application_for(
                receipt,
                identity.lane_id,
                identity.dataspace_id,
                identity.lane_incarnation,
            )?;
    }
    Ok(targets_retirement)
}
fn lane_payload_targets_retirement(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    retiring: &BTreeSet<LaneRetirementIdentity>,
) -> bool {
    let descriptor = &payload.origin_proposal.descriptor;
    if retiring.contains(&LaneRetirementIdentity {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
    }) {
        return true;
    }
    if payload.routing_plans.len() != payload.native_amx_receipts.len() {
        return true;
    }
    payload
        .routing_plans
        .iter()
        .zip(&payload.native_amx_receipts)
        .any(|(plan, receipt)| {
            let (crate::queue::RoutingPlan::NativeAmx(plan), Some(receipt)) = (plan, receipt)
            else {
                return !matches!(
                    (plan, receipt),
                    (crate::queue::RoutingPlan::Single(_), None)
                );
            };
            if receipt.plan_digest != plan.plan_digest
                || receipt.legs.len() != plan.participants.len()
                || receipt
                    .legs
                    .iter()
                    .zip(&plan.participants)
                    .any(|(leg, planned)| {
                        leg.lane_id != planned.route.lane_id
                            || leg.dataspace_id != planned.route.dataspace_id
                    })
            {
                return true;
            }
            native_amx_receipt_targets_retirement(receipt, retiring).unwrap_or(true)
        })
}
fn lane_proposal_coordinator_targets_retirement(
    proposal: &LaneBlockProposalV1,
    retiring: &BTreeSet<LaneRetirementIdentity>,
) -> bool {
    let descriptor = &proposal.descriptor;
    retiring.contains(&LaneRetirementIdentity {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
    })
}
fn routing_plan_from_execution_context(
    context: &ExternalExecutionContext,
) -> Option<crate::queue::RoutingPlan> {
    crate::queue::routing_plan_from_execution_context(context).ok()
}
fn geometry_catalog_fingerprint(bindings: &[LaneGeometryBinding]) -> Hash {
    let encoded = bindings.to_vec().encode();
    Hash::new_from_chunks(&[CATALOG_DOMAIN, encoded.as_slice()])
}
#[cfg(test)]
fn unscoped_lineage_root(bindings: &[LaneGeometryBinding]) -> Hash {
    let catalog = geometry_catalog_fingerprint(bindings);
    Hash::new_from_chunks(&[UNSCOPED_LINEAGE_DOMAIN, catalog.as_ref()])
}
fn lineage_root_is_zero(root: Hash) -> bool {
    root.as_ref().iter().all(|byte| *byte == 0)
}
fn geometry_transition_id(
    transition_sequence: u64,
    transition_height: u64,
    previous_catalog: Hash,
    previous_lineage_root: Hash,
    updated_catalog: Hash,
    updated_lineage_root: Hash,
) -> Hash {
    Hash::new_from_chunks(&[
        TRANSITION_DOMAIN,
        &transition_sequence.to_le_bytes(),
        &transition_height.to_le_bytes(),
        previous_catalog.as_ref(),
        previous_lineage_root.as_ref(),
        updated_catalog.as_ref(),
        updated_lineage_root.as_ref(),
    ])
}
fn geometry_checkpoint_commitment(checkpoint: &LaneGeometrySnapshotCheckpoint) -> Hash {
    let mut payload = Vec::new();
    payload.push(checkpoint.version);
    payload.extend_from_slice(&checkpoint.snapshot_height.to_le_bytes());
    match checkpoint.snapshot_block_hash {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(checkpoint.snapshot_state_hash.as_ref());
    payload.extend_from_slice(checkpoint.catalog.as_ref());
    payload.extend_from_slice(checkpoint.lineage_root.as_ref());
    match checkpoint.transition_sequence {
        Some(sequence) => {
            payload.push(1);
            payload.extend_from_slice(&sequence.to_le_bytes());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_height {
        Some(height) => {
            payload.push(1);
            payload.extend_from_slice(&height.to_le_bytes());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_previous_catalog {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_previous_lineage_root {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_id {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(&checkpoint.bindings.clone().encode());
    payload.extend_from_slice(&checkpoint.merge_releases.clone().encode());
    match checkpoint.pending_archive_gc_root {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    Hash::new_from_chunks(&[CHECKPOINT_DOMAIN, payload.as_slice()])
}
fn geometry_pending_archive_gc_root(pending: &[LaneGeometryPendingArchiveGc]) -> Hash {
    Hash::new_from_chunks(&[PENDING_GC_DOMAIN, pending.to_vec().encode().as_slice()])
}
fn geometry_merge_marker_set_root(markers: &[(StatePath, Vec<u8>)]) -> Hash {
    Hash::new_from_chunks(&[
        MERGE_RELEASE_MARKERS_DOMAIN,
        markers.to_vec().encode().as_slice(),
    ])
}
fn lane_geometry_snapshot_checkpoint(
    snapshot_height: u64,
    snapshot_block_hash: Option<HashOf<BlockHeader>>,
    snapshot_state_hash: Hash,
    bindings: Vec<LaneGeometryBinding>,
    lineage_root: Hash,
    transition_sequence: Option<u64>,
    transition_height: Option<u64>,
    transition_previous_catalog: Option<Hash>,
    transition_previous_lineage_root: Option<Hash>,
    transition_id: Option<Hash>,
    merge_releases: Vec<LaneGeometryMergeRelease>,
    pending_archive_gc_root: Option<Hash>,
) -> LaneGeometrySnapshotCheckpoint {
    let mut checkpoint = LaneGeometrySnapshotCheckpoint {
        version: CHECKPOINT_VERSION,
        snapshot_height,
        snapshot_block_hash,
        snapshot_state_hash,
        catalog: geometry_catalog_fingerprint(&bindings),
        lineage_root,
        transition_sequence,
        transition_height,
        transition_previous_catalog,
        transition_previous_lineage_root,
        transition_id,
        bindings,
        merge_releases,
        pending_archive_gc_root,
        commitment: Hash::prehashed([0; Hash::LENGTH]),
    };
    checkpoint.commitment = geometry_checkpoint_commitment(&checkpoint);
    checkpoint
}
fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "lane geometry journal contains an unsafe relative path",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
}
fn geometry_file_identity(metadata: &SecureMetadata) -> GeometryFileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        GeometryFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        let volume_serial_number = metadata.volume_serial_number();
        let file_index = metadata.file_index();
        let unsupported_nonce = if volume_serial_number.is_some() && file_index.is_some() {
            0
        } else {
            // Some Windows filesystems do not expose stable volume/file IDs. A fresh nonce makes
            // every subsequent comparison fail closed instead of treating all paths as equal.
            UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed)
        };
        GeometryFileIdentity {
            volume_serial_number,
            file_index,
            unsupported_nonce,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::sync::atomic::Ordering;
        let _ = metadata;
        GeometryFileIdentity {
            unsupported_nonce: UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed),
        }
    }
}
fn checked_geometry_file_identity(
    metadata: &SecureMetadata,
    path: &Path,
) -> Result<GeometryFileIdentity> {
    let identity = geometry_file_identity(metadata);
    #[cfg(windows)]
    if identity.unsupported_nonce != 0 {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "Windows filesystem did not expose a stable volume and file identity",
            ),
            path.to_path_buf(),
        ));
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = identity;
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "lane geometry requires stable filesystem object identities",
            ),
            path.to_path_buf(),
        ));
    }
    #[cfg(any(unix, windows))]
    {
        let _ = path;
        Ok(identity)
    }
}
fn decode_exact<T: Decode>(bytes: &[u8]) -> std::result::Result<T, norito::core::Error> {
    let mut input = bytes;
    let value = T::decode(&mut input)?;
    if !input.is_empty() {
        return Err(norito::core::Error::Message(
            "trailing bytes in lane geometry sidecar".to_owned(),
        ));
    }
    Ok(value)
}
