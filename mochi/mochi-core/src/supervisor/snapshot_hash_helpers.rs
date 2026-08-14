const SNAPSHOT_DIRECTORY_SORT_WINDOW_V1: usize = 4_096;
const SNAPSHOT_TREE_MAX_DEPTH_V1: usize = 64;
#[derive(Debug, Eq, PartialEq)]
struct SnapshotDirectoryEntry {
    name: std::ffi::OsString,
    path: PathBuf,
    is_directory: bool,
}
impl Ord for SnapshotDirectoryEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}
impl PartialOrd for SnapshotDirectoryEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
fn snapshot_directory_window_after(
    directory: &Path,
    cursor: Option<&OsStr>,
    window_entries: usize,
) -> io::Result<Vec<SnapshotDirectoryEntry>> {
    if window_entries == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot directory window must retain at least one entry",
        ));
    }
    let named = fs::symlink_metadata(directory)?;
    if named.file_type().is_symlink() || !named.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot directory `{}` is not a direct regular directory",
                directory.display()
            ),
        ));
    }
    let mut storage = Vec::new();
    storage.try_reserve_exact(window_entries).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "snapshot directory window allocation failed",
        )
    })?;
    let mut retained = std::collections::BinaryHeap::from(storage);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_str().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot storage contains a non-UTF-8 entry name",
            ));
        }
        if cursor.is_some_and(|cursor| name.as_os_str() <= cursor) {
            continue;
        }
        let file_type = entry.file_type()?;
        let is_directory = if file_type.is_dir() {
            true
        } else if file_type.is_file() {
            false
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "snapshot storage contains unsupported entry `{}`",
                    entry.path().display()
                ),
            ));
        };
        let candidate = SnapshotDirectoryEntry {
            name,
            path: entry.path(),
            is_directory,
        };
        if retained.len() < window_entries {
            retained.push(candidate);
        } else if retained.peek().is_some_and(|largest| candidate < *largest) {
            let _ = retained.pop();
            retained.push(candidate);
        }
    }
    let mut window = retained.into_vec();
    window.sort_unstable();
    Ok(window)
}
fn visit_snapshot_files_sorted(
    root: &Path,
    visit: &mut impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    fn visit_directory(
        directory: &Path,
        depth: usize,
        visit: &mut impl FnMut(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        if depth > SNAPSHOT_TREE_MAX_DEPTH_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "snapshot storage exceeds the V1 {SNAPSHOT_TREE_MAX_DEPTH_V1}-level directory-depth limit"
                ),
            ));
        }
        let mut cursor: Option<std::ffi::OsString> = None;
        loop {
            let window = snapshot_directory_window_after(
                directory,
                cursor.as_deref(),
                SNAPSHOT_DIRECTORY_SORT_WINDOW_V1,
            )?;
            if window.is_empty() {
                return Ok(());
            }
            let mut descend = None;
            for entry in window {
                cursor = Some(entry.name);
                if entry.is_directory {
                    // Discard the remainder of this bounded window before
                    // descending. The next scan resumes strictly after the
                    // directory name, so only one 4,096-entry window is ever
                    // resident across the complete tree.
                    descend = Some(entry.path);
                    break;
                }
                visit(&entry.path)?;
            }
            if let Some(directory) = descend {
                visit_directory(
                    &directory,
                    depth.checked_add(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "snapshot directory depth overflowed usize",
                        )
                    })?,
                    visit,
                )?;
                continue;
            }
        }
    }
    if !root.exists() {
        return Ok(());
    }
    visit_directory(root, 0, visit)
}
fn normalized_relative_path(base: &Path, path: &Path) -> io::Result<String> {
    let relative = path.strip_prefix(base).unwrap_or(path);
    let mut normalized = String::new();
    for (index, component) in relative.components().enumerate() {
        let component = component.as_os_str().to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot relative path is not canonical UTF-8",
            )
        })?;
        let separator = if index == 0 { 0_usize } else { 1_usize };
        normalized
            .try_reserve(separator.saturating_add(component.len()))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "snapshot relative path allocation failed",
                )
            })?;
        if separator != 0 {
            normalized.push('/');
        }
        normalized.push_str(&component);
    }
    Ok(normalized)
}
fn snapshot_file_metadata_unchanged(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    if !expected.is_file() || !observed.is_file() || expected.len() != observed.len() {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev() == observed.dev()
            && expected.ino() == observed.ino()
            && expected.mtime() == observed.mtime()
            && expected.mtime_nsec() == observed.mtime_nsec()
            && expected.ctime() == observed.ctime()
            && expected.ctime_nsec() == observed.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        expected.modified().ok() == observed.modified().ok()
    }
}
fn hash_snapshot_file_bounded(path: &Path, max_bytes: u64) -> io::Result<(Hash, u64)> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` is not a regular file",
                path.display()
            ),
        ));
    }
    if named.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` exceeds its {max_bytes}-byte limit",
                path.display()
            ),
        ));
    }
    let mut file = fs::File::open(path)?;
    let opened = file.metadata()?;
    if !snapshot_file_metadata_unchanged(&named, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` changed while opening",
                path.display()
            ),
        ));
    }
    let (hash, observed_bytes) = Hash::new_from_reader_bounded(&mut file, max_bytes)?;
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || observed_bytes != named.len()
        || !snapshot_file_metadata_unchanged(&named, &opened_after)
        || !snapshot_file_metadata_unchanged(&named, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` changed while hashing",
                path.display()
            ),
        ));
    }
    Ok((hash, observed_bytes))
}
fn hash_snapshot_file(path: &Path) -> io::Result<(Hash, u64)> {
    hash_snapshot_file_bounded(path, u64::MAX)
}
fn regular_snapshot_files_equal(left: &Path, right: &Path) -> io::Result<bool> {
    const COMPARE_BUFFER_BYTES: usize = 64 * 1024;
    let left_named = fs::symlink_metadata(left)?;
    let right_named = fs::symlink_metadata(right)?;
    if left_named.file_type().is_symlink()
        || right_named.file_type().is_symlink()
        || !left_named.is_file()
        || !right_named.is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot comparison requires two regular non-symlink files",
        ));
    }
    if left_named.len() != right_named.len() {
        return Ok(false);
    }
    let mut left_file = fs::File::open(left)?;
    let mut right_file = fs::File::open(right)?;
    let left_opened = left_file.metadata()?;
    let right_opened = right_file.metadata()?;
    if !snapshot_file_metadata_unchanged(&left_named, &left_opened)
        || !snapshot_file_metadata_unchanged(&right_named, &right_opened)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot comparison input changed while opening",
        ));
    }
    let mut left_buffer = [0_u8; COMPARE_BUFFER_BYTES];
    let mut right_buffer = [0_u8; COMPARE_BUFFER_BYTES];
    let mut remaining = left_named.len();
    while remaining != 0 {
        let chunk = usize::try_from(remaining.min(COMPARE_BUFFER_BYTES as u64))
            .expect("bounded snapshot comparison chunk fits usize");
        left_file.read_exact(&mut left_buffer[..chunk])?;
        right_file.read_exact(&mut right_buffer[..chunk])?;
        if left_buffer[..chunk] != right_buffer[..chunk] {
            return Ok(false);
        }
        remaining -= chunk as u64;
    }
    let mut left_probe = [0_u8; 1];
    let mut right_probe = [0_u8; 1];
    if left_file.read(&mut left_probe)? != 0 || right_file.read(&mut right_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot comparison input grew while reading",
        ));
    }
    let left_opened_after = left_file.metadata()?;
    let right_opened_after = right_file.metadata()?;
    let left_named_after = fs::symlink_metadata(left)?;
    let right_named_after = fs::symlink_metadata(right)?;
    if left_named_after.file_type().is_symlink()
        || right_named_after.file_type().is_symlink()
        || !snapshot_file_metadata_unchanged(&left_named, &left_opened_after)
        || !snapshot_file_metadata_unchanged(&left_named, &left_named_after)
        || !snapshot_file_metadata_unchanged(&right_named, &right_opened_after)
        || !snapshot_file_metadata_unchanged(&right_named, &right_named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot comparison input changed while reading",
        ));
    }
    Ok(true)
}
fn read_snapshot_file_bounded(path: &Path, max_bytes: usize) -> io::Result<Vec<u8>> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` is not a regular file",
                path.display()
            ),
        ));
    }
    if named.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact `{}` exceeds its {max_bytes}-byte limit",
                path.display()
            ),
        ));
    }
    let expected_len = usize::try_from(named.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot artifact length does not fit usize",
        )
    })?;
    let mut file = fs::File::open(path)?;
    let opened = file.metadata()?;
    if !snapshot_file_metadata_unchanged(&named, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot artifact changed while opening",
        ));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(expected_len).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "snapshot artifact allocation failed",
        )
    })?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes)?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot artifact grew while reading",
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || !snapshot_file_metadata_unchanged(&named, &opened_after)
        || !snapshot_file_metadata_unchanged(&named, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot artifact changed while reading",
        ));
    }
    Ok(bytes)
}
fn hash_directory(root: &Path) -> io::Result<Hash> {
    Hash::new_from_writer(|writer| {
        visit_snapshot_files_sorted(root, &mut |file| {
            let rel = normalized_relative_path(root, file)?;
            let (file_hash, file_bytes) = hash_snapshot_file(file)?;
            writer.write_all(
                &u64::try_from(rel.len())
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "snapshot path length overflow")
                    })?
                    .to_le_bytes(),
            )?;
            writer.write_all(rel.as_bytes())?;
            writer.write_all(&file_bytes.to_le_bytes())?;
            writer.write_all(file_hash.as_ref())?;
            Ok(())
        })
    })
}
#[cfg(test)]
mod snapshot_hash_helper_tests {
    use super::*;
    #[test]
    fn snapshot_file_hash_streams_without_changing_digest() {
        let temp = tempfile::tempdir().expect("temporary snapshot root");
        let path = temp.path().join("multi-chunk.bin");
        let bytes = (0..192 * 1024 + 17)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&path, &bytes).expect("write multi-chunk snapshot artifact");
        let (observed, observed_bytes) =
            hash_snapshot_file(&path).expect("stream snapshot artifact hash");
        assert_eq!(observed, Hash::new(&bytes));
        assert_eq!(observed_bytes, bytes.len() as u64);
    }
    #[test]
    fn snapshot_file_comparison_streams_and_detects_a_late_difference() {
        let temp = tempfile::tempdir().expect("temporary snapshot root");
        let left = temp.path().join("left.bin");
        let right = temp.path().join("right.bin");
        let mut bytes = vec![0x5A_u8; 192 * 1024 + 17];
        fs::write(&left, &bytes).expect("write left snapshot artifact");
        fs::write(&right, &bytes).expect("write matching right snapshot artifact");
        assert!(regular_snapshot_files_equal(&left, &right).expect("compare matching files"));
        *bytes.last_mut().expect("fixture is not empty") ^= 0xFF;
        fs::write(&right, &bytes).expect("write differing right snapshot artifact");
        assert!(!regular_snapshot_files_equal(&left, &right).expect("compare differing files"));
    }
    #[test]
    fn bounded_snapshot_reader_accepts_exact_and_rejects_max_plus_one() {
        let temp = tempfile::tempdir().expect("temporary snapshot root");
        let path = temp.path().join("metadata.json");
        let expected = [0x2A_u8; 32];
        fs::write(&path, expected).expect("write exact snapshot artifact");
        assert_eq!(
            read_snapshot_file_bounded(&path, expected.len()).expect("read exact artifact"),
            expected
        );
        fs::write(&path, [0x2A_u8; 33]).expect("write oversized snapshot artifact");
        let error = read_snapshot_file_bounded(&path, expected.len())
            .expect_err("max plus one must reject before allocation");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    #[test]
    fn directory_hash_streams_the_canonical_sorted_records() {
        let temp = tempfile::tempdir().expect("temporary snapshot root");
        let nested = temp.path().join("nested");
        fs::create_dir(&nested).expect("create nested snapshot directory");
        fs::write(temp.path().join("z.bin"), b"zeta").expect("write root artifact");
        fs::write(nested.join("a.bin"), b"alpha").expect("write nested artifact");
        let mut canonical = Vec::new();
        for (relative, bytes) in [("nested/a.bin", b"alpha".as_slice()), ("z.bin", b"zeta")] {
            canonical.extend_from_slice(&(relative.len() as u64).to_le_bytes());
            canonical.extend_from_slice(relative.as_bytes());
            canonical.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            canonical.extend_from_slice(Hash::new(&bytes).as_ref());
        }
        assert_eq!(
            hash_directory(temp.path()).expect("stream directory digest"),
            Hash::new(canonical)
        );
    }
    #[test]
    fn directory_window_retains_only_the_smallest_bounded_suffix() {
        let temp = tempfile::tempdir().expect("temporary snapshot root");
        for name in ["d", "b", "a", "c"] {
            fs::write(temp.path().join(name), name).expect("write window fixture");
        }
        let first = snapshot_directory_window_after(temp.path(), None, 3)
            .expect("select first bounded directory window");
        assert_eq!(
            first
                .iter()
                .map(|entry| entry.name.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let second = snapshot_directory_window_after(temp.path(), Some(OsStr::new("c")), 3)
            .expect("select next bounded directory window");
        assert_eq!(
            second
                .iter()
                .map(|entry| entry.name.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["d"]
        );
    }
}
