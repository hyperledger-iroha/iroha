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
    if journal.version != JOURNAL_VERSION || journal.records.len() > MAX_GEOMETRY_TRANSITIONS {
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
    // Full uncompacted history must start at the original configured primary.
    if let Some(first) = journal.records.first()
        && (first.transition_sequence != 0
            || journal
                .configured_primary_binding
                .as_ref()
                .is_none_or(|primary| first.previous_bindings.first() != Some(primary)))
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry history omits its original configured primary prefix",
        ));
    }
    validate_lane_geometry_phase_frontier(store_root, journal)?;
    let mut transition_ids = BTreeSet::new();
    if journal.records.windows(2).any(|pair| {
        pair[0].transition_sequence.checked_add(1) != Some(pair[1].transition_sequence)
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
        if record.previous_bindings[0].network_id != record.updated_bindings[0].network_id {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry transition changes the authenticated network",
            ));
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
        if previous_by_lane
            .iter()
            .any(|(lane, binding)| updated_by_lane.get(lane) != Some(binding))
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "native geometry journal contains an unauthorized retirement or replacement",
            ));
        }
        for operation in &record.operations {
            let binding = &operation.created;
            validate_geometry_binding_structure(store_root, binding)?;
            if binding.lane_id != operation.lane_id
                || previous_by_lane.contains_key(&operation.lane_id)
                || updated_by_lane.get(&operation.lane_id).copied() != Some(binding)
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "native geometry creation differs from its exact catalog addition",
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
    let identity = binding.identity();
    if binding.blocks_path != identity.blocks_relative() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry path does not match its complete immutable identity",
        ));
    }
    validate_geometry_journal_relative_path(store_root, &binding.blocks_path, true)
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
    let network_id = bindings[0].network_id;
    if bindings
        .iter()
        .any(|binding| binding.network_id != network_id)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry catalog mixes network identities",
        ));
    }
    let mut incarnations = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for binding in bindings {
        validate_geometry_binding_structure(store_root, binding)?;
        if !incarnations.insert(binding.incarnation) || !paths.insert(binding.blocks_path.clone()) {
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
    for reserved in [
        Path::new("blocks/canonical"),
        Path::new("merge_ledger/canonical.log"),
    ] {
        if relative.starts_with(reserved) || reserved.starts_with(relative) {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry cannot own the canonical-chain namespace",
            ));
        }
    }
    // An in-memory Kura retains the same authenticated instance and relative
    // namespace rules, but owns no filesystem root or ancestor objects.
    if store_root.as_os_str().is_empty() {
        return Ok(());
    }
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
