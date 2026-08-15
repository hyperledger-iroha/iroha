//! Stable, bounded file reads for persistent `SoraNet` security snapshots.
#![allow(unexpected_cfgs)]
use std::{
    fs,
    io::{self, Read as _, Seek as _, Write},
    path::Path,
};
use tempfile::NamedTempFile;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!(
    "SoraNet snapshot loading requires a defined no-follow open flag on this Unix target"
);
/// Read a missing or stable direct regular file under an exact byte ceiling.
pub(super) fn read_optional_bounded_regular_file(
    path: &Path,
    max_bytes: usize,
    subject: &'static str,
) -> io::Result<Option<Vec<u8>>> {
    read_optional_bounded_regular_file_with_hook(path, max_bytes, subject, || {})
}
/// Create a unique temporary snapshot beside `destination`.
///
/// The random create-new name avoids stale crash files blocking future writes;
/// dropping the returned handle removes an uncommitted file automatically.
pub(super) fn create_temporary_direct_regular_file(
    destination: &Path,
    subject: &'static str,
) -> io::Result<NamedTempFile> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file = tempfile::Builder::new()
        .prefix(".iroha-soranet-snapshot-")
        .tempfile_in(parent)?;
    validate_direct_regular_file(&file.as_file().metadata()?, subject)?;
    Ok(file)
}
/// Atomically replace `destination` with a completed temporary snapshot.
pub(super) fn persist_temporary_snapshot(
    file: NamedTempFile,
    destination: &Path,
) -> io::Result<fs::File> {
    file.persist(destination).map_err(|error| error.error)
}
/// Writer that rejects a snapshot before it exceeds its read-side byte limit.
pub(super) struct BoundedWriter<W> {
    inner: W,
    max_bytes: usize,
    written: usize,
    subject: &'static str,
}
impl<W> BoundedWriter<W> {
    pub(super) const fn new(inner: W, max_bytes: usize, subject: &'static str) -> Self {
        Self {
            inner,
            max_bytes,
            written: 0,
            subject,
        }
    }
    pub(super) fn into_inner(self) -> W {
        self.inner
    }
}
impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max_bytes.saturating_sub(self.written) {
            return Err(too_large(self.subject, self.max_bytes));
        }
        let written = self.inner.write(bytes)?;
        self.written = self.written.checked_add(written).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} byte count overflowed this platform", self.subject),
            )
        })?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
fn read_optional_bounded_regular_file_with_hook(
    path: &Path,
    max_bytes: usize,
    subject: &'static str,
    after_read: impl FnOnce(),
) -> io::Result<Option<Vec<u8>>> {
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{subject} byte limit cannot be represented as u64"),
        )
    })?;
    let named_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    validate_direct_regular_file(&named_before, subject)?;
    if named_before.len() > max_bytes_u64 {
        return Err(too_large(subject, max_bytes));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(SNAPSHOT_O_NOFOLLOW_FLAG);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    validate_direct_regular_file(&opened_before, subject)?;
    if !metadata_identifies_same_file(&named_before, &opened_before)
        || opened_before.len() > max_bytes_u64
    {
        return Err(changed(subject, "while opening"));
    }
    let expected_len = usize::try_from(opened_before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} length cannot be addressed on this platform"),
        )
    })?;
    let read_capacity = expected_len.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} read limit overflows this platform"),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(read_capacity).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            format!("failed to reserve {read_capacity} bytes for {subject}"),
        )
    })?;
    bytes.resize(read_capacity, 0);
    let mut observed = 0usize;
    while observed < read_capacity {
        let read = file.read(&mut bytes[observed..])?;
        if read == 0 {
            break;
        }
        observed = observed.checked_add(read).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{subject} byte count overflowed this platform"),
            )
        })?;
    }
    bytes.truncate(observed);
    if bytes.len() > max_bytes {
        return Err(too_large(subject, max_bytes));
    }
    after_read();
    file.rewind()?;
    let mut compared = 0usize;
    let verification_limit = bytes.len().checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} verification limit overflows this platform"),
        )
    })?;
    let mut comparison = [0_u8; 8 * 1024];
    while compared < verification_limit {
        let remaining = verification_limit - compared;
        let chunk_len = remaining.min(comparison.len());
        let read = file.read(&mut comparison[..chunk_len])?;
        if read == 0 {
            break;
        }
        let Some(expected) = bytes.get(compared..compared.saturating_add(read)) else {
            return Err(changed(subject, "during verification reread"));
        };
        if expected != &comparison[..read] {
            return Err(changed(subject, "during verification reread"));
        }
        compared = compared.checked_add(read).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{subject} verification byte count overflowed this platform"),
            )
        })?;
    }
    if compared != bytes.len() {
        return Err(changed(subject, "during verification reread"));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    let observed_len = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} byte count cannot be represented as u64"),
        )
    })?;
    validate_direct_regular_file(&opened_after, subject)?;
    validate_direct_regular_file(&named_after, subject)?;
    if !metadata_identifies_same_file(&opened_before, &opened_after)
        || !metadata_identifies_same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != named_after.len()
        || opened_after.len() != observed_len
    {
        return Err(changed(subject, "while being read"));
    }
    Ok(Some(bytes))
}
fn validate_direct_regular_file(metadata: &fs::Metadata, subject: &'static str) -> io::Result<()> {
    if metadata_is_link(metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} must be a direct regular file"),
        ));
    }
    Ok(())
}
fn too_large(subject: &'static str, max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{subject} exceeds its {max_bytes}-byte limit"),
    )
}
fn changed(subject: &'static str, phase: &'static str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{subject} changed identity, type, or length {phase}"),
    )
}
fn metadata_is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}
#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}
#[cfg(not(any(unix, windows)))]
fn metadata_identifies_same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    #[test]
    fn bounded_reader_accepts_exact_limit_and_rejects_one_more_byte() {
        let directory = tempdir().expect("temporary directory");
        let exact = directory.path().join("exact.snapshot");
        let oversized = directory.path().join("oversized.snapshot");
        fs::write(&exact, [0x11; 8]).expect("write exact snapshot");
        fs::write(&oversized, [0x22; 9]).expect("write oversized snapshot");
        assert_eq!(
            read_optional_bounded_regular_file(&exact, 8, "test snapshot")
                .expect("read exact snapshot")
                .expect("snapshot exists"),
            [0x11; 8]
        );
        let error = read_optional_bounded_regular_file(&oversized, 8, "test snapshot")
            .expect_err("one extra byte must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("8-byte limit"));
    }
    #[test]
    fn bounded_writer_accepts_exact_limit_and_rejects_one_more_byte() {
        let mut exact = BoundedWriter::new(Vec::new(), 8, "test snapshot");
        exact.write_all(&[0x88; 8]).expect("write exact snapshot");
        assert_eq!(exact.into_inner(), [0x88; 8]);
        let mut oversized = BoundedWriter::new(Vec::new(), 8, "test snapshot");
        oversized.write_all(&[0x99; 8]).expect("write exact prefix");
        let error = oversized
            .write_all(&[0x99])
            .expect_err("one extra byte must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("8-byte limit"));
    }
    #[test]
    fn temporary_snapshot_replaces_destination_and_cleans_up_when_dropped() {
        let directory = tempdir().expect("temporary directory");
        let destination = directory.path().join("ledger.norito");
        fs::write(&destination, b"old").expect("write old snapshot");
        let mut replacement =
            create_temporary_direct_regular_file(&destination, "temporary replay snapshot")
                .expect("create replacement");
        replacement
            .write_all(b"new")
            .expect("write replacement snapshot");
        replacement
            .as_file()
            .sync_all()
            .expect("sync replacement snapshot");
        persist_temporary_snapshot(replacement, &destination).expect("replace prior snapshot");
        assert_eq!(fs::read(&destination).expect("read replacement"), b"new");
        let abandoned =
            create_temporary_direct_regular_file(&destination, "temporary replay snapshot")
                .expect("create abandoned snapshot");
        let abandoned_path = abandoned.path().to_owned();
        drop(abandoned);
        assert!(
            !abandoned_path.exists(),
            "dropping an uncommitted snapshot must remove it"
        );
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_allows_symbolic_link_in_parent_path() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target_directory = directory.path().join("target");
        let linked_directory = directory.path().join("linked");
        fs::create_dir(&target_directory).expect("create target directory");
        symlink(&target_directory, &linked_directory).expect("link parent directory");
        fs::write(target_directory.join("ledger.snapshot"), [0x33; 4])
            .expect("write target snapshot");
        assert_eq!(
            read_optional_bounded_regular_file(
                &linked_directory.join("ledger.snapshot"),
                8,
                "test snapshot",
            )
            .expect("read through linked parent")
            .expect("snapshot exists"),
            [0x33; 4]
        );
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target = directory.path().join("target.snapshot");
        let link = directory.path().join("link.snapshot");
        fs::write(&target, [0x33; 4]).expect("write target");
        symlink(&target, &link).expect("create symlink");
        let error = read_optional_bounded_regular_file(&link, 8, "test snapshot")
            .expect_err("symbolic link must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("direct regular file"));
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_path_replacement_during_read() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("ledger.snapshot");
        let replacement = directory.path().join("replacement.snapshot");
        fs::write(&path, [0x44; 4]).expect("write initial snapshot");
        fs::write(&replacement, [0x55; 4]).expect("write replacement snapshot");
        let error = read_optional_bounded_regular_file_with_hook(&path, 8, "test snapshot", || {
            fs::rename(&replacement, &path).expect("replace path after read")
        })
        .expect_err("path replacement must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("changed identity"));
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_same_length_in_place_rewrite() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("ledger.snapshot");
        fs::write(&path, [0x66; 4]).expect("write initial snapshot");
        let error = read_optional_bounded_regular_file_with_hook(&path, 8, "test snapshot", || {
            fs::write(&path, [0x77; 4]).expect("rewrite path after first read")
        })
        .expect_err("same-length rewrite must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("verification reread"));
    }
}
