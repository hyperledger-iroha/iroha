//! Native, fail-closed private-file storage for Electron main-process state.
//!
//! The JavaScript boundary deliberately accepts one already-existing parent, one private
//! directory, and one leaf filename. Keeping the namespace this small lets the host validate
//! every object identity and reject indirection before publishing security-sensitive state.
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use rand_core_06::{OsRng, RngCore as _};
use std::{
    fmt, fs, io,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
const MAXIMUM_BYTES_HARD_LIMIT: u32 = 64 * 1024 * 1024;
const TEMP_NAME_ATTEMPTS: usize = 32;
static STORAGE_LOCK: Mutex<()> = Mutex::new(());
#[derive(Debug)]
enum SecureFsError {
    InvalidInput(String),
    UnsafeStorage(String),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}
impl SecureFsError {
    fn io(action: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
    fn unsafe_storage(message: impl Into<String>) -> Self {
        Self::UnsafeStorage(message.into())
    }
}
impl fmt::Display for SecureFsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::UnsafeStorage(message) => {
                formatter.write_str(message)
            }
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "{action} {}: {source}", path.display()),
        }
    }
}
impl From<SecureFsError> for napi::Error {
    fn from(error: SecureFsError) -> Self {
        let status = match error {
            SecureFsError::InvalidInput(_) => napi::Status::InvalidArg,
            SecureFsError::UnsafeStorage(_) | SecureFsError::Io { .. } => {
                napi::Status::GenericFailure
            }
        };
        napi::Error::new(status, error.to_string())
    }
}
#[cfg(unix)]
type FileIdentity = (u64, u64);
#[cfg(windows)]
type FileIdentity = (u64, [u8; 16]);
#[cfg(not(any(unix, windows)))]
type FileIdentity = ();
#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(not(any(unix, windows)))]
fn file_identity(_metadata: &fs::Metadata) -> FileIdentity {}
#[cfg(unix)]
fn identity_available(_identity: FileIdentity) -> bool {
    true
}
#[cfg(windows)]
fn identity_available(identity: FileIdentity) -> bool {
    identity.1 != [0; 16]
}
#[cfg(not(any(unix, windows)))]
fn identity_available(_identity: FileIdentity) -> bool {
    false
}
#[cfg(unix)]
fn is_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}
#[cfg(not(any(unix, windows)))]
fn is_single_link(_metadata: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    file_identity(left) == file_identity(right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}
#[cfg(not(any(unix, windows)))]
fn metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn publication_preserved_object(staged: &fs::Metadata, published: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    file_identity(staged) == file_identity(published)
        && staged.nlink() == 1
        && published.nlink() == 1
        && staged.len() == published.len()
        && staged.uid() == published.uid()
        && staged.gid() == published.gid()
        && staged.mode() == published.mode()
        && staged.mtime() == published.mtime()
        && staged.mtime_nsec() == published.mtime_nsec()
}
#[cfg(windows)]
fn publication_preserved_object(staged: &fs::Metadata, published: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    staged.file_size() == published.file_size()
        && staged.file_attributes() == published.file_attributes()
        && staged.creation_time() == published.creation_time()
        && staged.last_write_time() == published.last_write_time()
}
#[cfg(not(any(unix, windows)))]
fn publication_preserved_object(_staged: &fs::Metadata, _published: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn path_identity(path: &Path, metadata: &fs::Metadata) -> Result<FileIdentity, SecureFsError> {
    let _ = path;
    Ok(file_identity(metadata))
}
#[cfg(windows)]
fn path_identity(path: &Path, _metadata: &fs::Metadata) -> Result<FileIdentity, SecureFsError> {
    platform::path_identity(path)
        .map_err(|source| SecureFsError::io("query path filesystem identity", path, source))
}
#[cfg(not(any(unix, windows)))]
fn path_identity(_path: &Path, metadata: &fs::Metadata) -> Result<FileIdentity, SecureFsError> {
    Ok(file_identity(metadata))
}
#[cfg(unix)]
fn handle_identity(
    file: &fs::File,
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<FileIdentity, SecureFsError> {
    let _ = (file, path);
    Ok(file_identity(metadata))
}
#[cfg(windows)]
fn handle_identity(
    file: &fs::File,
    path: &Path,
    _metadata: &fs::Metadata,
) -> Result<FileIdentity, SecureFsError> {
    platform::handle_identity(file)
        .map_err(|source| SecureFsError::io("query handle filesystem identity", path, source))
}
#[cfg(not(any(unix, windows)))]
fn handle_identity(
    _file: &fs::File,
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<FileIdentity, SecureFsError> {
    Ok(file_identity(metadata))
}
#[cfg(unix)]
fn path_is_single_link(_path: &Path, metadata: &fs::Metadata) -> Result<bool, SecureFsError> {
    Ok(is_single_link(metadata))
}
#[cfg(windows)]
fn path_is_single_link(path: &Path, _metadata: &fs::Metadata) -> Result<bool, SecureFsError> {
    platform::path_link_count(path)
        .map(|links| links == 1)
        .map_err(|source| SecureFsError::io("query path link count", path, source))
}
#[cfg(not(any(unix, windows)))]
fn path_is_single_link(_path: &Path, metadata: &fs::Metadata) -> Result<bool, SecureFsError> {
    Ok(is_single_link(metadata))
}
#[cfg(unix)]
fn handle_is_single_link(
    _file: &fs::File,
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<bool, SecureFsError> {
    Ok(is_single_link(metadata))
}
#[cfg(windows)]
fn handle_is_single_link(
    file: &fs::File,
    path: &Path,
    _metadata: &fs::Metadata,
) -> Result<bool, SecureFsError> {
    platform::handle_link_count(file)
        .map(|links| links == 1)
        .map_err(|source| SecureFsError::io("query handle link count", path, source))
}
#[cfg(not(any(unix, windows)))]
fn handle_is_single_link(
    _file: &fs::File,
    _path: &Path,
    metadata: &fs::Metadata,
) -> Result<bool, SecureFsError> {
    Ok(is_single_link(metadata))
}
fn validate_maximum_bytes(maximum_bytes: u32) -> Result<u64, SecureFsError> {
    if maximum_bytes == 0 || maximum_bytes > MAXIMUM_BYTES_HARD_LIMIT {
        return Err(SecureFsError::InvalidInput(format!(
            "maximumBytes must be between 1 and {MAXIMUM_BYTES_HARD_LIMIT}"
        )));
    }
    Ok(u64::from(maximum_bytes))
}
fn validate_root_path(root: &Path) -> Result<(), SecureFsError> {
    if !root.is_absolute() {
        return Err(SecureFsError::InvalidInput(
            "secure private root must be an absolute path".to_owned(),
        ));
    }
    if root.parent().is_none() || root.file_name().is_none() {
        return Err(SecureFsError::InvalidInput(
            "secure private root must name a directory below an existing parent".to_owned(),
        ));
    }
    let mut saw_normal = false;
    for component in root.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {}
            Component::Normal(_) => saw_normal = true,
            Component::CurDir | Component::ParentDir => {
                return Err(SecureFsError::InvalidInput(
                    "secure private root must be lexically normalized".to_owned(),
                ));
            }
        }
    }
    if !saw_normal {
        return Err(SecureFsError::InvalidInput(
            "secure private root must not be a filesystem root".to_owned(),
        ));
    }
    platform::validate_root_syntax(root)
}
fn validate_filename(filename: &str) -> Result<(), SecureFsError> {
    if filename.is_empty() || filename.as_bytes().contains(&0) {
        return Err(SecureFsError::InvalidInput(
            "secure private filename must be non-empty and contain no NUL".to_owned(),
        ));
    }
    let path = Path::new(filename);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(SecureFsError::InvalidInput(
            "secure private filename must be one path component".to_owned(),
        ));
    }
    platform::validate_filename_syntax(filename)
}
fn path_has_indirection(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || platform::metadata_is_reparse_point(metadata)
}
struct DirectoryPin {
    path: PathBuf,
    identity: FileIdentity,
    handle: fs::File,
}
struct DirectoryChain(Vec<DirectoryPin>);
impl DirectoryChain {
    fn verify(&self) -> Result<(), SecureFsError> {
        for pinned in &self.0 {
            let handle_metadata = pinned.handle.metadata().map_err(|source| {
                SecureFsError::io("reinspect pinned ancestor", &pinned.path, source)
            })?;
            if !handle_metadata.is_dir()
                || path_has_indirection(&handle_metadata)
                || handle_identity(&pinned.handle, &pinned.path, &handle_metadata)?
                    != pinned.identity
            {
                return Err(SecureFsError::unsafe_storage(format!(
                    "pinned secure private ancestor changed: {}",
                    pinned.path.display()
                )));
            }
            verify_directory_identity(&pinned.path, pinned.identity)?;
            let path_metadata = fs::symlink_metadata(&pinned.path)
                .map_err(|source| SecureFsError::io("reinspect ancestor", &pinned.path, source))?;
            platform::validate_ancestor_directory(&pinned.path, &path_metadata)?;
        }
        Ok(())
    }
}
fn inspect_directory_chain(path: &Path) -> Result<DirectoryChain, SecureFsError> {
    let mut cursor = PathBuf::new();
    let mut directories = Vec::new();
    for component in path.components() {
        cursor.push(component.as_os_str());
        if !cursor.has_root() {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor)
            .map_err(|source| SecureFsError::io("inspect ancestor", &cursor, source))?;
        if path_has_indirection(&metadata) || !metadata.is_dir() {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private ancestor {} must be a direct directory",
                cursor.display()
            )));
        }
        let identity = path_identity(&cursor, &metadata)?;
        if !identity_available(identity) || !platform::directory_metadata_is_supported(&metadata) {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private ancestor {} must have a stable supported identity",
                cursor.display()
            )));
        }
        platform::validate_ancestor_directory(&cursor, &metadata)?;
        let handle = platform::pin_ancestor_directory(&cursor)
            .map_err(|source| SecureFsError::io("pin secure private ancestor", &cursor, source))?;
        let opened_metadata = handle.metadata().map_err(|source| {
            SecureFsError::io("inspect pinned secure private ancestor", &cursor, source)
        })?;
        if !opened_metadata.is_dir()
            || path_has_indirection(&opened_metadata)
            || handle_identity(&handle, &cursor, &opened_metadata)? != identity
        {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private ancestor identity changed while pinning: {}",
                cursor.display()
            )));
        }
        directories.push(DirectoryPin {
            path: cursor.clone(),
            identity,
            handle,
        });
    }
    Ok(DirectoryChain(directories))
}
fn direct_directory_identity(path: &Path) -> Result<FileIdentity, SecureFsError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| SecureFsError::io("inspect directory", path, source))?;
    let identity = path_identity(path, &metadata)?;
    if path_has_indirection(&metadata)
        || !metadata.is_dir()
        || !identity_available(identity)
        || !platform::directory_metadata_is_supported(&metadata)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private directory {} must be direct with a stable supported identity",
            path.display()
        )));
    }
    Ok(identity)
}
fn verify_directory_identity(path: &Path, expected: FileIdentity) -> Result<(), SecureFsError> {
    if direct_directory_identity(path)? != expected {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private directory identity changed: {}",
            path.display()
        )));
    }
    Ok(())
}
struct ValidatedRoot {
    identity: FileIdentity,
    chain: DirectoryChain,
}
impl ValidatedRoot {
    fn verify(&self, root: &Path) -> Result<(), SecureFsError> {
        self.chain.verify()?;
        verify_directory_identity(root, self.identity)?;
        platform::validate_private_directory(root)
    }
}
fn validate_private_root(root: &Path) -> Result<ValidatedRoot, SecureFsError> {
    validate_root_path(root)?;
    let chain = inspect_directory_chain(root)?;
    platform::validate_supported_filesystem(root)?;
    let identity = direct_directory_identity(root)?;
    platform::validate_private_directory(root)?;
    Ok(ValidatedRoot { identity, chain })
}
fn ensure_private_root(root: &Path) -> Result<ValidatedRoot, SecureFsError> {
    validate_root_path(root)?;
    let parent = root.parent().ok_or_else(|| {
        SecureFsError::InvalidInput("secure private root must have a parent".to_owned())
    })?;
    let parent_chain = inspect_directory_chain(parent)?;
    platform::validate_supported_filesystem(parent)?;
    platform::validate_parent_directory(parent)?;
    let parent_identity = direct_directory_identity(parent)?;
    match fs::symlink_metadata(root) {
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            match platform::create_private_directory(root) {
                Ok(()) => {}
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    return Err(SecureFsError::io(
                        "create secure private directory",
                        root,
                        source,
                    ));
                }
            }
        }
        Err(source) => {
            return Err(SecureFsError::io(
                "inspect secure private directory",
                root,
                source,
            ));
        }
    }
    parent_chain.verify()?;
    verify_directory_identity(parent, parent_identity)?;
    let validated = validate_private_root(root)?;
    platform::sync_parent_after_create(parent)?;
    parent_chain.verify()?;
    verify_directory_identity(parent, parent_identity)?;
    validated.verify(root)?;
    Ok(validated)
}
fn open_private_file(path: &Path) -> Result<fs::File, SecureFsError> {
    platform::open_private_file(path)
        .map_err(|source| SecureFsError::io("open secure private file", path, source))
}
fn validate_private_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    maximum_bytes: u64,
) -> Result<(), SecureFsError> {
    if path_has_indirection(metadata)
        || !metadata.is_file()
        || !identity_available(path_identity(path, metadata)?)
        || !path_is_single_link(path, metadata)?
        || metadata.len() == 0
        || metadata.len() > maximum_bytes
        || !platform::file_metadata_is_supported(metadata)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private file {} must be a non-empty bounded direct single-link regular file",
            path.display()
        )));
    }
    platform::validate_private_file_path(path)
}
fn read_private_file(
    root: &Path,
    filename: &str,
    maximum_bytes: u64,
) -> Result<Option<Vec<u8>>, SecureFsError> {
    let validated_root = validate_private_root(root)?;
    let path = root.join(filename);
    let path_before = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            validated_root.verify(root)?;
            return Ok(None);
        }
        Err(source) => {
            return Err(SecureFsError::io(
                "inspect secure private file",
                &path,
                source,
            ));
        }
    };
    validate_private_file_metadata(&path, &path_before, maximum_bytes)?;
    let path_before_identity = path_identity(&path, &path_before)?;
    let mut file = open_private_file(&path)?;
    let opened_before = file
        .metadata()
        .map_err(|source| SecureFsError::io("inspect opened private file", &path, source))?;
    let opened_before_identity = handle_identity(&file, &path, &opened_before)?;
    if path_before_identity != opened_before_identity
        || !handle_is_single_link(&file, &path, &opened_before)?
        || !metadata_unchanged(&path_before, &opened_before)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private file identity changed while opening: {}",
            path.display()
        )));
    }
    platform::validate_private_file_handle(&file, &path)?;
    let capacity = usize::try_from(opened_before.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| SecureFsError::io("read secure private file", &path, source))?;
    let opened_after = file
        .metadata()
        .map_err(|source| SecureFsError::io("reinspect opened private file", &path, source))?;
    let path_after = fs::symlink_metadata(&path)
        .map_err(|source| SecureFsError::io("reinspect secure private file", &path, source))?;
    let opened_after_identity = handle_identity(&file, &path, &opened_after)?;
    let path_after_identity = path_identity(&path, &path_after)?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes
        || opened_after_identity != opened_before_identity
        || path_after_identity != opened_before_identity
        || !handle_is_single_link(&file, &path, &opened_after)?
        || !path_is_single_link(&path, &path_after)?
        || !metadata_unchanged(&opened_before, &opened_after)
        || !metadata_unchanged(&opened_before, &path_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private file changed while reading: {}",
            path.display()
        )));
    }
    platform::validate_private_file_handle(&file, &path)?;
    platform::validate_private_file_path(&path)?;
    validated_root.verify(root)?;
    Ok(Some(bytes))
}
struct TemporaryPath {
    path: PathBuf,
    published: bool,
}
impl TemporaryPath {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: false,
        }
    }
    fn mark_published(&mut self) {
        self.published = true;
    }
}
impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn create_private_temporary_file(
    root: &Path,
    maximum_bytes: u64,
) -> Result<(TemporaryPath, fs::File), SecureFsError> {
    for _ in 0..TEMP_NAME_ATTEMPTS {
        let mut random = [0_u8; 24];
        OsRng.fill_bytes(&mut random);
        let path = root.join(format!(".secure-private-{}.tmp", hex::encode(random)));
        match platform::create_private_file(&path) {
            Ok(file) => {
                let metadata = file.metadata().map_err(|source| {
                    SecureFsError::io("inspect new secure private temporary file", &path, source)
                })?;
                // The temporary is empty until the caller writes it, so validate every invariant
                // except the final non-empty length.
                if path_has_indirection(&metadata)
                    || !metadata.is_file()
                    || !identity_available(handle_identity(&file, &path, &metadata)?)
                    || !handle_is_single_link(&file, &path, &metadata)?
                    || metadata.len() != 0
                    || !platform::file_metadata_is_supported(&metadata)
                {
                    return Err(SecureFsError::unsafe_storage(format!(
                        "new secure private temporary file has unsafe metadata: {}",
                        path.display()
                    )));
                }
                platform::validate_private_file_handle(&file, &path)?;
                if maximum_bytes == 0 {
                    return Err(SecureFsError::InvalidInput(
                        "maximumBytes must be positive".to_owned(),
                    ));
                }
                return Ok((TemporaryPath::new(path), file));
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(SecureFsError::io(
                    "create secure private temporary file",
                    &path,
                    source,
                ));
            }
        }
    }
    Err(SecureFsError::unsafe_storage(
        "could not allocate a collision-free secure private temporary filename",
    ))
}
fn write_private_file(
    root: &Path,
    filename: &str,
    bytes: &[u8],
    maximum_bytes: u64,
) -> Result<Vec<u8>, SecureFsError> {
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(SecureFsError::InvalidInput(
            "secure private file content must be non-empty and within maximumBytes".to_owned(),
        ));
    }
    let validated_root = validate_private_root(root)?;
    let destination = root.join(filename);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) => validate_private_file_metadata(&destination, &metadata, maximum_bytes)?,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(SecureFsError::io(
                "inspect secure private destination",
                &destination,
                source,
            ));
        }
    }
    let (mut temporary, mut file) = create_private_temporary_file(root, maximum_bytes)?;
    let temporary_path = temporary.path.clone();
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| {
            SecureFsError::io(
                "write and synchronize secure private temporary file",
                &temporary_path,
                source,
            )
        })?;
    file.seek(SeekFrom::Start(0)).map_err(|source| {
        SecureFsError::io(
            "rewind secure private temporary file",
            &temporary_path,
            source,
        )
    })?;
    let mut stage_readback = Vec::with_capacity(bytes.len());
    Read::by_ref(&mut file)
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut stage_readback)
        .map_err(|source| {
            SecureFsError::io(
                "read back secure private temporary file",
                &temporary_path,
                source,
            )
        })?;
    if stage_readback != bytes {
        return Err(SecureFsError::unsafe_storage(format!(
            "synchronized secure private temporary file differs from input: {}",
            temporary_path.display()
        )));
    }
    let staged_handle_metadata = file.metadata().map_err(|source| {
        SecureFsError::io(
            "inspect synchronized secure private temporary file handle",
            &temporary_path,
            source,
        )
    })?;
    let staged_path_metadata = fs::symlink_metadata(&temporary_path).map_err(|source| {
        SecureFsError::io(
            "inspect synchronized secure private temporary file path",
            &temporary_path,
            source,
        )
    })?;
    validate_private_file_metadata(&temporary_path, &staged_path_metadata, maximum_bytes)?;
    platform::validate_private_file_handle(&file, &temporary_path)?;
    let staged_path_identity = path_identity(&temporary_path, &staged_path_metadata)?;
    let staged_identity = handle_identity(&file, &temporary_path, &staged_handle_metadata)?;
    if staged_path_identity != staged_identity
        || !path_is_single_link(&temporary_path, &staged_path_metadata)?
        || !handle_is_single_link(&file, &temporary_path, &staged_handle_metadata)?
        || !metadata_unchanged(&staged_path_metadata, &staged_handle_metadata)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private temporary file identity changed or acquired another link: {}",
            temporary_path.display()
        )));
    }
    validated_root.verify(root)?;
    platform::replace_file(&temporary_path, &destination).map_err(|source| {
        SecureFsError::io(
            "atomically replace secure private file",
            &destination,
            source,
        )
    })?;
    temporary.mark_published();
    let published_handle_metadata = file.metadata().map_err(|source| {
        SecureFsError::io(
            "inspect published secure private file handle",
            &destination,
            source,
        )
    })?;
    let published_path_metadata = fs::symlink_metadata(&destination).map_err(|source| {
        SecureFsError::io(
            "inspect published secure private file",
            &destination,
            source,
        )
    })?;
    let published_handle_identity =
        handle_identity(&file, &destination, &published_handle_metadata)?;
    let published_path_identity = path_identity(&destination, &published_path_metadata)?;
    if published_handle_identity != staged_identity
        || published_path_identity != staged_identity
        || !handle_is_single_link(&file, &destination, &published_handle_metadata)?
        || !path_is_single_link(&destination, &published_path_metadata)?
        || !publication_preserved_object(&staged_handle_metadata, &published_handle_metadata)
        || !metadata_unchanged(&published_handle_metadata, &published_path_metadata)
    {
        return Err(SecureFsError::unsafe_storage(format!(
            "secure private file identity changed during atomic replacement: {}",
            destination.display()
        )));
    }
    platform::validate_private_file_handle(&file, &destination)?;
    platform::sync_private_directory(root)?;
    validated_root.verify(root)?;
    let published = read_private_file(root, filename, maximum_bytes)?.ok_or_else(|| {
        SecureFsError::unsafe_storage(format!(
            "published secure private file disappeared: {}",
            destination.display()
        ))
    })?;
    if published != bytes {
        return Err(SecureFsError::unsafe_storage(format!(
            "published secure private file differs from input: {}",
            destination.display()
        )));
    }
    Ok(published)
}
fn lock_storage() -> Result<std::sync::MutexGuard<'static, ()>, SecureFsError> {
    STORAGE_LOCK
        .lock()
        .map_err(|_| SecureFsError::unsafe_storage("secure private storage lock was poisoned"))
}
/// Report the exact secure private-file contract implemented by this addon.
///
/// N-API callbacks do not expose their Rust argument count through JavaScript's
/// `Function.length`, so consumers bind the reviewed signatures to this
/// explicit version instead of inferring an ABI from wrapper metadata.
#[napi(js_name = "securePrivateFileAbiVersion")]
pub fn secure_private_file_abi_version() -> u32 {
    1
}
/// Ensure that `root_path` is a direct current-user-private directory.
#[napi(js_name = "securePrivateDirectoryEnsure")]
pub fn secure_private_directory_ensure(root_path: String) -> napi::Result<()> {
    let _guard = lock_storage()?;
    ensure_private_root(Path::new(&root_path))?;
    Ok(())
}
/// Read one bounded private file, returning `null` when the leaf does not exist.
#[napi(js_name = "securePrivateFileRead")]
pub fn secure_private_file_read(
    root_path: String,
    filename: String,
    maximum_bytes: u32,
) -> napi::Result<Option<Buffer>> {
    let maximum_bytes = validate_maximum_bytes(maximum_bytes)?;
    validate_root_path(Path::new(&root_path))?;
    validate_filename(&filename)?;
    let _guard = lock_storage()?;
    read_private_file(Path::new(&root_path), &filename, maximum_bytes)
        .map(|bytes| bytes.map(Buffer::from))
        .map_err(Into::into)
}
/// Atomically replace one bounded private file and return its exact durable readback.
#[napi(js_name = "securePrivateFileWriteAtomic")]
pub fn secure_private_file_write_atomic(
    root_path: String,
    filename: String,
    bytes: Buffer,
    maximum_bytes: u32,
) -> napi::Result<Buffer> {
    let maximum_bytes = validate_maximum_bytes(maximum_bytes)?;
    validate_root_path(Path::new(&root_path))?;
    validate_filename(&filename)?;
    let _guard = lock_storage()?;
    write_private_file(
        Path::new(&root_path),
        &filename,
        bytes.as_ref(),
        maximum_bytes,
    )
    .map(Buffer::from)
    .map_err(Into::into)
}
#[cfg(unix)]
mod platform {
    use super::SecureFsError;
    use std::{
        fs, io,
        os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _},
        path::Path,
    };
    const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
    const PRIVATE_FILE_MODE: u32 = 0o600;
    pub(super) fn validate_root_syntax(_root: &Path) -> Result<(), SecureFsError> {
        Ok(())
    }
    pub(super) fn validate_filename_syntax(_filename: &str) -> Result<(), SecureFsError> {
        Ok(())
    }
    pub(super) fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
        false
    }
    pub(super) fn directory_metadata_is_supported(metadata: &fs::Metadata) -> bool {
        metadata.ino() != 0
    }
    pub(super) fn file_metadata_is_supported(metadata: &fs::Metadata) -> bool {
        metadata.ino() != 0
    }
    pub(super) fn validate_ancestor_directory(
        path: &Path,
        metadata: &fs::Metadata,
    ) -> Result<(), SecureFsError> {
        let owner = metadata.uid();
        let mode = metadata.mode();
        let trusted_owner = owner == effective_uid() || owner == 0;
        let safely_sticky = mode & 0o1000 != 0 && trusted_owner;
        if !trusted_owner || (mode & 0o022 != 0 && !safely_sticky) {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private ancestor {} must have a trusted owner and deny unsafe namespace replacement",
                path.display()
            )));
        }
        extended_acl::validate_ancestor_path(path)?;
        Ok(())
    }
    fn effective_uid() -> u32 {
        rustix::process::geteuid().as_raw()
    }
    pub(super) fn validate_supported_filesystem(_path: &Path) -> Result<(), SecureFsError> {
        Ok(())
    }
    pub(super) fn validate_parent_directory(path: &Path) -> Result<(), SecureFsError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|source| SecureFsError::io("inspect secure private parent", path, source))?;
        if metadata.uid() != effective_uid() || metadata.mode() & 0o022 != 0 {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private parent {} must be owned by the current user and not writable by group or other",
                path.display()
            )));
        }
        extended_acl::validate_ancestor_path(path)?;
        Ok(())
    }
    pub(super) fn validate_private_directory(path: &Path) -> Result<(), SecureFsError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|source| SecureFsError::io("inspect secure private root", path, source))?;
        if metadata.uid() != effective_uid() || metadata.mode() & 0o777 != PRIVATE_DIRECTORY_MODE {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private root {} must be owned by the current user with mode 0700",
                path.display()
            )));
        }
        extended_acl::validate_private_path(path)?;
        Ok(())
    }
    pub(super) fn validate_private_file_path(path: &Path) -> Result<(), SecureFsError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|source| SecureFsError::io("inspect secure private file", path, source))?;
        if metadata.uid() != effective_uid() || metadata.mode() & 0o777 != PRIVATE_FILE_MODE {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private file {} must be owned by the current user with mode 0600",
                path.display()
            )));
        }
        extended_acl::validate_private_path(path)?;
        Ok(())
    }
    pub(super) fn validate_private_file_handle(
        file: &fs::File,
        path: &Path,
    ) -> Result<(), SecureFsError> {
        let metadata = file.metadata().map_err(|source| {
            SecureFsError::io("inspect secure private file handle", path, source)
        })?;
        if metadata.uid() != effective_uid() || metadata.mode() & 0o777 != PRIVATE_FILE_MODE {
            return Err(SecureFsError::unsafe_storage(format!(
                "opened secure private file {} is not current-user mode 0600",
                path.display()
            )));
        }
        extended_acl::validate_file(file, path)?;
        Ok(())
    }
    pub(super) fn create_private_directory(path: &Path) -> io::Result<()> {
        let mut builder = fs::DirBuilder::new();
        builder.mode(PRIVATE_DIRECTORY_MODE);
        builder.create(path)?;
        if let Err(source) = extended_acl::clear_path(path) {
            let _ = fs::remove_dir(path);
            return Err(source);
        }
        Ok(())
    }
    pub(super) fn pin_ancestor_directory(path: &Path) -> io::Result<fs::File> {
        let mut options = fs::OpenOptions::new();
        options.read(true).custom_flags(
            (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
        options.open(path)
    }
    pub(super) fn open_private_file(path: &Path) -> io::Result<fs::File> {
        let mut options = fs::OpenOptions::new();
        options
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        options.open(path)
    }
    pub(super) fn create_private_file(path: &Path) -> io::Result<fs::File> {
        let mut options = fs::OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .mode(PRIVATE_FILE_MODE)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        let file = options.open(path)?;
        if let Err(source) = extended_acl::clear_file(&file) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(source);
        }
        Ok(file)
    }
    pub(super) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }
    fn sync_directory(path: &Path) -> io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.read(true).custom_flags(
            (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
        options.open(path)?.sync_all()
    }
    pub(super) fn sync_parent_after_create(path: &Path) -> Result<(), SecureFsError> {
        sync_directory(path)
            .map_err(|source| SecureFsError::io("synchronize secure private parent", path, source))
    }
    pub(super) fn sync_private_directory(path: &Path) -> Result<(), SecureFsError> {
        sync_directory(path).map_err(|source| {
            SecureFsError::io("synchronize secure private directory", path, source)
        })
    }
    #[cfg(target_os = "macos")]
    #[allow(unsafe_code)]
    mod extended_acl {
        use super::SecureFsError;
        use std::{
            ffi::{CString, c_char, c_int, c_void},
            fs, io,
            os::{fd::AsRawFd as _, unix::ffi::OsStrExt as _},
            path::Path,
            ptr,
        };
        const ACL_TYPE_EXTENDED: c_int = 0x0000_0100;
        const ACL_FIRST_ENTRY: c_int = 0;
        const ACL_NEXT_ENTRY: c_int = -1;
        const ACL_EXTENDED_DENY: c_int = 2;
        type Acl = *mut c_void;
        type AclEntry = *mut c_void;
        unsafe extern "C" {
            fn acl_free(object: *mut c_void) -> c_int;
            fn acl_get_entry(acl: Acl, entry_id: c_int, entry: *mut AclEntry) -> c_int;
            fn acl_get_tag_type(entry: AclEntry, tag_type: *mut c_int) -> c_int;
            fn acl_get_fd_np(fd: c_int, acl_type: c_int) -> Acl;
            fn acl_get_link_np(path: *const c_char, acl_type: c_int) -> Acl;
            fn acl_init(count: c_int) -> Acl;
            fn acl_set_fd_np(fd: c_int, acl: Acl, acl_type: c_int) -> c_int;
            fn acl_set_link_np(path: *const c_char, acl_type: c_int, acl: Acl) -> c_int;
            fn acl_valid(acl: Acl) -> c_int;
        }
        struct AclGuard(Acl);
        impl Drop for AclGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    // SAFETY: Every ACL wrapped by this guard is owned by the POSIX ACL API.
                    unsafe {
                        acl_free(self.0);
                    }
                }
            }
        }
        fn c_path(path: &Path) -> io::Result<CString> {
            CString::new(path.as_os_str().as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "path contains an embedded NUL")
            })
        }
        fn acl_or_absent(acl: Acl, path: &Path) -> Result<Option<AclGuard>, SecureFsError> {
            if acl.is_null() {
                let source = io::Error::last_os_error();
                if source.kind() == io::ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(SecureFsError::io("read macOS extended ACL", path, source));
            }
            Ok(Some(AclGuard(acl)))
        }
        fn require_valid(acl: &AclGuard, path: &Path) -> Result<(), SecureFsError> {
            // SAFETY: The guard owns a live ACL returned by the macOS ACL API.
            if unsafe { acl_valid(acl.0) } == 0 {
                return Ok(());
            }
            Err(SecureFsError::io(
                "validate macOS extended ACL",
                path,
                io::Error::last_os_error(),
            ))
        }
        fn is_entry_exhaustion(source: &io::Error) -> bool {
            // macOS returns EINVAL after the final ACL entry (including an empty ACL).
            source.kind() == io::ErrorKind::InvalidInput
        }
        fn require_empty(acl: Acl, path: &Path) -> Result<(), SecureFsError> {
            let Some(acl) = acl_or_absent(acl, path)? else {
                return Ok(());
            };
            require_valid(&acl, path)?;
            let mut entry = ptr::null_mut();
            // SAFETY: `acl` is live and `entry` is a valid out pointer.
            match unsafe { acl_get_entry(acl.0, ACL_FIRST_ENTRY, &mut entry) } {
                0 => Err(SecureFsError::unsafe_storage(format!(
                    "secure private object {} must not have an extended ACL",
                    path.display()
                ))),
                _ => {
                    let source = io::Error::last_os_error();
                    if is_entry_exhaustion(&source) {
                        Ok(())
                    } else {
                        Err(SecureFsError::io(
                            "inspect macOS extended ACL",
                            path,
                            source,
                        ))
                    }
                }
            }
        }
        fn require_deny_only(acl: Acl, path: &Path) -> Result<(), SecureFsError> {
            let Some(acl) = acl_or_absent(acl, path)? else {
                return Ok(());
            };
            require_valid(&acl, path)?;
            let mut entry_id = ACL_FIRST_ENTRY;
            loop {
                let mut entry = ptr::null_mut();
                // SAFETY: `acl` is live and `entry` is a valid out pointer.
                match unsafe { acl_get_entry(acl.0, entry_id, &mut entry) } {
                    0 => {
                        let mut tag_type = 0;
                        // SAFETY: A successful acl_get_entry returned a live ACL entry.
                        if unsafe { acl_get_tag_type(entry, &mut tag_type) } != 0 {
                            return Err(SecureFsError::io(
                                "inspect macOS ancestor ACL tag",
                                path,
                                io::Error::last_os_error(),
                            ));
                        }
                        if tag_type != ACL_EXTENDED_DENY {
                            return Err(SecureFsError::unsafe_storage(format!(
                                "secure private ancestor {} must not have an extended allow ACL",
                                path.display()
                            )));
                        }
                        entry_id = ACL_NEXT_ENTRY;
                    }
                    _ => {
                        let source = io::Error::last_os_error();
                        if is_entry_exhaustion(&source) {
                            return Ok(());
                        }
                        return Err(SecureFsError::io(
                            "inspect macOS ancestor ACL",
                            path,
                            source,
                        ));
                    }
                }
            }
        }
        pub(super) fn validate_ancestor_path(path: &Path) -> Result<(), SecureFsError> {
            let path_c = c_path(path)
                .map_err(|source| SecureFsError::io("encode macOS ACL path", path, source))?;
            // SAFETY: `path_c` is a live NUL-terminated path.
            require_deny_only(
                unsafe { acl_get_link_np(path_c.as_ptr(), ACL_TYPE_EXTENDED) },
                path,
            )
        }
        pub(super) fn validate_private_path(path: &Path) -> Result<(), SecureFsError> {
            let path_c = c_path(path)
                .map_err(|source| SecureFsError::io("encode macOS ACL path", path, source))?;
            // SAFETY: `path_c` is a live NUL-terminated path.
            require_empty(
                unsafe { acl_get_link_np(path_c.as_ptr(), ACL_TYPE_EXTENDED) },
                path,
            )
        }
        pub(super) fn validate_file(file: &fs::File, path: &Path) -> Result<(), SecureFsError> {
            // SAFETY: The descriptor remains live for the duration of this ACL query.
            require_empty(
                unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) },
                path,
            )
        }
        fn empty_acl() -> io::Result<AclGuard> {
            // SAFETY: Zero requests an initialized ACL containing no entries.
            let acl = unsafe { acl_init(0) };
            if acl.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(AclGuard(acl))
        }
        pub(super) fn clear_path(path: &Path) -> io::Result<()> {
            let path_c = c_path(path)?;
            let acl = empty_acl()?;
            // SAFETY: Both path and ACL are live for the duration of the call.
            if unsafe { acl_set_link_np(path_c.as_ptr(), ACL_TYPE_EXTENDED, acl.0) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
        pub(super) fn clear_file(file: &fs::File) -> io::Result<()> {
            let acl = empty_acl()?;
            // SAFETY: The descriptor and ACL remain live for the duration of the call.
            if unsafe { acl_set_fd_np(file.as_raw_fd(), acl.0, ACL_TYPE_EXTENDED) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }
    #[cfg(not(target_os = "macos"))]
    mod extended_acl {
        use super::SecureFsError;
        use std::{fs, io, path::Path};
        pub(super) fn validate_ancestor_path(_path: &Path) -> Result<(), SecureFsError> {
            Ok(())
        }
        pub(super) fn validate_private_path(_path: &Path) -> Result<(), SecureFsError> {
            Ok(())
        }
        pub(super) fn validate_file(_file: &fs::File, _path: &Path) -> Result<(), SecureFsError> {
            Ok(())
        }
        pub(super) fn clear_path(_path: &Path) -> io::Result<()> {
            Ok(())
        }
        pub(super) fn clear_file(_file: &fs::File) -> io::Result<()> {
            Ok(())
        }
    }
}
#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    use super::SecureFsError;
    use std::{
        ffi::{OsStr, c_void},
        fs, io, mem,
        os::windows::{
            ffi::OsStrExt as _,
            fs::{MetadataExt as _, OpenOptionsExt as _},
            io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
        },
        path::{Component, Path, Prefix},
        ptr,
    };
    #[cfg(test)]
    use windows_sys::Win32::Security::{Authorization::SetSecurityInfo, WinWorldSid};
    use windows_sys::Win32::{
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GENERIC_ALL, GENERIC_READ, GENERIC_WRITE,
            HANDLE, INVALID_HANDLE_VALUE, LocalFree,
        },
        Security::{
            ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_REVISION, ACL_SIZE_INFORMATION,
            AclSizeInformation, AddAccessAllowedAceEx,
            Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT},
            CONTAINER_INHERIT_ACE, CopySid, CreateWellKnownSid, DACL_SECURITY_INFORMATION,
            EqualSid, GetAce, GetAclInformation, GetLengthSid, GetSecurityDescriptorControl,
            GetTokenInformation, INHERIT_ONLY_ACE, INHERITED_ACE, InitializeAcl,
            InitializeSecurityDescriptor, OBJECT_INHERIT_ACE, OWNER_SECURITY_INFORMATION, PSID,
            SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SECURITY_MAX_SID_SIZE,
            SetSecurityDescriptorControl, SetSecurityDescriptorDacl, SetSecurityDescriptorOwner,
            TOKEN_QUERY, TOKEN_USER, TokenUser, WinBuiltinAdministratorsSid, WinLocalSystemSid,
        },
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW, DELETE,
            FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_WRITE_THROUGH, FILE_ID_INFO, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FileIdInfo, GetFileInformationByHandle,
            GetFileInformationByHandleEx, GetVolumeInformationW, GetVolumePathNameW,
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, OPEN_EXISTING,
            READ_CONTROL, WRITE_DAC, WRITE_OWNER,
        },
        System::{
            SystemServices::{
                ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE, SECURITY_DESCRIPTOR_REVISION,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };
    const ANCESTOR_REPLACEMENT_OR_CONTROL_RIGHTS: u32 =
        FILE_DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER | GENERIC_WRITE | GENERIC_ALL;
    const PARENT_CREATION_RIGHTS: u32 = FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY;
    fn wide_literal(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
    fn wide_path(path: &Path) -> Vec<u16> {
        let needs_verbatim_prefix = matches!(
            path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
        );
        let mut result = if needs_verbatim_prefix {
            "\\\\?\\".encode_utf16().collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        result.extend(path.as_os_str().encode_wide());
        result.push(0);
        result
    }
    fn last_error() -> io::Error {
        io::Error::last_os_error()
    }
    fn win32_error(code: u32) -> io::Error {
        io::Error::from_raw_os_error(i32::try_from(code).unwrap_or(i32::MAX))
    }
    fn file_information(file: &fs::File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: The file handle is live and `information` is a valid output buffer.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) } == 0
        {
            return Err(last_error());
        }
        Ok(information)
    }
    pub(super) fn handle_identity(file: &fs::File) -> io::Result<super::FileIdentity> {
        let mut information = FILE_ID_INFO::default();
        // SAFETY: The file handle is live and `information` is the exact documented output
        // buffer for `FileIdInfo`.
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle().cast(),
                FileIdInfo,
                (&raw mut information).cast(),
                u32::try_from(mem::size_of::<FILE_ID_INFO>()).expect("FILE_ID_INFO size fits u32"),
            )
        } == 0
        {
            return Err(last_error());
        }
        Ok((
            information.VolumeSerialNumber,
            information.FileId.Identifier,
        ))
    }
    pub(super) fn handle_link_count(file: &fs::File) -> io::Result<u32> {
        Ok(file_information(file)?.nNumberOfLinks)
    }
    pub(super) fn path_identity(path: &Path) -> io::Result<super::FileIdentity> {
        handle_identity(&open_metadata_object(path)?)
    }
    pub(super) fn path_link_count(path: &Path) -> io::Result<u32> {
        handle_link_count(&open_metadata_object(path)?)
    }
    struct SidBuffer {
        words: Vec<usize>,
        length: u32,
    }
    impl SidBuffer {
        fn with_length(length: u32) -> io::Result<Self> {
            let bytes = usize::try_from(length)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "SID is too large"))?;
            let words = bytes.div_ceil(mem::size_of::<usize>()).max(1);
            Ok(Self {
                words: vec![0; words],
                length,
            })
        }
        fn as_sid(&self) -> PSID {
            self.words.as_ptr().cast_mut().cast()
        }
    }
    struct PrivateSids {
        user: SidBuffer,
        system: SidBuffer,
        administrators: SidBuffer,
    }
    impl PrivateSids {
        fn current() -> io::Result<Self> {
            let mut token: HANDLE = ptr::null_mut();
            // SAFETY: `token` is a valid out pointer and the pseudo process handle is valid.
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
                return Err(last_error());
            }
            // SAFETY: OpenProcessToken returned an owned kernel handle.
            let token = unsafe { OwnedHandle::from_raw_handle(token.cast()) };
            let mut required = 0_u32;
            // SAFETY: A null buffer with zero length is the documented size-query form.
            let first = unsafe {
                GetTokenInformation(
                    token.as_raw_handle().cast(),
                    TokenUser,
                    ptr::null_mut(),
                    0,
                    &mut required,
                )
            };
            if first != 0
                || required == 0
                || last_error().raw_os_error()
                    != Some(i32::try_from(ERROR_INSUFFICIENT_BUFFER).unwrap_or(122))
            {
                return Err(last_error());
            }
            let token_words = usize::try_from(required)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "token SID is too large"))?
                .div_ceil(mem::size_of::<usize>())
                .max(1);
            let mut token_information = vec![0_usize; token_words];
            // SAFETY: The aligned allocation has the exact byte capacity requested by Windows.
            if unsafe {
                GetTokenInformation(
                    token.as_raw_handle().cast(),
                    TokenUser,
                    token_information.as_mut_ptr().cast(),
                    required,
                    &mut required,
                )
            } == 0
            {
                return Err(last_error());
            }
            // SAFETY: A successful TokenUser query initializes a TOKEN_USER at the buffer start.
            let token_user = unsafe { &*token_information.as_ptr().cast::<TOKEN_USER>() };
            // SAFETY: Windows returned a valid SID within the still-live token-information buffer.
            let user_length = unsafe { GetLengthSid(token_user.User.Sid) };
            if user_length == 0 {
                return Err(last_error());
            }
            let user = SidBuffer::with_length(user_length)?;
            // SAFETY: Both SID pointers are valid for `user_length` bytes.
            if unsafe { CopySid(user_length, user.as_sid(), token_user.User.Sid) } == 0 {
                return Err(last_error());
            }
            let system = well_known_sid(WinLocalSystemSid)?;
            let administrators = well_known_sid(WinBuiltinAdministratorsSid)?;
            Ok(Self {
                user,
                system,
                administrators,
            })
        }
        fn all(&self) -> [PSID; 3] {
            [
                self.user.as_sid(),
                self.system.as_sid(),
                self.administrators.as_sid(),
            ]
        }
        fn unique(&self) -> Vec<PSID> {
            let mut unique = Vec::new();
            for sid in self.all() {
                if unique.iter().any(|existing| {
                    // SAFETY: Every pointer is a SID owned by this still-live structure.
                    (unsafe { EqualSid(sid, *existing) }) != 0
                }) {
                    continue;
                }
                unique.push(sid);
            }
            unique
        }
    }
    fn well_known_sid(sid_type: i32) -> io::Result<SidBuffer> {
        let mut sid = SidBuffer::with_length(SECURITY_MAX_SID_SIZE)?;
        let mut length = sid.length;
        // SAFETY: `sid` owns a SECURITY_MAX_SID_SIZE-byte aligned output buffer.
        if unsafe { CreateWellKnownSid(sid_type, ptr::null_mut(), sid.as_sid(), &mut length) } == 0
        {
            return Err(last_error());
        }
        sid.length = length;
        Ok(sid)
    }
    struct PrivateSecurityDescriptor {
        _sids: PrivateSids,
        _acl_words: Vec<usize>,
        descriptor: Box<SECURITY_DESCRIPTOR>,
    }
    impl PrivateSecurityDescriptor {
        fn new(directory: bool) -> io::Result<Self> {
            let sids = PrivateSids::current()?;
            let unique_sids = sids.unique();
            let acl_bytes = mem::size_of::<ACL>()
                + unique_sids
                    .iter()
                    .map(|sid| {
                        // SAFETY: The SIDs are valid for this constructor's lifetime.
                        let sid_length = unsafe { GetLengthSid(*sid) } as usize;
                        mem::size_of::<ACCESS_ALLOWED_ACE>()
                            .saturating_sub(mem::size_of::<u32>())
                            .saturating_add(sid_length)
                    })
                    .sum::<usize>();
            let mut acl_words = vec![0_usize; acl_bytes.div_ceil(mem::size_of::<usize>()).max(1)];
            let acl = acl_words.as_mut_ptr().cast::<ACL>();
            // SAFETY: `acl_words` is aligned and at least `acl_bytes` long.
            if unsafe {
                InitializeAcl(
                    acl,
                    u32::try_from(acl_bytes).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "private ACL is too large")
                    })?,
                    ACL_REVISION,
                )
            } == 0
            {
                return Err(last_error());
            }
            let ace_flags = if directory {
                OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
            } else {
                0
            };
            for sid in unique_sids {
                // SAFETY: The ACL has precomputed capacity and `sid` remains alive in `sids`.
                if unsafe {
                    AddAccessAllowedAceEx(acl, ACL_REVISION, ace_flags, FILE_ALL_ACCESS, sid)
                } == 0
                {
                    return Err(last_error());
                }
            }
            let mut descriptor = Box::<SECURITY_DESCRIPTOR>::default();
            let descriptor_pointer = descriptor.as_mut() as *mut SECURITY_DESCRIPTOR;
            // SAFETY: The descriptor, ACL, and SIDs remain owned by the returned structure.
            if unsafe {
                InitializeSecurityDescriptor(
                    descriptor_pointer.cast(),
                    SECURITY_DESCRIPTOR_REVISION,
                )
            } == 0
                || unsafe {
                    SetSecurityDescriptorOwner(descriptor_pointer.cast(), sids.user.as_sid(), 0)
                } == 0
                || unsafe { SetSecurityDescriptorDacl(descriptor_pointer.cast(), 1, acl, 0) } == 0
                || unsafe {
                    SetSecurityDescriptorControl(
                        descriptor_pointer.cast(),
                        SE_DACL_PROTECTED,
                        SE_DACL_PROTECTED,
                    )
                } == 0
            {
                return Err(last_error());
            }
            Ok(Self {
                _sids: sids,
                _acl_words: acl_words,
                descriptor,
            })
        }
        fn security_attributes(&mut self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES {
                nLength: u32::try_from(mem::size_of::<SECURITY_ATTRIBUTES>())
                    .expect("SECURITY_ATTRIBUTES size fits u32"),
                lpSecurityDescriptor: self.descriptor.as_mut() as *mut SECURITY_DESCRIPTOR as _,
                bInheritHandle: 0,
            }
        }
    }
    struct LocalSecurityDescriptor(*mut c_void);
    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: GetSecurityInfo allocates this descriptor with LocalAlloc.
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }
    struct LocalSid(PSID);
    impl LocalSid {
        fn trusted_installer() -> io::Result<Self> {
            // The service SID is stable across supported Windows releases.
            let literal = wide_literal(OsStr::new(
                "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464",
            ));
            let mut sid = ptr::null_mut();
            // SAFETY: `literal` is a live NUL-terminated SID string and `sid` is an out pointer.
            if unsafe { ConvertStringSidToSidW(literal.as_ptr(), &mut sid) } == 0 {
                return Err(last_error());
            }
            Ok(Self(sid))
        }
    }
    impl Drop for LocalSid {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: ConvertStringSidToSidW allocates this SID with LocalAlloc.
                unsafe {
                    LocalFree(self.0.cast());
                }
            }
        }
    }
    fn validate_private_acl(
        file: &fs::File,
        path: &Path,
        directory: bool,
    ) -> Result<(), SecureFsError> {
        let sids = PrivateSids::current().map_err(|source| {
            SecureFsError::io("resolve current Windows identity", path, source)
        })?;
        let mut owner: PSID = ptr::null_mut();
        let mut dacl: *mut ACL = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        // SAFETY: The file handle is live and all output pointers are valid.
        let result = unsafe {
            GetSecurityInfo(
                file.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(SecureFsError::io(
                "read Windows private ACL",
                path,
                win32_error(result),
            ));
        }
        let _descriptor_guard = LocalSecurityDescriptor(descriptor);
        if owner.is_null() || dacl.is_null() {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows private object {} must have an owner and protected DACL",
                path.display()
            )));
        }
        // SAFETY: GetSecurityInfo returned valid owner and current-SID pointers.
        if unsafe { EqualSid(owner, sids.user.as_sid()) } == 0 {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows private object {} is not owned by the current user",
                path.display()
            )));
        }
        let mut control = 0_u16;
        let mut revision = 0_u32;
        // SAFETY: `descriptor` is valid until `_descriptor_guard` is dropped.
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
            return Err(SecureFsError::io(
                "inspect Windows private security descriptor",
                path,
                last_error(),
            ));
        }
        if control & SE_DACL_PROTECTED == 0 {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows private object {} must have inheritance disabled",
                path.display()
            )));
        }
        let mut information = ACL_SIZE_INFORMATION::default();
        // SAFETY: `dacl` is valid and `information` has the documented layout.
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                u32::try_from(mem::size_of::<ACL_SIZE_INFORMATION>())
                    .expect("ACL_SIZE_INFORMATION size fits u32"),
                AclSizeInformation,
            )
        } == 0
        {
            return Err(SecureFsError::io(
                "inspect Windows private ACL entries",
                path,
                last_error(),
            ));
        }
        let expected_sids = sids.unique();
        if information.AceCount
            != u32::try_from(expected_sids.len()).expect("private SID count fits u32")
        {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows private object {} must grant exactly the canonical private principals",
                path.display()
            )));
        }
        let expected_flags = u8::try_from(if directory {
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
        } else {
            0
        })
        .expect("ACE flags fit u8");
        let mut matched = vec![false; expected_sids.len()];
        for index in 0..information.AceCount {
            let mut ace_pointer = ptr::null_mut();
            // SAFETY: The index is bounded by the ACL-provided ACE count.
            if unsafe { GetAce(dacl, index, &mut ace_pointer) } == 0 || ace_pointer.is_null() {
                return Err(SecureFsError::io(
                    "read Windows private ACL entry",
                    path,
                    last_error(),
                ));
            }
            // SAFETY: GetAce returned an ACCESS_ALLOWED_ACE-sized entry after type validation.
            let ace = unsafe { &*ace_pointer.cast::<ACCESS_ALLOWED_ACE>() };
            if u32::from(ace.Header.AceType) != ACCESS_ALLOWED_ACE_TYPE
                || ace.Header.AceFlags & INHERITED_ACE as u8 != 0
                || ace.Header.AceFlags != expected_flags
                || ace.Mask != FILE_ALL_ACCESS
            {
                return Err(SecureFsError::unsafe_storage(format!(
                    "Windows private object {} has a non-canonical access entry",
                    path.display()
                )));
            }
            let ace_sid = (&ace.SidStart as *const u32).cast_mut().cast();
            let Some(position) = expected_sids.iter().position(|expected| {
                // SAFETY: Both values are valid SID pointers owned by live descriptors.
                (unsafe { EqualSid(ace_sid, *expected) }) != 0
            }) else {
                return Err(SecureFsError::unsafe_storage(format!(
                    "Windows private object {} grants an unexpected principal",
                    path.display()
                )));
            };
            if matched[position] {
                return Err(SecureFsError::unsafe_storage(format!(
                    "Windows private object {} duplicates an access principal",
                    path.display()
                )));
            }
            matched[position] = true;
        }
        if !matched.into_iter().all(|value| value) {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows private object {} omits a required access principal",
                path.display()
            )));
        }
        Ok(())
    }
    fn validate_safe_directory_acl(
        file: &fs::File,
        path: &Path,
        require_current_user_owner: bool,
        reject_unexpected_creation_rights: bool,
    ) -> Result<(), SecureFsError> {
        let sids = PrivateSids::current().map_err(|source| {
            SecureFsError::io("resolve current Windows identity", path, source)
        })?;
        let trusted_installer = LocalSid::trusted_installer().map_err(|source| {
            SecureFsError::io("resolve Windows TrustedInstaller identity", path, source)
        })?;
        let mut owner: PSID = ptr::null_mut();
        let mut dacl: *mut ACL = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        // SAFETY: The file handle is live and all output pointers are valid.
        let result = unsafe {
            GetSecurityInfo(
                file.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(SecureFsError::io(
                "read Windows parent ACL",
                path,
                win32_error(result),
            ));
        }
        let _descriptor_guard = LocalSecurityDescriptor(descriptor);
        if owner.is_null() || dacl.is_null() {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows secure private parent {} must have an owner and DACL",
                path.display()
            )));
        }
        let mut trusted_sids = sids.all().to_vec();
        trusted_sids.push(trusted_installer.0);
        // SAFETY: GetSecurityInfo returned a valid owner; all trusted SIDs remain live.
        let trusted_owner = trusted_sids
            .iter()
            .any(|trusted| (unsafe { EqualSid(owner, *trusted) }) != 0);
        let current_owner = (unsafe { EqualSid(owner, sids.user.as_sid()) }) != 0;
        if !trusted_owner || (require_current_user_owner && !current_owner) {
            return Err(SecureFsError::unsafe_storage(format!(
                "Windows secure private directory {} does not have the required trusted owner",
                path.display()
            )));
        }
        let mut information = ACL_SIZE_INFORMATION::default();
        // SAFETY: `dacl` is valid and `information` has the documented layout.
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                u32::try_from(mem::size_of::<ACL_SIZE_INFORMATION>())
                    .expect("ACL_SIZE_INFORMATION size fits u32"),
                AclSizeInformation,
            )
        } == 0
        {
            return Err(SecureFsError::io(
                "inspect Windows parent ACL entries",
                path,
                last_error(),
            ));
        }
        let mut rejected_rights = ANCESTOR_REPLACEMENT_OR_CONTROL_RIGHTS;
        if reject_unexpected_creation_rights {
            rejected_rights |= PARENT_CREATION_RIGHTS;
        }
        for index in 0..information.AceCount {
            let mut ace_pointer = ptr::null_mut();
            // SAFETY: The index is bounded by the ACL-provided ACE count.
            if unsafe { GetAce(dacl, index, &mut ace_pointer) } == 0 || ace_pointer.is_null() {
                return Err(SecureFsError::io(
                    "read Windows parent ACL entry",
                    path,
                    last_error(),
                ));
            }
            // SAFETY: Every ACL entry starts with an ACE_HEADER.
            let header = unsafe { &*ace_pointer.cast::<ACE_HEADER>() };
            if header.AceFlags & u8::try_from(INHERIT_ONLY_ACE).expect("ACE flag fits u8") != 0
                || u32::from(header.AceType) == ACCESS_DENIED_ACE_TYPE
            {
                continue;
            }
            if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
                return Err(SecureFsError::unsafe_storage(format!(
                    "Windows secure private parent {} has an unsupported effective ACL entry",
                    path.display()
                )));
            }
            // SAFETY: Type validation above establishes ACCESS_ALLOWED_ACE layout.
            let ace = unsafe { &*ace_pointer.cast::<ACCESS_ALLOWED_ACE>() };
            let ace_sid = (&ace.SidStart as *const u32).cast_mut().cast();
            let expected_principal = trusted_sids.iter().any(|expected| {
                // SAFETY: Both values are valid SID pointers owned by live descriptors.
                (unsafe { EqualSid(ace_sid, *expected) }) != 0
            });
            if !expected_principal && ace.Mask & rejected_rights != 0 {
                return Err(SecureFsError::unsafe_storage(format!(
                    "Windows secure private parent {} grants namespace mutation or security control to an unexpected principal",
                    path.display()
                )));
            }
        }
        Ok(())
    }
    fn validate_windows_leaf(value: &str) -> Result<(), SecureFsError> {
        if value.ends_with([' ', '.'])
            || value
                .chars()
                .any(|character| character < '\u{20}' || r#"<>:"/\|?*"#.contains(character))
        {
            return Err(SecureFsError::InvalidInput(
                "Windows secure private filename contains a reserved character".to_owned(),
            ));
        }
        let stem = value
            .split('.')
            .next()
            .unwrap_or(value)
            .trim_end_matches([' ', '.'])
            .to_ascii_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        );
        if reserved {
            return Err(SecureFsError::InvalidInput(
                "Windows secure private filename is a reserved device name".to_owned(),
            ));
        }
        Ok(())
    }
    pub(super) fn validate_root_syntax(root: &Path) -> Result<(), SecureFsError> {
        let mut components = root.components();
        let prefix = match components.next() {
            Some(Component::Prefix(prefix)) => prefix.kind(),
            _ => {
                return Err(SecureFsError::InvalidInput(
                    "Windows secure private root must start with a local drive".to_owned(),
                ));
            }
        };
        if !matches!(prefix, Prefix::Disk(_) | Prefix::VerbatimDisk(_))
            || !matches!(components.next(), Some(Component::RootDir))
        {
            return Err(SecureFsError::InvalidInput(
                "Windows secure private root must be an absolute local-drive path".to_owned(),
            ));
        }
        for component in components {
            let Component::Normal(component) = component else {
                return Err(SecureFsError::InvalidInput(
                    "Windows secure private root must be lexically normalized".to_owned(),
                ));
            };
            let component = component.to_str().ok_or_else(|| {
                SecureFsError::InvalidInput(
                    "Windows secure private root must contain valid Unicode".to_owned(),
                )
            })?;
            validate_windows_leaf(component)?;
        }
        Ok(())
    }
    pub(super) fn validate_filename_syntax(filename: &str) -> Result<(), SecureFsError> {
        validate_windows_leaf(filename)
    }
    pub(super) fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    pub(super) fn directory_metadata_is_supported(_metadata: &fs::Metadata) -> bool {
        true
    }
    pub(super) fn file_metadata_is_supported(_metadata: &fs::Metadata) -> bool {
        true
    }
    pub(super) fn validate_ancestor_directory(
        path: &Path,
        _metadata: &fs::Metadata,
    ) -> Result<(), SecureFsError> {
        let file = open_directory(path)
            .map_err(|source| SecureFsError::io("open secure private ancestor", path, source))?;
        validate_safe_directory_acl(&file, path, false, false)
    }
    pub(super) fn validate_supported_filesystem(path: &Path) -> Result<(), SecureFsError> {
        let path_wide = wide_path(path);
        let mut volume_path = vec![0_u16; 32_768];
        // SAFETY: Both UTF-16 buffers are NUL-terminated/appropriately sized.
        if unsafe {
            GetVolumePathNameW(
                path_wide.as_ptr(),
                volume_path.as_mut_ptr(),
                u32::try_from(volume_path.len()).expect("volume path buffer fits u32"),
            )
        } == 0
        {
            return Err(SecureFsError::io(
                "resolve Windows private filesystem",
                path,
                last_error(),
            ));
        }
        let mut filesystem_name = vec![0_u16; 64];
        // SAFETY: `volume_path` was initialized by GetVolumePathNameW and all output buffers match
        // their documented lengths.
        if unsafe {
            GetVolumeInformationW(
                volume_path.as_ptr(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                filesystem_name.as_mut_ptr(),
                u32::try_from(filesystem_name.len()).expect("filesystem name buffer fits u32"),
            )
        } == 0
        {
            return Err(SecureFsError::io(
                "inspect Windows private filesystem",
                path,
                last_error(),
            ));
        }
        let length = filesystem_name
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(filesystem_name.len());
        let name = String::from_utf16_lossy(&filesystem_name[..length]).to_ascii_uppercase();
        if name != "NTFS" && name != "REFS" {
            return Err(SecureFsError::unsafe_storage(format!(
                "secure private storage requires NTFS or ReFS, found {name}"
            )));
        }
        Ok(())
    }
    pub(super) fn validate_parent_directory(path: &Path) -> Result<(), SecureFsError> {
        let file = open_directory(path)
            .map_err(|source| SecureFsError::io("open secure private parent", path, source))?;
        validate_safe_directory_acl(&file, path, true, true)
    }
    pub(super) fn validate_private_directory(path: &Path) -> Result<(), SecureFsError> {
        let file = open_directory(path)
            .map_err(|source| SecureFsError::io("open secure private root", path, source))?;
        validate_private_acl(&file, path, true)
    }
    pub(super) fn validate_private_file_path(path: &Path) -> Result<(), SecureFsError> {
        let file = open_private_file(path)
            .map_err(|source| SecureFsError::io("open secure private file ACL", path, source))?;
        validate_private_acl(&file, path, false)
    }
    pub(super) fn validate_private_file_handle(
        file: &fs::File,
        path: &Path,
    ) -> Result<(), SecureFsError> {
        validate_private_acl(file, path, false)
    }
    pub(super) fn create_private_directory(path: &Path) -> io::Result<()> {
        let mut descriptor = PrivateSecurityDescriptor::new(true)?;
        let attributes = descriptor.security_attributes();
        let path = wide_path(path);
        // SAFETY: The path and descriptor remain live for the duration of CreateDirectoryW.
        if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
            return Err(last_error());
        }
        Ok(())
    }
    fn create_file_with_security(path: &Path) -> io::Result<fs::File> {
        let mut descriptor = PrivateSecurityDescriptor::new(false)?;
        let attributes = descriptor.security_attributes();
        let path = wide_path(path);
        // SAFETY: All pointers are valid; CREATE_NEW prevents namespace replacement.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                &attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_WRITE_THROUGH,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(last_error());
        }
        // SAFETY: CreateFileW returned an owned file handle.
        Ok(unsafe { fs::File::from_raw_handle(handle.cast()) })
    }
    fn open_directory_with_share(path: &Path, share_mode: u32) -> io::Result<fs::File> {
        let path = wide_path(path);
        // SAFETY: The path is a live NUL-terminated UTF-16 buffer.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | READ_CONTROL,
                share_mode,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(last_error());
        }
        // SAFETY: CreateFileW returned an owned directory handle.
        Ok(unsafe { fs::File::from_raw_handle(handle.cast()) })
    }
    fn open_metadata_object(path: &Path) -> io::Result<fs::File> {
        open_directory_with_share(path, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
    }
    fn open_directory(path: &Path) -> io::Result<fs::File> {
        open_directory_with_share(path, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
    }
    pub(super) fn pin_ancestor_directory(path: &Path) -> io::Result<fs::File> {
        open_directory_with_share(path, FILE_SHARE_READ | FILE_SHARE_WRITE)
    }
    pub(super) fn open_private_file(path: &Path) -> io::Result<fs::File> {
        let mut options = fs::OpenOptions::new();
        options
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        options.open(path)
    }
    pub(super) fn create_private_file(path: &Path) -> io::Result<fs::File> {
        create_file_with_security(path)
    }
    pub(super) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        let source = wide_path(source);
        let destination = wide_path(destination);
        // SAFETY: Both paths are live NUL-terminated UTF-16 buffers.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(last_error());
        }
        Ok(())
    }
    pub(super) fn sync_parent_after_create(_path: &Path) -> Result<(), SecureFsError> {
        // CreateDirectoryW publishes the final ACL in the same kernel operation. Windows does not
        // expose a portable directory-fsync primitive; later file replacement uses WRITE_THROUGH.
        Ok(())
    }
    pub(super) fn sync_private_directory(_path: &Path) -> Result<(), SecureFsError> {
        // MoveFileExW(MOVEFILE_WRITE_THROUGH) is the supported namespace durability boundary.
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn grant_world_parent_mutation_for_test(path: &Path) -> io::Result<()> {
        let sids = PrivateSids::current()?;
        let world = well_known_sid(WinWorldSid)?;
        let all_sids = [
            (sids.user.as_sid(), FILE_ALL_ACCESS),
            (sids.system.as_sid(), FILE_ALL_ACCESS),
            (sids.administrators.as_sid(), FILE_ALL_ACCESS),
            (world.as_sid(), FILE_ADD_FILE),
        ];
        let acl_bytes = mem::size_of::<ACL>()
            + all_sids
                .iter()
                .map(|(sid, _)| {
                    // SAFETY: Every SID is owned by a live buffer in this function.
                    let sid_length = unsafe { GetLengthSid(*sid) } as usize;
                    mem::size_of::<ACCESS_ALLOWED_ACE>()
                        .saturating_sub(mem::size_of::<u32>())
                        .saturating_add(sid_length)
                })
                .sum::<usize>();
        let mut acl_words = vec![0_usize; acl_bytes.div_ceil(mem::size_of::<usize>()).max(1)];
        let acl = acl_words.as_mut_ptr().cast::<ACL>();
        // SAFETY: `acl_words` is aligned and large enough for the computed ACL.
        if unsafe {
            InitializeAcl(
                acl,
                u32::try_from(acl_bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "ACL too large"))?,
                ACL_REVISION,
            )
        } == 0
        {
            return Err(last_error());
        }
        for (sid, mask) in all_sids {
            // SAFETY: ACL capacity was computed for every SID and all buffers are still alive.
            if unsafe { AddAccessAllowedAceEx(acl, ACL_REVISION, 0, mask, sid) } == 0 {
                return Err(last_error());
            }
        }
        let path_wide = wide_path(path);
        // SAFETY: The path is a live NUL-terminated UTF-16 buffer.
        let handle = unsafe {
            CreateFileW(
                path_wide.as_ptr(),
                READ_CONTROL | WRITE_DAC,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(last_error());
        }
        // SAFETY: CreateFileW returned an owned directory handle.
        let directory = unsafe { fs::File::from_raw_handle(handle.cast()) };
        // SAFETY: The handle and ACL are live for the duration of SetSecurityInfo.
        let result = unsafe {
            SetSecurityInfo(
                directory.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                acl,
                ptr::null(),
            )
        };
        if result != ERROR_SUCCESS {
            return Err(win32_error(result));
        }
        Ok(())
    }
}
#[cfg(not(any(unix, windows)))]
mod platform {
    use super::SecureFsError;
    use std::{fs, io, path::Path};
    fn unsupported() -> SecureFsError {
        SecureFsError::unsafe_storage(
            "secure private filesystem is supported only on Unix and Windows",
        )
    }
    pub(super) fn validate_root_syntax(_root: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_filename_syntax(_filename: &str) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
        true
    }
    pub(super) fn directory_metadata_is_supported(_metadata: &fs::Metadata) -> bool {
        false
    }
    pub(super) fn file_metadata_is_supported(_metadata: &fs::Metadata) -> bool {
        false
    }
    pub(super) fn validate_ancestor_directory(
        _path: &Path,
        _metadata: &fs::Metadata,
    ) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_supported_filesystem(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_parent_directory(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_private_directory(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_private_file_path(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn validate_private_file_handle(
        _file: &fs::File,
        _path: &Path,
    ) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn create_private_directory(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unsupported()))
    }
    pub(super) fn pin_ancestor_directory(_path: &Path) -> io::Result<fs::File> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unsupported()))
    }
    pub(super) fn open_private_file(_path: &Path) -> io::Result<fs::File> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unsupported()))
    }
    pub(super) fn create_private_file(_path: &Path) -> io::Result<fs::File> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unsupported()))
    }
    pub(super) fn replace_file(_source: &Path, _destination: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, unsupported()))
    }
    pub(super) fn sync_parent_after_create(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
    pub(super) fn sync_private_directory(_path: &Path) -> Result<(), SecureFsError> {
        Err(unsupported())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    struct MacOsAclGuard {
        path: PathBuf,
    }
    #[cfg(target_os = "macos")]
    impl Drop for MacOsAclGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/bin/chmod")
                .arg("-N")
                .arg(&self.path)
                .status();
        }
    }
    #[cfg(target_os = "macos")]
    fn add_macos_acl(path: &Path, entry: &str) -> MacOsAclGuard {
        let output = std::process::Command::new("/bin/chmod")
            .arg("+a")
            .arg(entry)
            .arg(path)
            .output()
            .expect("run macOS chmod");
        assert!(
            output.status.success(),
            "chmod +a failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        MacOsAclGuard {
            path: path.to_path_buf(),
        }
    }
    #[cfg(windows)]
    fn create_windows_junction(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd.exe")
            .arg("/D")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .expect("run mklink /J");
        assert!(
            output.status.success(),
            "mklink /J failed for {} -> {}: {}{}",
            link.display(),
            target.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn private_root(parent: &Path) -> PathBuf {
        parent.join("governance-integrity-v1")
    }
    fn canonical_parent(parent: &tempfile::TempDir) -> PathBuf {
        parent
            .path()
            .canonicalize()
            .expect("canonical temporary parent")
    }
    #[test]
    fn validates_paths_and_bounds() {
        assert_eq!(secure_private_file_abi_version(), 1);
        assert!(secure_private_directory_ensure("relative".to_owned()).is_err());
        assert!(
            secure_private_file_read("relative".to_owned(), "state.json".to_owned(), 1).is_err()
        );
        assert!(
            secure_private_file_write_atomic(
                "relative".to_owned(),
                "state.json".to_owned(),
                Buffer::from(vec![1]),
                1,
            )
            .is_err()
        );
        assert!(validate_root_path(Path::new("relative")).is_err());
        assert!(validate_root_path(Path::new("/")).is_err());
        assert!(validate_filename("").is_err());
        assert!(validate_filename("../state.json").is_err());
        assert!(validate_filename("nested/state.json").is_err());
        assert!(validate_maximum_bytes(0).is_err());
        assert!(validate_maximum_bytes(MAXIMUM_BYTES_HARD_LIMIT + 1).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn creates_private_root_and_round_trips_replacements() {
        use std::os::unix::fs::MetadataExt as _;
        let parent = tempfile::tempdir().expect("temporary parent");
        let root = private_root(&canonical_parent(&parent));
        let root_path = root.to_string_lossy().into_owned();
        secure_private_directory_ensure(root_path.clone()).expect("create private root");
        let root_metadata = fs::symlink_metadata(&root).expect("root metadata");
        assert_eq!(root_metadata.mode() & 0o777, 0o700);
        assert_eq!(root_metadata.uid(), rustix::process::geteuid().as_raw());
        let missing = secure_private_file_read(root_path.clone(), "state.json".to_owned(), 1024)
            .expect("read missing");
        assert!(missing.is_none());
        let first = br#"{"version":1}"#;
        let written = secure_private_file_write_atomic(
            root_path.clone(),
            "state.json".to_owned(),
            Buffer::from(first.to_vec()),
            1024,
        )
        .expect("first write");
        assert_eq!(written.as_ref(), first);
        let second = br#"{"version":2,"records":[1,2,3]}"#;
        let written = secure_private_file_write_atomic(
            root_path.clone(),
            "state.json".to_owned(),
            Buffer::from(second.to_vec()),
            1024,
        )
        .expect("replace");
        assert_eq!(written.as_ref(), second);
        let readback = secure_private_file_read(root_path, "state.json".to_owned(), 1024)
            .expect("read replacement")
            .expect("present");
        assert_eq!(readback.as_ref(), second);
        let file_metadata = fs::symlink_metadata(root.join("state.json")).expect("file metadata");
        assert_eq!(file_metadata.mode() & 0o777, 0o600);
        assert_eq!(file_metadata.nlink(), 1);
        assert!(fs::read_dir(&root).expect("list root").all(|entry| {
            !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".secure-private-")
        }));
    }
    #[cfg(unix)]
    #[test]
    fn rejects_permissive_roots_and_files() {
        use std::os::unix::fs::PermissionsExt as _;
        let parent = tempfile::tempdir().expect("temporary parent");
        let root = private_root(&canonical_parent(&parent));
        fs::create_dir(&root).expect("create root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
            .expect("make root permissive");
        assert!(validate_private_root(&root).is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("make root private");
        let path = root.join("state.json");
        fs::write(&path, b"state").expect("write file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("make file permissive");
        assert!(read_private_file(&root, "state.json", 1024).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_hard_links() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let parent = tempfile::tempdir().expect("temporary parent");
        let parent_path = canonical_parent(&parent);
        let root = private_root(&parent_path);
        ensure_private_root(&root).expect("create root");
        let outside = parent_path.join("outside.json");
        fs::write(&outside, b"outside").expect("write outside");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("protect outside");
        symlink(&outside, root.join("state.json")).expect("create symlink");
        assert!(read_private_file(&root, "state.json", 1024).is_err());
        fs::remove_file(root.join("state.json")).expect("remove symlink");
        fs::hard_link(&outside, root.join("state.json")).expect("create hard link");
        assert!(read_private_file(&root, "state.json", 1024).is_err());
        assert!(write_private_file(&root, "state.json", b"replacement", 1024).is_err());
        assert_eq!(fs::read(&outside).expect("outside remains"), b"outside");
    }
    #[cfg(unix)]
    #[test]
    fn rejects_empty_and_oversized_content() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let root = private_root(&canonical_parent(&parent));
        ensure_private_root(&root).expect("create root");
        assert!(write_private_file(&root, "state.json", b"", 8).is_err());
        assert!(write_private_file(&root, "state.json", b"123456789", 8).is_err());
        fs::write(root.join("state.json"), b"").expect("empty file");
        assert!(read_private_file(&root, "state.json", 8).is_err());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn accepts_deny_only_macos_ancestor_acl() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let parent_path = canonical_parent(&parent);
        let _acl = add_macos_acl(&parent_path, "everyone deny delete");
        let root = private_root(&parent_path);
        ensure_private_root(&root).expect("deny-only ACL must be safe on an ancestor");
        assert_eq!(
            write_private_file(&root, "state.json", b"private state", 1024)
                .expect("write below deny-only ancestor"),
            b"private state"
        );
        assert_eq!(
            read_private_file(&root, "state.json", 1024)
                .expect("read below deny-only ancestor")
                .expect("private state exists"),
            b"private state"
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_macos_extended_allow_acls() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let parent_path = canonical_parent(&parent);
        let root = private_root(&parent_path);
        {
            let _acl = add_macos_acl(&parent_path, "everyone allow read");
            assert!(
                ensure_private_root(&root).is_err(),
                "an extended allow ACE on an ancestor must be rejected"
            );
        }
        ensure_private_root(&root).expect("create private root after clearing ancestor ACL");
        {
            let _acl = add_macos_acl(&root, "everyone allow read");
            assert!(
                validate_private_root(&root).is_err(),
                "any extended ACL on the private root must be rejected"
            );
        }
        write_private_file(&root, "state.json", b"private state", 1024)
            .expect("create private file");
        {
            let _acl = add_macos_acl(&root.join("state.json"), "everyone allow read");
            assert!(
                read_private_file(&root, "state.json", 1024).is_err(),
                "any extended ACL on the private file must be rejected"
            );
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_rejects_the_unsupported_zero_file_id_sentinel() {
        assert!(!identity_available((0, [0; 16])));
        assert!(!identity_available((u64::MAX, [0; 16])));
        let mut supported_file_id = [0_u8; 16];
        supported_file_id[15] = 1;
        assert!(identity_available((0, supported_file_id)));
    }
    #[cfg(windows)]
    #[test]
    fn windows_round_trips_replacements_at_long_unicode_paths() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let mut parent_path = canonical_parent(&parent);
        for index in 0..6 {
            parent_path.push(format!(
                "private-storage-segment-{index:02}-with-enough-length"
            ));
        }
        fs::create_dir_all(&parent_path).expect("create long parent path");
        assert!(
            parent_path
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .count()
                > 260,
            "test path must exercise the Windows long-path boundary"
        );
        let root = private_root(&parent_path);
        ensure_private_root(&root).expect("create Windows private root");
        let root_identity = platform::path_identity(&root).expect("read Windows root identity");
        assert!(
            root_identity.1.iter().any(|byte| *byte != 0),
            "the full 128-bit file identifier must be present"
        );
        let filename = "governance-日本語-данные.json";
        assert_eq!(
            write_private_file(&root, filename, br#"{"version":1}"#, 1024)
                .expect("first Windows write"),
            br#"{"version":1}"#
        );
        assert_eq!(
            write_private_file(&root, filename, br#"{"version":2}"#, 1024)
                .expect("replace Windows file"),
            br#"{"version":2}"#
        );
        assert_eq!(
            read_private_file(&root, filename, 1024)
                .expect("read Windows replacement")
                .expect("Windows file exists"),
            br#"{"version":2}"#
        );
        assert!(
            fs::read_dir(&root)
                .expect("list Windows root")
                .all(|entry| {
                    !entry
                        .expect("Windows directory entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".secure-private-")
                })
        );
    }
    #[cfg(windows)]
    #[test]
    fn windows_rejects_broadened_parent_and_private_root_dacls() {
        let unsafe_parent = tempfile::tempdir().expect("temporary unsafe parent");
        let unsafe_parent_path = canonical_parent(&unsafe_parent);
        platform::grant_world_parent_mutation_for_test(&unsafe_parent_path)
            .expect("broaden parent DACL");
        assert!(
            ensure_private_root(&private_root(&unsafe_parent_path)).is_err(),
            "a parent DACL granting world namespace mutation must be rejected"
        );
        let private_parent = tempfile::tempdir().expect("temporary private parent");
        let private_parent_path = canonical_parent(&private_parent);
        let root = private_root(&private_parent_path);
        ensure_private_root(&root).expect("create Windows private root");
        platform::grant_world_parent_mutation_for_test(&root).expect("broaden private-root DACL");
        assert!(
            validate_private_root(&root).is_err(),
            "a private root with an extra world ACE must be rejected"
        );
    }
    #[cfg(windows)]
    #[test]
    fn windows_rejects_hard_links_and_directory_reparse_points() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let parent_path = canonical_parent(&parent);
        let root = private_root(&parent_path);
        ensure_private_root(&root).expect("create Windows private root");
        let outside = parent_path.join("outside.json");
        fs::write(&outside, b"outside").expect("write outside file");
        fs::hard_link(&outside, root.join("state.json")).expect("create Windows hard link");
        assert!(
            read_private_file(&root, "state.json", 1024).is_err(),
            "a hard-linked private file must be rejected"
        );
        assert!(
            write_private_file(&root, "state.json", b"replacement", 1024).is_err(),
            "a hard-linked destination must not be replaced"
        );
        assert_eq!(
            fs::read(&outside).expect("outside file remains"),
            b"outside"
        );
        let reparse_parent = tempfile::tempdir().expect("temporary reparse parent");
        let reparse_parent_path = canonical_parent(&reparse_parent);
        let target = reparse_parent_path.join("junction-target");
        fs::create_dir(&target).expect("create junction target");
        let junction_root = private_root(&reparse_parent_path);
        create_windows_junction(&junction_root, &target);
        assert!(
            ensure_private_root(&junction_root).is_err(),
            "a reparse-point private root must be rejected"
        );
        fs::remove_dir(&junction_root).expect("remove test junction");
    }
    #[cfg(windows)]
    #[test]
    fn windows_rejects_ambiguous_and_reserved_paths() {
        assert!(validate_filename("NUL.json").is_err());
        assert!(validate_filename("COM1").is_err());
        assert!(validate_filename("state.json.").is_err());
        assert!(validate_filename("state.json ").is_err());
        assert!(validate_filename("state:stream").is_err());
        assert!(validate_filename("state?.json").is_err());
        assert!(validate_root_path(Path::new(r"\\server\share\private")).is_err());
        assert!(validate_root_path(Path::new(r"\root-relative\private")).is_err());
        assert!(validate_root_path(Path::new(r"C:\safe\CON\private")).is_err());
        assert!(validate_root_path(Path::new(r"C:\safe\private.")).is_err());
        assert!(validate_filename("governance-日本語-данные.json").is_ok());
        assert!(validate_root_path(Path::new(r"C:\safe\governance-integrity-v1")).is_ok());
        assert!(validate_root_path(Path::new(r"\\?\C:\safe\governance-integrity-v1")).is_ok());
    }
}
