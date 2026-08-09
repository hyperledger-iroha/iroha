// Bootstrap geometry path validation and synchronization helpers.

fn bootstrap_ensure_geometry_directory(store_root: &Path, directory: &Path) -> Result<()> {
    let relative = directory.strip_prefix(store_root).map_err(|_| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidInput,
            "bootstrap geometry directory escapes the Kura store root",
        )
    })?;
    validate_relative_path(relative)?;
    let mut cursor = store_root.to_path_buf();
    for component in relative.components() {
        let parent = cursor.clone();
        let parent_before =
            fs::symlink_metadata(&parent).map_err(|error| Error::IO(error, parent.clone()))?;
        if parent_before.file_type().is_symlink() || !parent_before.is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry parent is not a non-symlink directory",
                ),
                parent,
            ));
        }
        cursor.push(component.as_os_str());
        match fs::create_dir(&cursor) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Error::IO(error, cursor)),
        }
        if !bootstrap_validate_path_kind(store_root, &cursor, true)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "bootstrap geometry directory disappeared after creation",
                ),
                cursor,
            ));
        }
        let parent_after =
            fs::symlink_metadata(&parent).map_err(|error| Error::IO(error, parent.clone()))?;
        if checked_geometry_file_identity(&parent_before, &parent)?
            != checked_geometry_file_identity(&parent_after, &parent)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry parent changed during child creation",
                ),
                parent,
            ));
        }
        sync_dir(&parent).map_err(|error| Error::IO(error, parent))?;
    }
    Ok(())
}

fn bootstrap_sync_geometry_path(store_root: &Path, path: &Path, directory: bool) -> Result<()> {
    let before =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !bootstrap_validate_path_kind(store_root, path, directory)? {
        return Err(Error::IO(
            std::io::Error::new(ErrorKind::NotFound, "bootstrap geometry source is missing"),
            path.to_path_buf(),
        ));
    }
    if directory {
        sync_dir(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    } else {
        let file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if checked_geometry_file_identity(&before, path)?
            != checked_geometry_file_identity(&opened, path)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry file changed while opening",
                ),
                path.to_path_buf(),
            ));
        }
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    }
    let after = fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if checked_geometry_file_identity(&before, path)?
        != checked_geometry_file_identity(&after, path)?
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry path changed while synchronizing",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
}

fn bootstrap_open_geometry_parent(store_root: &Path, parent: &Path) -> Result<File> {
    if !bootstrap_validate_path_kind(store_root, parent, true)? {
        return Err(Error::IO(
            std::io::Error::new(ErrorKind::NotFound, "bootstrap geometry parent is missing"),
            parent.to_path_buf(),
        ));
    }
    let before =
        fs::symlink_metadata(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let directory = File::open(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let opened = directory
        .metadata()
        .map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let after =
        fs::symlink_metadata(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    if before.file_type().is_symlink()
        || !before.is_dir()
        || checked_geometry_file_identity(&before, parent)?
            != checked_geometry_file_identity(&opened, parent)?
        || checked_geometry_file_identity(&before, parent)?
            != checked_geometry_file_identity(&after, parent)?
        || !bootstrap_validate_path_kind(store_root, parent, true)?
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry parent changed while being opened",
            ),
            parent.to_path_buf(),
        ));
    }
    Ok(directory)
}
