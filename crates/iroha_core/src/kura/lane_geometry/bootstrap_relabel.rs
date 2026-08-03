// Bootstrap recovery helpers for atomically relabelling durable lane geometry.

fn bootstrap_move_geometry_path(
    store_root: &Path,
    source: &Path,
    target: &Path,
    directory: bool,
) -> Result<bool> {
    if source == target {
        if bootstrap_validate_path_kind(store_root, source, directory)? {
            return Ok(false);
        }
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::NotFound,
                "unchanged primary relabel path is missing during bootstrap recovery",
            ),
            source.to_path_buf(),
        ));
    }
    let source_exists = bootstrap_validate_path_kind(store_root, source, directory)?;
    let target_exists = bootstrap_validate_path_kind(store_root, target, directory)?;
    match (source_exists, target_exists) {
        (false, false) | (false, true) => return Ok(false),
        (true, true) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "both primary relabel paths exist during bootstrap recovery",
                ),
                target.to_path_buf(),
            ));
        }
        (true, false) => {}
    }
    bootstrap_sync_geometry_path(store_root, source, directory)?;
    let identity = checked_geometry_file_identity(
        &fs::symlink_metadata(source).map_err(|error| Error::IO(error, source.to_path_buf()))?,
        source,
    )?;
    let source_parent = source.parent().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel source has no parent",
            ),
            source.to_path_buf(),
        )
    })?;
    let target_parent = target.parent().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel target has no parent",
            ),
            target.to_path_buf(),
        )
    })?;
    let source_name = source.file_name().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel source has no name",
            ),
            source.to_path_buf(),
        )
    })?;
    let target_name = target.file_name().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel target has no name",
            ),
            target.to_path_buf(),
        )
    })?;
    bootstrap_ensure_geometry_directory(store_root, target_parent)?;
    sync_dir(target_parent).map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
    let source_parent_handle = bootstrap_open_geometry_parent(store_root, source_parent)?;
    let target_parent_handle = bootstrap_open_geometry_parent(store_root, target_parent)?;
    inject_geometry_move_target_collision_for_test(target, directory)?;
    if !bootstrap_validate_path_kind(store_root, source, directory)?
        || checked_geometry_file_identity(
            &fs::symlink_metadata(source)
                .map_err(|error| Error::IO(error, source.to_path_buf()))?,
            source,
        )? != identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "primary relabel source identity changed before bootstrap rename",
            ),
            source.to_path_buf(),
        ));
    }
    rename_geometry_path_noreplace_at(
        &source_parent_handle,
        source_name,
        &target_parent_handle,
        target_name,
    )
    .map_err(|error| Error::IO(error, source.to_path_buf()))?;
    if !bootstrap_validate_path_kind(store_root, target, directory)?
        || checked_geometry_file_identity(
            &fs::symlink_metadata(target)
                .map_err(|error| Error::IO(error, target.to_path_buf()))?,
            target,
        )? != identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "primary relabel target identity changed during bootstrap recovery",
            ),
            target.to_path_buf(),
        ));
    }
    source_parent_handle
        .sync_all()
        .map_err(|error| Error::IO(error, source_parent.to_path_buf()))?;
    if source.parent() != target.parent() {
        target_parent_handle
            .sync_all()
            .map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
    }
    Ok(true)
}

fn bootstrap_preflight_geometry_path(
    store_root: &Path,
    source: &Path,
    target: &Path,
    directory: bool,
) -> Result<()> {
    if source == target {
        return bootstrap_validate_path_kind(store_root, source, directory)?
            .then_some(())
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "unchanged primary relabel path is missing during bootstrap recovery",
                    ),
                    source.to_path_buf(),
                )
            });
    }
    let source_exists = bootstrap_validate_path_kind(store_root, source, directory)?;
    let target_exists = bootstrap_validate_path_kind(store_root, target, directory)?;
    match (source_exists, target_exists) {
        (false, false) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "neither primary relabel path exists during bootstrap recovery",
                ),
                source.to_path_buf(),
            ));
        }
        (true, true) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "both primary relabel paths exist during bootstrap recovery",
                ),
                target.to_path_buf(),
            ));
        }
        (true, false) | (false, true) => {}
    }
    bootstrap_validate_existing_ancestors(store_root, source)?;
    bootstrap_validate_existing_ancestors(store_root, target)
}

fn bootstrap_move_geometry_binding(
    store_root: &Path,
    source: &LaneGeometryBinding,
    target: &LaneGeometryBinding,
) -> Result<()> {
    let source_blocks = store_root.join(&source.blocks_path);
    let target_blocks = store_root.join(&target.blocks_path);
    let source_merge = store_root.join(&source.merge_path);
    let target_merge = store_root.join(&target.merge_path);
    bootstrap_preflight_geometry_path(store_root, &source_blocks, &target_blocks, true)?;
    bootstrap_preflight_geometry_path(store_root, &source_merge, &target_merge, false)?;

    let rollback = || -> Result<()> {
        let merge_result =
            bootstrap_move_geometry_path(store_root, &target_merge, &source_merge, false);
        let blocks_result =
            bootstrap_move_geometry_path(store_root, &target_blocks, &source_blocks, true);
        match (merge_result, blocks_result) {
            (Ok(_), Ok(_)) => Ok(()),
            (merge, blocks) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel rollback failed (merge: {merge:?}; blocks: {blocks:?})"
                )),
                source_blocks.clone(),
            )),
        }
    };

    if let Err(error) =
        bootstrap_move_geometry_path(store_root, &source_blocks, &target_blocks, true)
    {
        return match rollback() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel block move failed ({error}); rollback failed ({rollback_error})"
                )),
                source_blocks,
            )),
        };
    }
    match bootstrap_move_geometry_path(store_root, &source_merge, &target_merge, false) {
        Ok(_) => Ok(()),
        Err(error) => match rollback() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel merge move failed ({error}); block-directory rollback failed ({rollback_error})"
                )),
                source_blocks,
            )),
        },
    }
}

fn bootstrap_require_lane_marker(
    store_root: &Path,
    blocks: &Path,
    binding: &LaneGeometryBinding,
) -> Result<()> {
    let path = blocks.join(MARKER_FILE_NAME);
    let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
        store_root,
        &path,
        blocks,
        usize::try_from(MAX_LANE_MARKER_BYTES)?,
    )?
    else {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::NotFound,
                "durable primary binding has no incarnation marker",
            ),
            path,
        ));
    };
    let mut cursor = bytes.as_slice();
    let marker = LaneIncarnationMarker::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
    if marker.encode() != bytes
        || marker.version != MARKER_VERSION
        || marker.lane_id != binding.lane_id
        || marker.incarnation != binding.incarnation
        || marker.activation_height != binding.activation_height
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "durable primary binding marker does not match its journal identity",
            ),
            path,
        ));
    }
    Ok(())
}
