//! Bounded local-NTFS directory planning through original read-only native owners.
//!
//! Each file remains immutable while it is read, and each directory's metadata and exact entry
//! names are checked around its enumeration and descendants. Earlier closed files are not retained
//! until the end of the scan; this is the existing per-file planning boundary, not an atomic
//! snapshot of the entire tree.

use super::*;
use iroha_fs::{FileSnapshot, ReaderDirectory};
use std::fs;

pub(super) fn next_entry_count(current: usize) -> Result<usize, CarPlanError> {
    let count = current
        .checked_add(1)
        .ok_or(CarPlanError::TooManyDirectoryEntries {
            count: usize::MAX,
            maximum: CAR_DIRECTORY_MAX_ENTRIES,
        })?;
    if count > CAR_DIRECTORY_MAX_ENTRIES {
        return Err(CarPlanError::TooManyDirectoryEntries {
            count,
            maximum: CAR_DIRECTORY_MAX_ENTRIES,
        });
    }
    Ok(count)
}

pub(super) fn next_file_count(current: usize) -> Result<usize, CarPlanError> {
    let count = current.checked_add(1).ok_or(CarPlanError::TooManyFiles {
        count: usize::MAX,
        maximum: CAR_PLAN_MAX_FILES,
    })?;
    ensure_plan_file_count(count)?;
    Ok(count)
}

pub(super) fn add_payload_bytes(current: u64, additional: u64) -> Result<u64, CarPlanError> {
    let bytes = current
        .checked_add(additional)
        .ok_or(CarPlanError::DirectoryPayloadTooLarge {
            bytes: u64::MAX,
            maximum: CAR_EAGER_DIRECTORY_MAX_BYTES,
        })?;
    if bytes > CAR_EAGER_DIRECTORY_MAX_BYTES {
        return Err(CarPlanError::DirectoryPayloadTooLarge {
            bytes,
            maximum: CAR_EAGER_DIRECTORY_MAX_BYTES,
        });
    }
    Ok(bytes)
}

pub(super) fn validate_component(
    name: &str,
    logical_parent: &[String],
    native_path: &Path,
) -> Result<(), CarPlanError> {
    if !is_portable_normal_component(name) || name.len() > CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES {
        return Err(CarPlanError::InvalidPath(format_path(native_path)));
    }
    if logical_parent.len() >= CAR_LOGICAL_PATH_MAX_COMPONENTS {
        return Err(CarPlanError::InvalidPath(format!(
            "directory tree exceeds {CAR_LOGICAL_PATH_MAX_COMPONENTS} components at {}",
            native_path.display()
        )));
    }
    let logical_bytes = logical_parent
        .iter()
        .try_fold(name.len(), |total, component| {
            total
                .checked_add(component.len())
                .and_then(|value| value.checked_add(1))
        });
    if logical_bytes.is_none_or(|bytes| bytes > CAR_LOGICAL_PATH_MAX_BYTES) {
        return Err(CarPlanError::InvalidPath(format!(
            "logical path exceeds {CAR_LOGICAL_PATH_MAX_BYTES} bytes at {}",
            native_path.display()
        )));
    }
    Ok(())
}

fn require_directory_snapshot(
    directory: &ReaderDirectory,
    expected: FileSnapshot,
) -> Result<(), CarPlanError> {
    let current = directory
        .snapshot()
        .map_err(|error| directory_plan_io(directory.path(), error))?;
    if current != expected {
        return Err(CarPlanError::InvalidPath(format!(
            "directory changed during scan: {}",
            directory.path().display()
        )));
    }
    directory
        .revalidate()
        .map_err(|error| directory_plan_io(directory.path(), error))
}

pub(super) fn sort_directory_names(names: &mut [String], path: &Path) -> Result<(), CarPlanError> {
    // UTF-8 byte order preserves exact spelling; do not fold native names or accept repeated
    // entries if a changing namespace produces duplicates during the initial enumeration.
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CarPlanError::InvalidPath(format!(
            "duplicate directory entry during scan: {}",
            path.display()
        )));
    }
    Ok(())
}

fn require_directory_inventory(
    directory: &ReaderDirectory,
    expected: &[String],
) -> Result<(), CarPlanError> {
    // The existing entry limit bounds this vector. Reuse the admitted sorted names rather
    // than allocating a second name inventory; each observed name must appear exactly once.
    let mut seen = Vec::new();
    try_reserve_plan(
        &mut seen,
        expected.len(),
        "native directory inventory recheck",
    )?;
    seen.resize(expected.len(), false);
    let changed = || {
        CarPlanError::InvalidPath(format!(
            "directory entries changed during scan: {}",
            directory.path().display()
        ))
    };
    for entry in fs::read_dir(directory.path())
        .map_err(|error| directory_plan_io(directory.path(), error))?
    {
        let entry = entry.map_err(|error| directory_plan_io(directory.path(), error))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(changed)?;
        let index = expected
            .binary_search_by(|prior| prior.as_str().cmp(name))
            .map_err(|_| changed())?;
        if seen[index] {
            return Err(changed());
        }
        seen[index] = true;
    }
    if seen.iter().any(|found| !found) {
        return Err(changed());
    }
    Ok(())
}

struct NativeDirectoryScan<'a> {
    root: &'a ReaderDirectory,
    root_snapshot: FileSnapshot,
    files: Vec<FileEntry>,
    entry_count: usize,
    total_bytes: u64,
}

impl NativeDirectoryScan<'_> {
    fn validate_root(&self) -> Result<(), CarPlanError> {
        require_directory_snapshot(self.root, self.root_snapshot)
    }

    fn scan(
        &mut self,
        directory: &ReaderDirectory,
        logical_path: &mut Vec<String>,
        on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), CarPlanError> {
        self.validate_root()?;
        let before = directory
            .snapshot()
            .map_err(|error| directory_plan_io(directory.path(), error))?;
        // The retained owner excludes deletion of this original and every ancestor. The
        // pathname enumeration is fenced by exact directory snapshots and named-owner checks.
        let read_dir = fs::read_dir(directory.path())
            .map_err(|error| directory_plan_io(directory.path(), error))?;
        let mut names = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|error| directory_plan_io(directory.path(), error))?;
            self.entry_count = next_entry_count(self.entry_count)?;
            let name = entry.file_name();
            let path = directory.path().join(&name);
            let name = name
                .to_str()
                .ok_or_else(|| CarPlanError::NonUtf8Path(format_path(&path)))?;
            validate_component(name, logical_path, &path)?;
            try_reserve_plan(&mut names, 1, "native directory entries")?;
            names.push(try_owned_plan_string(name, "native directory entry name")?);
        }
        require_directory_snapshot(directory, before)?;
        self.validate_root()?;
        // Rust strings sort by UTF-8 bytes, matching canonical paths and the Unix scanner;
        // Windows OsString/UTF-16 ordering differs for supplementary-plane characters.
        sort_directory_names(&mut names, directory.path())?;
        for name in &names {
            let path = directory.path().join(name);
            let linked =
                fs::symlink_metadata(&path).map_err(|error| directory_plan_io(&path, error))?;
            if linked.file_type().is_symlink() {
                return Err(CarPlanError::InvalidPath(format_path(&path)));
            }
            try_reserve_plan(logical_path, 1, "native directory logical path")?;
            logical_path.push(try_owned_plan_string(
                name,
                "native directory path component",
            )?);
            // Path metadata selects the requested kind only. Every original is admitted by
            // iroha_fs, which rejects all reparse tags, unsafe custody and wrong object kinds.
            // A failed native open never falls back to an ordinary pathname reader.
            let result = if linked.is_dir() {
                directory
                    .open_child(name)
                    .map_err(|error| directory_plan_io(&path, error))
                    .and_then(|child| self.scan(&child, logical_path, on_file_opened))
            } else if linked.is_file() {
                self.read_file(directory, name, &path, logical_path, on_file_opened)
            } else {
                Err(CarPlanError::InvalidPath(format!(
                    "directory entry is not a regular file or directory: {}",
                    path.display()
                )))
            };
            logical_path.pop();
            result?;
        }
        require_directory_snapshot(directory, before)?;
        // Native directory metadata does not by itself prove that the enumerated names
        // stayed unchanged. Check exact membership while the same directory remains held.
        require_directory_inventory(directory, &names)?;
        require_directory_snapshot(directory, before)?;
        self.validate_root()
    }

    fn read_file(
        &mut self,
        directory: &ReaderDirectory,
        name: &str,
        path: &Path,
        logical_path: &[String],
        on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), CarPlanError> {
        let mut file = directory
            .open_retained_regular(name)
            .map_err(|error| directory_plan_io(path, error))?;
        let opened = file
            .snapshot()
            .map_err(|error| directory_plan_io(path, error))?;
        let length = opened.local_parts().2;
        next_file_count(self.files.len())?;
        let total_bytes = add_payload_bytes(self.total_bytes, length)?;
        on_file_opened(path).map_err(|error| directory_plan_io(path, error))?;
        let byte_count =
            usize::try_from(length).map_err(|_| CarPlanError::ContentLengthTooLarge)?;
        let mut data = Vec::new();
        try_reserve_plan(&mut data, byte_count, "native directory file payload")?;
        data.resize(byte_count, 0);
        file.file_mut()
            .read_exact(&mut data)
            .map_err(|error| directory_plan_io(path, error))?;
        let mut trailing = [0u8; 1];
        if file
            .file_mut()
            .read(&mut trailing)
            .map_err(|error| directory_plan_io(path, error))?
            != 0
        {
            return Err(CarPlanError::InvalidPath(format!(
                "directory file grew during scan: {}",
                path.display()
            )));
        }
        if file
            .snapshot()
            .map_err(|error| directory_plan_io(path, error))?
            != opened
        {
            return Err(CarPlanError::InvalidPath(format!(
                "directory file changed during scan: {}",
                path.display()
            )));
        }
        file.revalidate()
            .map_err(|error| directory_plan_io(path, error))?;
        self.validate_root()?;
        try_reserve_plan(&mut self.files, 1, "native directory file inventory")?;
        self.files.push(FileEntry {
            path: try_clone_logical_path(logical_path)?,
            data,
        });
        self.total_bytes = total_bytes;
        Ok(())
    }
}

pub(super) fn gather_files_secure(
    root: &Path,
    on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<Vec<FileEntry>, CarPlanError> {
    let directory = ReaderDirectory::open(root).map_err(|error| directory_plan_io(root, error))?;
    let root_snapshot = directory
        .snapshot()
        .map_err(|error| directory_plan_io(directory.path(), error))?;
    let mut scan = NativeDirectoryScan {
        root: &directory,
        root_snapshot,
        files: Vec::new(),
        entry_count: 0,
        total_bytes: 0,
    };
    let mut logical_path = Vec::new();
    try_reserve_plan(
        &mut logical_path,
        CAR_LOGICAL_PATH_MAX_COMPONENTS,
        "native directory logical path stack",
    )?;
    scan.scan(&directory, &mut logical_path, on_file_opened)?;
    scan.validate_root()?;
    Ok(scan.files)
}
