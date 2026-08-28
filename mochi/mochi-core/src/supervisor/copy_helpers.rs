fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut remaining_tree_entries = SNAPSHOT_TREE_ENTRY_LIMIT_V1;
    copy_dir_recursive_with_limits(
        src,
        dst,
        0,
        SNAPSHOT_TREE_MAX_DEPTH_V1,
        SNAPSHOT_DIRECTORY_ENTRY_LIMIT_V1,
        &mut remaining_tree_entries,
    )
}

fn copy_dir_recursive_with_limits(
    src: &Path,
    dst: &Path,
    depth: usize,
    max_depth: usize,
    directory_entry_limit: usize,
    remaining_tree_entries: &mut usize,
) -> std::io::Result<()> {
    if directory_entry_limit == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot copy directory entry limit must be positive",
        ));
    }
    if depth == 0 && *remaining_tree_entries == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot copy tree entry limit must be positive",
        ));
    }
    if depth > max_depth {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("snapshot copy exceeds the V1 {max_depth}-level directory-depth limit"),
        ));
    }
    let source_metadata = fs::symlink_metadata(src)?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot copy source `{}` must be a real directory",
                src.display()
            ),
        ));
    }
    match fs::symlink_metadata(dst) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "snapshot copy destination `{}` already exists",
                    dst.display()
                ),
            ));
        }
        Err(error) => return Err(error),
    }
    fs::create_dir(dst)?;
    let copied = (|| {
        let mut directory_entries = 0_usize;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            if directory_entries == directory_entry_limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "snapshot copy directory `{}` exceeds the V1 {directory_entry_limit}-entry limit",
                        src.display()
                    ),
                ));
            }
            directory_entries = directory_entries.checked_add(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "snapshot copy directory entry count overflowed usize",
                )
            })?;
            *remaining_tree_entries = remaining_tree_entries.checked_sub(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "snapshot copy exceeds the V1 total tree-entry limit",
                )
            })?;
            let source = entry.path();
            let target = dst.join(entry.file_name());
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                copy_dir_recursive_with_limits(
                    &source,
                    &target,
                    depth.checked_add(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "snapshot copy directory depth overflowed usize",
                        )
                    })?,
                    max_depth,
                    directory_entry_limit,
                    remaining_tree_entries,
                )?;
            } else if file_type.is_file() {
                copy_snapshot_file(&source, &target)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "snapshot copy source contains unsupported entry `{}`",
                        source.display()
                    ),
                ));
            }
        }
        fs::set_permissions(dst, source_metadata.permissions())?;
        sync_managed_directory(dst)?;
        sync_managed_directory(dst.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "snapshot copy destination has no parent directory",
            )
        })?)
    })();
    if copied.is_err() {
        let _ = fs::remove_dir_all(dst);
        if let Some(parent) = dst.parent() {
            let _ = sync_managed_directory(parent);
        }
    }
    copied
}

fn copy_snapshot_file(src: &Path, dst: &Path) -> io::Result<()> {
    let named = fs::symlink_metadata(src)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot copy source `{}` must be a regular non-symlink file",
                src.display()
            ),
        ));
    }
    let mut source = open_existing_file_no_follow_nonblocking(src)?;
    let opened = source.metadata()?;
    if !snapshot_file_metadata_unchanged(&named, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("snapshot copy source `{}` changed while opening", src.display()),
        ));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(named.permissions().mode() & 0o777);
    let mut destination = options.open(dst)?;
    let copied = io::copy(&mut source, &mut destination)?;
    destination.sync_all()?;
    let opened_after = source.metadata()?;
    let named_after = fs::symlink_metadata(src)?;
    if copied != named.len()
        || named_after.file_type().is_symlink()
        || !snapshot_file_metadata_unchanged(&named, &opened_after)
        || !snapshot_file_metadata_unchanged(&named, &named_after)
    {
        let _ = fs::remove_file(dst);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("snapshot copy source `{}` changed while copying", src.display()),
        ));
    }
    Ok(())
}

fn sync_managed_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
