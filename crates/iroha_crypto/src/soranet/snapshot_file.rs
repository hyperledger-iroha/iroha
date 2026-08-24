//! Stable, bounded file reads for persistent `SoraNet` security snapshots.
#![allow(unexpected_cfgs)]
#[cfg(unix)]
use std::{ffi::OsString, path::Component};
use std::{
    fs,
    io::{self, Read as _, Seek as _, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
pub(super) const SNAPSHOT_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
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
/// Canonical, owner-custodied location pinned for one persistent ledger lifetime.
///
/// The configured path must be absolute. Existing parent aliases are resolved once, missing
/// descendants are created owner-private, and all later I/O uses the retained canonical path.
/// This prevents a mutable parent symlink from switching a running ledger between snapshots.
/// Persistent custody is currently supported only on Unix, where owner, mode,
/// and link-count policy can be enforced; other targets fail with
/// [`io::ErrorKind::Unsupported`].
#[derive(Debug)]
pub(super) struct CustodiedSnapshotPath {
    destination: PathBuf,
    parent_path: PathBuf,
    parent: fs::File,
    #[cfg(unix)]
    owner_uid: u32,
}
impl CustodiedSnapshotPath {
    /// Prepare and pin the parent directory for `destination`.
    pub(super) fn prepare(destination: &Path) -> io::Result<Self> {
        #[cfg(not(unix))]
        {
            let _ = destination;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "persistent SoraNet replay state requires Unix owner/mode/link custody checks",
            ))
        }
        #[cfg(unix)]
        {
            validate_absolute_ledger_path(destination)?;
            let file_name = destination.file_name().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "replay ledger path must name a file",
                )
            })?;
            let configured_parent = destination.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "replay ledger path must have a parent directory",
                )
            })?;
            let parent_path = prepare_canonical_parent(configured_parent)?;
            let destination = parent_path.join(file_name);
            let parent_named = fs::symlink_metadata(&parent_path)?;
            validate_direct_directory(&parent_named, "replay ledger parent")?;
            let parent = fs::File::open(&parent_path)?;
            let parent_opened = parent.metadata()?;
            validate_direct_directory(&parent_opened, "opened replay ledger parent")?;
            if !metadata_identifies_same_file(&parent_named, &parent_opened) {
                return Err(changed("replay ledger parent", "while opening"));
            }
            let owner_uid = owner_uid_for_writable_directory(&parent_path)?;
            validate_custodied_ancestor_chain(&parent_path, owner_uid, false)?;
            let custody = Self {
                destination,
                parent_path,
                parent,
                owner_uid,
            };
            custody.verify_parent()?;
            Ok(custody)
        }
    }
    pub(super) fn destination(&self) -> &Path {
        &self.destination
    }
    pub(super) fn verify_parent(&self) -> io::Result<()> {
        let named = fs::symlink_metadata(&self.parent_path)?;
        let opened = self.parent.metadata()?;
        validate_direct_directory(&named, "replay ledger parent")?;
        validate_direct_directory(&opened, "opened replay ledger parent")?;
        if !metadata_identifies_same_file(&named, &opened) {
            return Err(changed("replay ledger parent", "while in use"));
        }
        #[cfg(unix)]
        validate_custodied_ancestor_chain(&self.parent_path, self.owner_uid, false)?;
        Ok(())
    }
    pub(super) fn sync_parent(&self) -> io::Result<()> {
        self.verify_parent()?;
        self.parent.sync_all()?;
        self.verify_parent()
    }
    pub(super) fn validate_opened_file(
        &self,
        path: &Path,
        file: &fs::File,
        subject: &'static str,
    ) -> io::Result<()> {
        self.verify_parent()?;
        let named = fs::symlink_metadata(path)?;
        let opened = file.metadata()?;
        self.validate_private_file(&named, subject)?;
        self.validate_private_file(&opened, subject)?;
        if !metadata_identifies_same_file(&named, &opened) {
            return Err(changed(subject, "while opening"));
        }
        self.verify_parent()
    }
    fn validate_private_file(
        &self,
        metadata: &fs::Metadata,
        subject: &'static str,
    ) -> io::Result<()> {
        validate_direct_regular_file(metadata, subject)?;
        #[cfg(unix)]
        validate_owner_private_file(metadata, self.owner_uid, subject)?;
        Ok(())
    }
}
/// Read a missing or stable direct regular file under an exact byte ceiling.
pub(super) fn read_optional_bounded_regular_file(
    custody: &CustodiedSnapshotPath,
    max_bytes: usize,
    subject: &'static str,
) -> io::Result<Option<Vec<u8>>> {
    read_optional_bounded_regular_file_with_hook(custody, max_bytes, subject, || {})
}
/// Create a unique temporary snapshot beside `destination`.
///
/// The random create-new name avoids stale crash files blocking future writes;
/// dropping the returned handle removes an uncommitted file automatically.
pub(super) fn create_temporary_direct_regular_file(
    custody: &CustodiedSnapshotPath,
    subject: &'static str,
) -> io::Result<NamedTempFile> {
    custody.verify_parent()?;
    let file = tempfile::Builder::new()
        .prefix(".iroha-soranet-snapshot-")
        .tempfile_in(&custody.parent_path)?;
    custody.validate_private_file(&file.as_file().metadata()?, subject)?;
    custody.verify_parent()?;
    Ok(file)
}
/// Atomically replace `destination` with a completed temporary snapshot.
pub(super) fn persist_temporary_snapshot(
    file: NamedTempFile,
    custody: &CustodiedSnapshotPath,
    subject: &'static str,
) -> io::Result<fs::File> {
    custody.verify_parent()?;
    custody.validate_private_file(&file.as_file().metadata()?, subject)?;
    match fs::symlink_metadata(custody.destination()) {
        Ok(metadata) => custody.validate_private_file(&metadata, subject)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let persisted = file
        .persist(custody.destination())
        .map_err(|error| error.error)?;
    custody.validate_opened_file(custody.destination(), &persisted, subject)?;
    Ok(persisted)
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
#[cfg(unix)]
fn validate_absolute_ledger_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("replay ledger path must be absolute: {}", path.display()),
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "replay ledger path must not contain dot components: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn prepare_canonical_parent(configured_parent: &Path) -> io::Result<PathBuf> {
    let mut missing = Vec::<OsString>::new();
    let mut existing = configured_parent.to_path_buf();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "replay ledger parent has no existing ancestor: {}",
                            configured_parent.display()
                        ),
                    )
                })?;
                missing.push(name.to_os_string());
                existing = existing
                    .parent()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            "replay ledger parent has no existing ancestor",
                        )
                    })?
                    .to_path_buf();
            }
            Err(error) => return Err(error),
        }
    }
    let mut canonical = fs::canonicalize(&existing)?;
    let existing_metadata = fs::symlink_metadata(&canonical)?;
    validate_direct_directory(&existing_metadata, "replay ledger ancestor")?;
    #[cfg(unix)]
    {
        let owner_uid = owner_uid_for_writable_directory(&canonical)?;
        validate_custodied_ancestor_chain(&canonical, owner_uid, !missing.is_empty())?;
    }
    while let Some(component) = missing.pop() {
        canonical.push(component);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&canonical)?;
        let metadata = fs::symlink_metadata(&canonical)?;
        validate_direct_directory(&metadata, "created replay ledger parent")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let owner_uid = owner_uid_for_writable_directory(&canonical)?;
            if metadata.uid() != owner_uid || metadata.mode() & 0o077 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "created replay ledger parent must be owner-private: {}",
                        canonical.display()
                    ),
                ));
            }
        }
    }
    #[cfg(unix)]
    {
        let owner_uid = owner_uid_for_writable_directory(&canonical)?;
        validate_custodied_ancestor_chain(&canonical, owner_uid, false)?;
    }
    Ok(canonical)
}
fn validate_direct_directory(metadata: &fs::Metadata, subject: &'static str) -> io::Result<()> {
    if metadata_is_link(metadata) || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} must be a direct directory"),
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn owner_uid_for_writable_directory(directory: &Path) -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt as _;
    let probe = tempfile::Builder::new()
        .prefix(".iroha-soranet-owner-probe-")
        .tempfile_in(directory)?;
    let metadata = probe.as_file().metadata()?;
    validate_direct_regular_file(&metadata, "replay ledger owner probe")?;
    if metadata.nlink() != 1 || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "replay ledger owner probe was not created owner-private",
        ));
    }
    Ok(metadata.uid())
}
#[cfg(unix)]
fn validate_custodied_ancestor_chain(
    parent: &Path,
    owner_uid: u32,
    allow_planned_sticky_child: bool,
) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let mut ancestors = Vec::new();
    let mut cursor = parent;
    loop {
        ancestors.push(cursor.to_path_buf());
        let Some(next) = cursor.parent() else {
            break;
        };
        if next == cursor {
            break;
        }
        cursor = next;
    }
    ancestors.reverse();
    let mut metadata = Vec::new();
    metadata.try_reserve_exact(ancestors.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "failed to reserve replay ledger ancestor metadata",
        )
    })?;
    for ancestor in &ancestors {
        let observed = fs::symlink_metadata(ancestor)?;
        validate_direct_directory(&observed, "replay ledger ancestor")?;
        if observed.uid() != 0 && observed.uid() != owner_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "replay ledger ancestor {} is not owned by the process owner or root",
                    ancestor.display()
                ),
            ));
        }
        metadata.push(observed);
    }
    for (index, observed) in metadata.iter().enumerate() {
        if observed.mode() & 0o022 == 0 {
            continue;
        }
        let is_root_sticky_boundary = observed.uid() == 0 && observed.mode() & 0o1000 != 0;
        if !is_root_sticky_boundary {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "replay ledger ancestor {} is writable by another principal",
                    ancestors[index].display()
                ),
            ));
        }
        let protected_child = metadata
            .get(index + 1)
            .is_some_and(|child| child.uid() == owner_uid && child.mode() & 0o022 == 0);
        let planned_child = allow_planned_sticky_child && index + 1 == metadata.len();
        if !protected_child && !planned_child {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "sticky replay ledger ancestor {} requires an owner-held protected child directory",
                    ancestors[index].display()
                ),
            ));
        }
    }
    Ok(())
}
#[cfg(unix)]
fn validate_owner_private_file(
    metadata: &fs::Metadata,
    owner_uid: u32,
    subject: &'static str,
) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    if metadata.uid() != owner_uid || metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{subject} must be owner-private and have exactly one link"),
        ));
    }
    Ok(())
}
fn read_optional_bounded_regular_file_with_hook(
    custody: &CustodiedSnapshotPath,
    max_bytes: usize,
    subject: &'static str,
    after_read: impl FnOnce(),
) -> io::Result<Option<Vec<u8>>> {
    custody.verify_parent()?;
    let path = custody.destination();
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
    custody.validate_private_file(&named_before, subject)?;
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
    custody.validate_private_file(&opened_before, subject)?;
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
    let bytes = read_bounded_snapshot_bytes(&mut file, read_capacity, max_bytes, subject)?;
    after_read();
    verify_snapshot_bytes_unchanged(&mut file, &bytes, subject)?;
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    let observed_len = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{subject} byte count cannot be represented as u64"),
        )
    })?;
    if !metadata_identifies_same_file(&opened_before, &opened_after)
        || !metadata_identifies_same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != named_after.len()
        || opened_after.len() != observed_len
    {
        return Err(changed(subject, "while being read"));
    }
    custody.validate_private_file(&opened_after, subject)?;
    custody.validate_private_file(&named_after, subject)?;
    custody.verify_parent()?;
    Ok(Some(bytes))
}
fn read_bounded_snapshot_bytes(
    file: &mut fs::File,
    read_capacity: usize,
    max_bytes: usize,
    subject: &'static str,
) -> io::Result<Vec<u8>> {
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
    Ok(bytes)
}
fn verify_snapshot_bytes_unchanged(
    file: &mut fs::File,
    bytes: &[u8],
    subject: &'static str,
) -> io::Result<()> {
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
    Ok(())
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
#[cfg(not(unix))]
fn metadata_identifies_same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    // Persistent replay custody is unsupported without Unix owner/mode/link policy. Retain a
    // fail-closed fallback for methods that remain type-checked on those targets.
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use tempfile::tempdir;
    #[cfg(unix)]
    fn write_private(path: &Path, bytes: &[u8]) {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(path).expect("open private fixture");
        file.write_all(bytes).expect("write private fixture");
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_accepts_exact_limit_and_rejects_one_more_byte() {
        let directory = tempdir().expect("temporary directory");
        let exact = directory.path().join("exact.snapshot");
        let oversized = directory.path().join("oversized.snapshot");
        write_private(&exact, &[0x11; 8]);
        write_private(&oversized, &[0x22; 9]);
        let exact = CustodiedSnapshotPath::prepare(&exact).expect("exact custody");
        let oversized = CustodiedSnapshotPath::prepare(&oversized).expect("oversized custody");
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
    #[cfg(unix)]
    #[test]
    fn temporary_snapshot_replaces_destination_and_cleans_up_when_dropped() {
        let directory = tempdir().expect("temporary directory");
        let destination = directory.path().join("ledger.norito");
        write_private(&destination, b"old");
        let custody = CustodiedSnapshotPath::prepare(&destination).expect("snapshot custody");
        let mut replacement =
            create_temporary_direct_regular_file(&custody, "temporary replay snapshot")
                .expect("create replacement");
        replacement
            .write_all(b"new")
            .expect("write replacement snapshot");
        replacement
            .as_file()
            .sync_all()
            .expect("sync replacement snapshot");
        persist_temporary_snapshot(replacement, &custody, "test snapshot")
            .expect("replace prior snapshot");
        custody.sync_parent().expect("sync parent");
        assert_eq!(
            fs::read(custody.destination()).expect("read replacement"),
            b"new"
        );
        let abandoned = create_temporary_direct_regular_file(&custody, "temporary replay snapshot")
            .expect("create abandoned snapshot");
        let abandoned_path = abandoned.path().to_owned();
        drop(abandoned);
        assert!(
            !abandoned_path.exists(),
            "dropping an uncommitted snapshot must remove it"
        );
    }
    #[test]
    fn custody_requires_an_absolute_path() {
        let error = CustodiedSnapshotPath::prepare(Path::new("relative/ledger.norito"))
            .expect_err("relative security-state paths must fail");
        #[cfg(unix)]
        {
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("must be absolute"));
        }
        #[cfg(not(unix))]
        {
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        }
    }
    #[cfg(not(unix))]
    #[test]
    fn persistent_replay_custody_fails_closed_without_unix_policy() {
        let error = CustodiedSnapshotPath::prepare(Path::new("C:\\replay\\ledger.norito"))
            .expect_err("persistent replay state must require an implemented custody policy");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("requires Unix"));
    }
    #[cfg(unix)]
    #[test]
    fn custody_pins_a_canonical_parent_alias() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target_directory = directory.path().join("target");
        let alternate_directory = directory.path().join("alternate");
        let linked_directory = directory.path().join("linked");
        fs::create_dir(&target_directory).expect("create target directory");
        fs::create_dir(&alternate_directory).expect("create alternate directory");
        symlink(&target_directory, &linked_directory).expect("link parent directory");
        write_private(&target_directory.join("ledger.snapshot"), &[0x33; 4]);
        write_private(&alternate_directory.join("ledger.snapshot"), &[0x44; 4]);
        let custody = CustodiedSnapshotPath::prepare(&linked_directory.join("ledger.snapshot"))
            .expect("pin canonical target");
        fs::remove_file(&linked_directory).expect("remove parent alias");
        symlink(&alternate_directory, &linked_directory).expect("redirect parent alias");
        assert_eq!(
            read_optional_bounded_regular_file(&custody, 8, "test snapshot")
                .expect("read pinned canonical parent")
                .expect("snapshot exists"),
            [0x33; 4]
        );
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_symbolic_link_files() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target = directory.path().join("target.snapshot");
        let link = directory.path().join("link.snapshot");
        write_private(&target, &[0x33; 4]);
        symlink(&target, &link).expect("create symlink");
        let custody = CustodiedSnapshotPath::prepare(&link).expect("snapshot custody");
        let error = read_optional_bounded_regular_file(&custody, 8, "test snapshot")
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
        write_private(&path, &[0x44; 4]);
        write_private(&replacement, &[0x55; 4]);
        let custody = CustodiedSnapshotPath::prepare(&path).expect("snapshot custody");
        let destination = custody.destination().to_path_buf();
        let replacement = fs::canonicalize(&replacement).expect("canonical replacement");
        let error =
            read_optional_bounded_regular_file_with_hook(&custody, 8, "test snapshot", || {
                fs::rename(&replacement, &destination).expect("replace path after read")
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
        write_private(&path, &[0x66; 4]);
        let custody = CustodiedSnapshotPath::prepare(&path).expect("snapshot custody");
        let destination = custody.destination().to_path_buf();
        let error =
            read_optional_bounded_regular_file_with_hook(&custody, 8, "test snapshot", || {
                write_private(&destination, &[0x77; 4])
            })
            .expect_err("same-length rewrite must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("verification reread"));
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_non_private_and_multiply_linked_files() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempdir().expect("temporary directory");
        let public = directory.path().join("public.snapshot");
        write_private(&public, &[0x11; 4]);
        fs::set_permissions(&public, fs::Permissions::from_mode(0o644))
            .expect("make snapshot public");
        let public_custody = CustodiedSnapshotPath::prepare(&public).expect("public custody");
        let error = read_optional_bounded_regular_file(&public_custody, 8, "test snapshot")
            .expect_err("non-private snapshot must fail");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

        let linked = directory.path().join("linked.snapshot");
        write_private(&linked, &[0x22; 4]);
        fs::hard_link(&linked, directory.path().join("second-link.snapshot"))
            .expect("create second hard link");
        let linked_custody = CustodiedSnapshotPath::prepare(&linked).expect("linked custody");
        let error = read_optional_bounded_regular_file(&linked_custody, 8, "test snapshot")
            .expect_err("multiply-linked snapshot must fail");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
    #[cfg(unix)]
    #[test]
    fn custody_rejects_replaceable_ancestor_and_parent_replacement() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempdir().expect("temporary directory");
        let replaceable = directory.path().join("replaceable");
        fs::create_dir(&replaceable).expect("create replaceable ancestor");
        fs::set_permissions(&replaceable, fs::Permissions::from_mode(0o777))
            .expect("make ancestor replaceable");
        let error = CustodiedSnapshotPath::prepare(&replaceable.join("ledger.snapshot"))
            .expect_err("replaceable ancestor must fail");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        fs::set_permissions(&replaceable, fs::Permissions::from_mode(0o700))
            .expect("restore private ancestor");

        let custody = CustodiedSnapshotPath::prepare(&replaceable.join("ledger.snapshot"))
            .expect("prepare stable parent");
        let moved = directory.path().join("moved");
        fs::rename(&replaceable, &moved).expect("move pinned parent");
        fs::create_dir(&replaceable).expect("replace pinned parent");
        let error = custody
            .verify_parent()
            .expect_err("parent identity replacement must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("changed identity"));
    }
}
