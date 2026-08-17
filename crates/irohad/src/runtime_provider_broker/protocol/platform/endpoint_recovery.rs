//! Exclusive broker-instance locking and identity-pinned endpoint removal.
use super::{RuntimeProviderBrokerServerErrorV1, SocketIdentity, socket_identity_from_stat};
use std::{ffi::OsStr, fmt::Write as _, fs};
const INSTANCE_LOCK_NAME: &str = ".runtime-provider-broker-v1.lock";
const INSTANCE_LOCK_MODE: rustix::fs::RawMode = 0o600;
const QUARANTINE_NAME_ATTEMPTS: usize = 4;
#[derive(Clone, Copy)]
struct ExactSocketEntry {
    identity: SocketIdentity,
    expected_service_uid: u32,
    socket_mode: u32,
}
/// Full-lifetime exclusive lock proving that no conforming broker is active.
pub(super) struct InstanceLockGuard {
    file: fs::File,
    identity: SocketIdentity,
    expected_service_uid: u32,
    marker_preexisted: bool,
}
impl InstanceLockGuard {
    /// Open or exclusively create, validate, and lock the fixed instance file.
    pub(super) fn acquire(
        parent_directory: &fs::File,
        expected_service_uid: u32,
    ) -> Result<Self, RuntimeProviderBrokerServerErrorV1> {
        let (file, marker_preexisted) = open_lock_file(parent_directory)?;
        let opened = rustix::fs::fstat(&file)
            .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
        if !lock_metadata_is_exact(&opened, expected_service_uid) {
            return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
        }
        let identity = socket_identity_from_stat(&opened);
        verify_lock_entry(parent_directory, identity, expected_service_uid)?;
        rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
        let guard = Self {
            file,
            identity,
            expected_service_uid,
            marker_preexisted,
        };
        guard.verify(parent_directory)?;
        Ok(guard)
    }
    /// Revalidate both the held descriptor and its fixed directory entry.
    pub(super) fn verify(
        &self,
        parent_directory: &fs::File,
    ) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
        let opened = rustix::fs::fstat(&self.file)
            .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
        if !lock_metadata_is_exact(&opened, self.expected_service_uid)
            || socket_identity_from_stat(&opened) != self.identity
        {
            return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
        }
        verify_lock_entry(parent_directory, self.identity, self.expected_service_uid)
    }
    fn discard_new_marker(
        self,
        parent_directory: &fs::File,
    ) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
        if self.marker_preexisted {
            return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
        }
        self.verify(parent_directory)?;
        let quarantine_name =
            rename_to_quarantine(parent_directory, OsStr::new(INSTANCE_LOCK_NAME), ".l-")?;
        let quarantined = rustix::fs::statat(
            parent_directory,
            quarantine_name.as_os_str(),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)?;
        let quarantine_identity = socket_identity_from_stat(&quarantined);
        if !lock_metadata_is_exact(&quarantined, self.expected_service_uid)
            || quarantine_identity != self.identity
        {
            restore_quarantined_entry(
                parent_directory,
                quarantine_name.as_os_str(),
                OsStr::new(INSTANCE_LOCK_NAME),
            );
            return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
        }
        rustix::fs::unlinkat(
            parent_directory,
            quarantine_name.as_os_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)?;
        match rustix::fs::statat(
            parent_directory,
            quarantine_name.as_os_str(),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Ok(_) | Err(_) => Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed),
        }
    }
    #[cfg(test)]
    /// Report whether acquisition opened a marker left by an earlier instance.
    pub(super) const fn marker_preexisted(&self) -> bool {
        self.marker_preexisted
    }
}
/// Acquire the instance lock and recover only a socket backed by an old marker.
pub(super) fn prepare_endpoint(
    parent_directory: &fs::File,
    socket_name: &OsStr,
    expected_service_uid: u32,
    socket_mode: u32,
) -> Result<InstanceLockGuard, RuntimeProviderBrokerServerErrorV1> {
    let guard = InstanceLockGuard::acquire(parent_directory, expected_service_uid)?;
    let observed = match socket_metadata(parent_directory, socket_name) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(guard),
        Err(error) => {
            if guard.marker_preexisted {
                return Err(error);
            }
            return match guard.discard_new_marker(parent_directory) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(cleanup_error),
            };
        }
    };
    if !guard.marker_preexisted {
        return match guard.discard_new_marker(parent_directory) {
            Ok(()) => Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
            Err(cleanup_error) => Err(cleanup_error),
        };
    }
    if !socket_metadata_is_exact(&observed, expected_service_uid, socket_mode) {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    let identity = socket_identity_from_stat(&observed);
    remove_exact_socket_entry_inner(
        parent_directory,
        socket_name,
        ExactSocketEntry {
            identity,
            expected_service_uid,
            socket_mode,
        },
        &guard,
        RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
        || {},
    )?;
    guard.verify(parent_directory)?;
    Ok(guard)
}
/// Remove one exact bound endpoint through an unpredictable quarantine name.
pub(super) fn cleanup_socket_entry(
    parent_directory: &fs::File,
    socket_name: &OsStr,
    identity: SocketIdentity,
    expected_service_uid: u32,
    socket_mode: u32,
    guard: &InstanceLockGuard,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    remove_exact_socket_entry_inner(
        parent_directory,
        socket_name,
        ExactSocketEntry {
            identity,
            expected_service_uid,
            socket_mode,
        },
        guard,
        RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed,
        || {},
    )
}
/// Return an unpredictable, fixed-width staging socket basename.
pub(super) fn staging_socket_name() -> Result<std::ffi::OsString, RuntimeProviderBrokerServerErrorV1>
{
    random_private_name(".b-")
}
#[cfg(test)]
/// Exercise stale recovery with a substitution before the quarantine rename.
pub(super) fn recover_stale_endpoint_with_probe<F>(
    parent_directory: &fs::File,
    socket_name: &OsStr,
    expected_service_uid: u32,
    socket_mode: u32,
    guard: &InstanceLockGuard,
    before_quarantine_rename: F,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    F: FnOnce(),
{
    if !guard.marker_preexisted {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    let observed = socket_metadata(parent_directory, socket_name)?
        .ok_or(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    if !socket_metadata_is_exact(&observed, expected_service_uid, socket_mode) {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    remove_exact_socket_entry_inner(
        parent_directory,
        socket_name,
        ExactSocketEntry {
            identity: socket_identity_from_stat(&observed),
            expected_service_uid,
            socket_mode,
        },
        guard,
        RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
        before_quarantine_rename,
    )
}
#[cfg(test)]
/// Exercise orderly cleanup with a substitution before the quarantine rename.
pub(super) fn cleanup_socket_entry_with_probe<F>(
    parent_directory: &fs::File,
    socket_name: &OsStr,
    identity: SocketIdentity,
    expected_service_uid: u32,
    socket_mode: u32,
    guard: &InstanceLockGuard,
    before_quarantine_rename: F,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    F: FnOnce(),
{
    remove_exact_socket_entry_inner(
        parent_directory,
        socket_name,
        ExactSocketEntry {
            identity,
            expected_service_uid,
            socket_mode,
        },
        guard,
        RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed,
        before_quarantine_rename,
    )
}
fn open_lock_file(
    parent_directory: &fs::File,
) -> Result<(fs::File, bool), RuntimeProviderBrokerServerErrorV1> {
    match open_existing_lock(parent_directory) {
        Ok(file) => Ok((file, true)),
        Err(rustix::io::Errno::NOENT) => match create_lock_exclusively(parent_directory) {
            Ok(file) => Ok((file, false)),
            Err(rustix::io::Errno::EXIST) => open_existing_lock(parent_directory)
                .map(|file| (file, true))
                .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
            Err(_) => Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
        },
        Err(_) => Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
    }
}
fn open_existing_lock(parent_directory: &fs::File) -> rustix::io::Result<fs::File> {
    rustix::fs::openat(
        parent_directory,
        INSTANCE_LOCK_NAME,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
}
fn create_lock_exclusively(parent_directory: &fs::File) -> rustix::io::Result<fs::File> {
    rustix::fs::openat(
        parent_directory,
        INSTANCE_LOCK_NAME,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(INSTANCE_LOCK_MODE),
    )
    .map(fs::File::from)
}
fn remove_exact_socket_entry_inner<F>(
    parent_directory: &fs::File,
    socket_name: &OsStr,
    exact: ExactSocketEntry,
    guard: &InstanceLockGuard,
    mismatch_error: RuntimeProviderBrokerServerErrorV1,
    before_quarantine_rename: F,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    F: FnOnce(),
{
    guard.verify(parent_directory)?;
    let observed = match socket_metadata(parent_directory, socket_name) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(()),
        Err(_) => return Err(mismatch_error),
    };
    if !socket_metadata_is_exact(&observed, exact.expected_service_uid, exact.socket_mode)
        || socket_identity_from_stat(&observed) != exact.identity
    {
        return Err(mismatch_error);
    }
    before_quarantine_rename();
    guard.verify(parent_directory)?;
    let quarantine_name = rename_to_quarantine(parent_directory, socket_name, ".q-")?;
    let quarantined = if let Ok(Some(metadata)) =
        socket_metadata(parent_directory, quarantine_name.as_os_str())
    {
        metadata
    } else {
        restore_quarantined_entry(parent_directory, quarantine_name.as_os_str(), socket_name);
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
    };
    let quarantine_identity = socket_identity_from_stat(&quarantined);
    if !socket_metadata_is_exact(&quarantined, exact.expected_service_uid, exact.socket_mode)
        || quarantine_identity != exact.identity
    {
        if restore_quarantined_entry(parent_directory, quarantine_name.as_os_str(), socket_name) {
            return Err(mismatch_error);
        }
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
    }
    if guard.verify(parent_directory).is_err() {
        restore_quarantined_entry(parent_directory, quarantine_name.as_os_str(), socket_name);
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
    }
    rustix::fs::unlinkat(
        parent_directory,
        quarantine_name.as_os_str(),
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)?;
    match socket_metadata(parent_directory, quarantine_name.as_os_str()) {
        Ok(None) => Ok(()),
        Ok(Some(_)) | Err(_) => Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed),
    }
}
fn rename_to_quarantine(
    parent_directory: &fs::File,
    entry_name: &OsStr,
    prefix: &str,
) -> Result<std::ffi::OsString, RuntimeProviderBrokerServerErrorV1> {
    for _ in 0..QUARANTINE_NAME_ATTEMPTS {
        let quarantine_name = random_private_name(prefix)
            .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)?;
        match rustix::fs::renameat_with(
            parent_directory,
            entry_name,
            parent_directory,
            quarantine_name.as_os_str(),
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => return Ok(quarantine_name),
            Err(rustix::io::Errno::EXIST) => {}
            Err(_) => return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed),
        }
    }
    Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)
}
fn restore_quarantined_entry(
    parent_directory: &fs::File,
    quarantine_name: &OsStr,
    socket_name: &OsStr,
) -> bool {
    rustix::fs::renameat_with(
        parent_directory,
        quarantine_name,
        parent_directory,
        socket_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .is_ok()
}
fn random_private_name(
    prefix: &str,
) -> Result<std::ffi::OsString, RuntimeProviderBrokerServerErrorV1> {
    let mut nonce = [0_u8; 12];
    rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut nonce)
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    if nonce == [0; 12] {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    let mut name = String::with_capacity(27);
    name.push_str(prefix);
    for byte in nonce {
        write!(&mut name, "{byte:02x}")
            .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    }
    Ok(name.into())
}
fn socket_metadata(
    parent_directory: &fs::File,
    socket_name: &OsStr,
) -> Result<Option<rustix::fs::Stat>, RuntimeProviderBrokerServerErrorV1> {
    match rustix::fs::statat(
        parent_directory,
        socket_name,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(_) => Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
    }
}
fn verify_lock_entry(
    parent_directory: &fs::File,
    identity: SocketIdentity,
    expected_service_uid: u32,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    let entry = rustix::fs::statat(
        parent_directory,
        INSTANCE_LOCK_NAME,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    if !lock_metadata_is_exact(&entry, expected_service_uid)
        || socket_identity_from_stat(&entry) != identity
    {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    Ok(())
}
fn lock_metadata_is_exact(metadata: &rustix::fs::Stat, expected_service_uid: u32) -> bool {
    rustix::fs::FileType::from_raw_mode(metadata.st_mode) == rustix::fs::FileType::RegularFile
        && metadata.st_uid == expected_service_uid
        && u32::from(metadata.st_mode & 0o7777) == u32::from(INSTANCE_LOCK_MODE)
        && metadata.st_nlink == 1
}
pub(super) fn socket_metadata_is_exact(
    metadata: &rustix::fs::Stat,
    expected_service_uid: u32,
    socket_mode: u32,
) -> bool {
    rustix::fs::FileType::from_raw_mode(metadata.st_mode) == rustix::fs::FileType::Socket
        && metadata.st_uid == expected_service_uid
        && u32::from(metadata.st_mode & 0o7777) == socket_mode
        && metadata.st_nlink == 1
}
