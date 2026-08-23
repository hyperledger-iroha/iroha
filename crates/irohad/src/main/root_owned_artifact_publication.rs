//! Root-custodied, immutable, no-replace artifact publication.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::{ffi::OsString, fs};

/// Failure before or after the no-replace commit boundary.
#[derive(Debug)]
pub(super) enum RootOwnedArtifactPublicationError {
    /// Publication did not cross the no-replace rename boundary.
    PreCommit(String),
    /// Rename succeeded, but durable or semantic confirmation failed.
    ///
    /// The final inode is deliberately left in place and must be reconciled by
    /// an operator; callers must never retry publication automatically.
    CommitUncertain {
        /// Human-readable artifact class.
        label: &'static str,
        /// Immutable final path whose commit must be reconciled.
        path: PathBuf,
        /// Failed post-commit confirmation.
        detail: String,
    },
}

impl std::fmt::Display for RootOwnedArtifactPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreCommit(message) => formatter.write_str(message),
            Self::CommitUncertain {
                label,
                path,
                detail,
            } => write!(
                formatter,
                "{label} publication commit-uncertain at `{}`: RENAME_NOREPLACE succeeded and the final inode was left in place: {detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RootOwnedArtifactPublicationError {}

impl RootOwnedArtifactPublicationError {
    fn pre_commit(message: impl Into<String>) -> Self {
        Self::PreCommit(message.into())
    }
}

/// Pinned destination for one root-owned, non-replaceable artifact.
#[derive(Debug)]
pub(super) struct RootOwnedNoReplaceArtifactPublicationTarget {
    path: PathBuf,
    label: &'static str,
    #[cfg(unix)]
    parent: fs::File,
    #[cfg(unix)]
    file_name: OsString,
    #[cfg(unix)]
    expected_uid: u32,
}

#[cfg(target_os = "macos")]
const MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES: usize = 64 * 1024;
#[cfg(target_os = "macos")]
const MACOS_XATTR_SHOWCOMPRESSION: std::os::raw::c_int = 0x20;
#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "Linux exposes descriptor-bound xattr inspection through libc"
)]
unsafe extern "C" {
    fn flistxattr(fd: std::os::raw::c_int, list: *mut std::os::raw::c_char, size: usize) -> isize;
}
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "macOS exposes descriptor-bound xattr inspection with hidden compression metadata through libc"
)]
unsafe extern "C" {
    fn flistxattr(
        fd: std::os::raw::c_int,
        list: *mut std::os::raw::c_char,
        size: usize,
        options: std::os::raw::c_int,
    ) -> isize;
}

#[cfg(target_os = "macos")]
fn run_bounded_macos_acl_command(
    program: &str,
    option: &str,
    path: &Path,
    label: &str,
) -> Result<std::process::Output, String> {
    let output = std::process::Command::new(program)
        .arg(option)
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| {
            format!(
                "failed to run macOS ACL command for {label} `{}`: {error}",
                path.display()
            )
        })?;
    if output.stdout.len() > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        || output.stderr.len() > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
    {
        return Err(format!(
            "macOS ACL command output exceeded its bound for {label} `{}`",
            path.display()
        ));
    }
    if !output.status.success() {
        return Err(format!(
            "macOS ACL command failed for {label} `{}`: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

#[cfg(target_os = "macos")]
/// Reject a path carrying any macOS extended ACL entries.
pub(super) fn require_no_macos_extended_acl(path: &Path, label: &str) -> Result<(), String> {
    let output = run_bounded_macos_acl_command("/bin/ls", "-ldeq", path, label)?;
    let suffix = output.stdout.strip_suffix(b"\n");
    if !output.stderr.is_empty() || suffix.is_none_or(|body| body.contains(&b'\n')) {
        return Err(format!(
            "{label} `{}` must not have an extended ACL",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_macos_extended_acl(path: &Path, label: &str) -> Result<(), String> {
    let output = run_bounded_macos_acl_command("/bin/chmod", "-N", path, label)?;
    if !output.stdout.is_empty() || !output.stderr.is_empty() {
        return Err(format!(
            "macOS ACL removal produced unexpected output for {label} `{}`",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(unix)]
fn same_custody_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
}

#[cfg(unix)]
fn stat_matches_metadata(stat: &rustix::fs::Stat, metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::RegularFile
        && u64::try_from(stat.st_dev).ok() == Some(metadata.dev())
        && stat.st_ino == metadata.ino()
        && u64::from(stat.st_nlink) == metadata.nlink()
        && stat.st_uid == metadata.uid()
        && stat.st_gid == metadata.gid()
        && u32::from(stat.st_mode) == metadata.mode()
        && u64::try_from(stat.st_size).ok() == Some(metadata.len())
}

#[cfg(target_os = "macos")]
fn require_acl_free_pinned_path(opened: &fs::File, path: &Path, label: &str) -> Result<(), String> {
    let opened_before = opened
        .metadata()
        .map_err(|error| format!("failed to inspect pinned {label}: {error}"))?;
    let path_before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
    if !same_metadata(&opened_before, &path_before) {
        return Err(format!(
            "{label} `{}` no longer identifies the pinned inode",
            path.display()
        ));
    }
    require_no_macos_extended_acl(path, label)?;
    let opened_after = opened
        .metadata()
        .map_err(|error| format!("failed to re-inspect pinned {label}: {error}"))?;
    let path_after = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to re-inspect {label} `{}` after ACL validation: {error}",
            path.display()
        )
    })?;
    if !same_metadata(&opened_before, &opened_after) || !same_metadata(&opened_after, &path_after) {
        return Err(format!(
            "{label} `{}` changed during ACL validation",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    unsafe_code,
    reason = "descriptor-bound xattr inspection requires the platform libc"
)]
fn require_no_xattrs(opened: &fs::File, path: &Path, label: &str) -> Result<(), String> {
    use std::os::fd::AsRawFd as _;

    // SAFETY: a null buffer and zero length request the descriptor-bound size;
    // the retained descriptor remains valid for the call.
    #[cfg(target_os = "linux")]
    let count = unsafe { flistxattr(opened.as_raw_fd(), std::ptr::null_mut(), 0) };
    #[cfg(target_os = "macos")]
    let count = unsafe {
        flistxattr(
            opened.as_raw_fd(),
            std::ptr::null_mut(),
            0,
            MACOS_XATTR_SHOWCOMPRESSION,
        )
    };
    if count < 0 {
        return Err(format!(
            "failed to inspect descriptor-bound xattrs for {label} `{}`: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    if count != 0 {
        return Err(format!("{label} `{}` must be xattr-free", path.display()));
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn macos_xattr_queries_include_hidden_compression_metadata() {
    assert_eq!(MACOS_XATTR_SHOWCOMPRESSION, 0x20);
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn require_no_xattrs(_opened: &fs::File, path: &Path, label: &str) -> Result<(), String> {
    Err(format!(
        "descriptor-bound xattr inspection is unsupported for {label} `{}` on this platform",
        path.display()
    ))
}

impl RootOwnedNoReplaceArtifactPublicationTarget {
    /// Prepare an absent root-owned destination while pinning its parent.
    pub(super) fn prepare_root_owned(
        path: &Path,
        label: &'static str,
    ) -> Result<Self, RootOwnedArtifactPublicationError> {
        #[cfg(unix)]
        {
            let effective_uid = rustix::process::geteuid().as_raw();
            if effective_uid != 0 {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} publication requires effective uid 0, got {effective_uid}"
                )));
            }
            Self::pin_for_owner(path, 0, label, true)
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "root-owned atomic {label} publication is unsupported on this platform"
            )))
        }
    }

    /// Return the immutable final path.
    #[must_use]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Read one bounded, stable artifact from a root-owned immutable path.
    pub(super) fn read_root_owned_bounded(
        path: &Path,
        max_bytes: usize,
        label: &'static str,
    ) -> Result<Vec<u8>, String> {
        #[cfg(unix)]
        {
            let effective_uid = rustix::process::geteuid().as_raw();
            if effective_uid != 0 {
                return Err(format!(
                    "{label} read requires effective uid 0, got {effective_uid}"
                ));
            }
            Self::pin_for_owner(path, 0, label, false)
                .map_err(|error| error.to_string())?
                .read_pinned_bounded(max_bytes)
        }
        #[cfg(not(unix))]
        {
            let _ = (path, max_bytes);
            Err(format!(
                "root-owned stable {label} reads are unsupported on this platform"
            ))
        }
    }

    /// Read using an explicit owner; exposed for Unix custody tests.
    #[cfg(all(test, unix))]
    pub(super) fn read_bounded_for_owner(
        path: &Path,
        max_bytes: usize,
        expected_uid: u32,
        label: &'static str,
    ) -> Result<Vec<u8>, String> {
        Self::pin_for_owner(path, expected_uid, label, false)
            .map_err(|error| error.to_string())?
            .read_pinned_bounded(max_bytes)
    }

    #[cfg(unix)]
    fn read_pinned_bounded(self, max_bytes: usize) -> Result<Vec<u8>, String> {
        use std::io::Read as _;

        if max_bytes == 0 {
            return Err(format!("{} byte limit must be positive", self.label));
        }
        self.verify_parent_identity()?;
        let named_before = rustix::fs::statat(
            &self.parent,
            &self.file_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|error| format!("failed to inspect {} before read: {error}", self.label))?;
        if rustix::fs::FileType::from_raw_mode(named_before.st_mode)
            != rustix::fs::FileType::RegularFile
            || u64::from(named_before.st_nlink) != 1
            || named_before.st_uid != self.expected_uid
            || u32::from(named_before.st_mode) & 0o7777 != 0o444
        {
            return Err(format!(
                "{} must be a direct single-link regular file owned by uid {} with mode 0444",
                self.label, self.expected_uid
            ));
        }
        let mut opened = fs::File::from(
            rustix::fs::openat(
                &self.parent,
                &self.file_name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| format!("failed to open pinned {}: {error}", self.label))?,
        );
        let opened_before = opened
            .metadata()
            .map_err(|error| format!("failed to inspect opened {}: {error}", self.label))?;
        let path_before = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to inspect named {} `{}`: {error}",
                self.label,
                self.path.display()
            )
        })?;
        if !opened_before.is_file()
            || !stat_matches_metadata(&named_before, &opened_before)
            || !same_metadata(&opened_before, &path_before)
        {
            return Err(format!("{} changed identity while opening", self.label));
        }
        require_no_xattrs(&opened, &self.path, self.label)?;
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(&opened, &self.path, self.label)?;
        let length = usize::try_from(opened_before.len())
            .map_err(|_| format!("{} length does not fit usize", self.label))?;
        if length == 0 || length > max_bytes {
            return Err(format!(
                "{} length {length} is outside 1..={max_bytes} bytes",
                self.label
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length.saturating_add(1))
            .map_err(|error| format!("failed to reserve {} read buffer: {error}", self.label))?;
        opened
            .by_ref()
            .take(opened_before.len().saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read exact {} bytes: {error}", self.label))?;
        if bytes.len() != length {
            return Err(format!("{} changed length while reading", self.label));
        }
        require_no_xattrs(&opened, &self.path, self.label)?;
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(&opened, &self.path, self.label)?;
        let opened_after = opened
            .metadata()
            .map_err(|error| format!("failed to re-inspect opened {}: {error}", self.label))?;
        let named_after = rustix::fs::statat(
            &self.parent,
            &self.file_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|error| format!("failed to re-inspect named {}: {error}", self.label))?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to re-inspect {} `{}` after read: {error}",
                self.label,
                self.path.display()
            )
        })?;
        if !same_metadata(&opened_before, &opened_after)
            || !same_metadata(&opened_after, &path_after)
            || !stat_matches_metadata(&named_after, &opened_after)
        {
            return Err(format!(
                "{} changed identity or metadata while reading",
                self.label
            ));
        }
        self.verify_parent_identity()?;
        Ok(bytes)
    }

    /// Prepare using an explicit owner; exposed for Unix custody tests.
    #[cfg(all(test, unix))]
    pub(super) fn prepare_for_owner(
        path: &Path,
        expected_uid: u32,
        label: &'static str,
    ) -> Result<Self, RootOwnedArtifactPublicationError> {
        Self::pin_for_owner(path, expected_uid, label, true)
    }

    #[cfg(unix)]
    #[expect(clippy::too_many_lines, reason = "ordered path-custody audit")]
    fn pin_for_owner(
        path: &Path,
        expected_uid: u32,
        label: &'static str,
        require_absent: bool,
    ) -> Result<Self, RootOwnedArtifactPublicationError> {
        use std::os::unix::fs::MetadataExt as _;

        validate_canonical_absolute_path(path, label)
            .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
        let file_name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} path must end in a file name"
                ))
            })?
            .to_owned();
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} path must have an absolute parent"
                ))
            })?;
        let canonical_parent = fs::canonicalize(parent_path).map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to canonicalize {label} parent `{}`: {error}",
                parent_path.display()
            ))
        })?;
        if canonical_parent != parent_path {
            return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "{label} parent path must already be canonical: `{}` resolves to `{}`",
                parent_path.display(),
                canonical_parent.display()
            )));
        }
        let mut ancestors = parent_path.ancestors().collect::<Vec<_>>();
        ancestors.reverse();
        for ancestor in ancestors {
            let metadata = fs::symlink_metadata(ancestor).map_err(|error| {
                RootOwnedArtifactPublicationError::pre_commit(format!(
                    "failed to inspect {label} parent `{}`: {error}",
                    ancestor.display()
                ))
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} parent chain contains a symlink or non-directory `{}`",
                    ancestor.display()
                )));
            }
            if metadata.uid() != 0 && metadata.uid() != expected_uid {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} parent `{}` is owned by untrusted uid {}",
                    ancestor.display(),
                    metadata.uid()
                )));
            }
            if metadata.mode() & 0o022 != 0 {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} parent `{}` is group- or world-writable",
                    ancestor.display()
                )));
            }
            #[cfg(target_os = "macos")]
            {
                require_no_macos_extended_acl(ancestor, &format!("{label} parent"))
                    .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
                let after = fs::symlink_metadata(ancestor).map_err(|error| {
                    RootOwnedArtifactPublicationError::pre_commit(format!(
                        "failed to re-inspect {label} parent `{}` after ACL validation: {error}",
                        ancestor.display()
                    ))
                })?;
                if !same_metadata(&metadata, &after) {
                    return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                        "{label} parent `{}` changed during ACL validation",
                        ancestor.display()
                    )));
                }
            }
            #[cfg(unix)]
            {
                let opened = fs::File::from(
                    rustix::fs::open(
                        ancestor,
                        rustix::fs::OFlags::RDONLY
                            | rustix::fs::OFlags::DIRECTORY
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::CLOEXEC,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(|error| {
                        RootOwnedArtifactPublicationError::pre_commit(format!(
                            "failed to pin {label} parent `{}` for xattr validation: {error}",
                            ancestor.display()
                        ))
                    })?,
                );
                require_no_xattrs(&opened, ancestor, &format!("{label} parent"))
                    .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
                let opened_after = opened.metadata().map_err(|error| {
                    RootOwnedArtifactPublicationError::pre_commit(format!(
                        "failed to re-inspect pinned {label} parent `{}`: {error}",
                        ancestor.display()
                    ))
                })?;
                let path_after = fs::symlink_metadata(ancestor).map_err(|error| {
                    RootOwnedArtifactPublicationError::pre_commit(format!(
                        "failed to re-inspect {label} parent `{}` after xattr validation: {error}",
                        ancestor.display()
                    ))
                })?;
                if !same_metadata(&metadata, &opened_after)
                    || !same_metadata(&opened_after, &path_after)
                {
                    return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                        "{label} parent `{}` changed during xattr validation",
                        ancestor.display()
                    )));
                }
            }
        }
        let direct_parent = fs::symlink_metadata(parent_path).map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to inspect {label} destination directory `{}`: {error}",
                parent_path.display()
            ))
        })?;
        if direct_parent.uid() != expected_uid {
            return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "{label} destination directory `{}` must be owned by uid {expected_uid}",
                parent_path.display()
            )));
        }
        let parent = fs::File::from(
            rustix::fs::open(
                parent_path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| {
                RootOwnedArtifactPublicationError::pre_commit(format!(
                    "failed to pin {label} destination directory `{}`: {error}",
                    parent_path.display()
                ))
            })?,
        );
        let opened = parent.metadata().map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to inspect pinned {label} destination directory: {error}"
            ))
        })?;
        let current = fs::symlink_metadata(parent_path).map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to re-inspect {label} destination directory: {error}"
            ))
        })?;
        if !opened.is_dir()
            || !same_metadata(&opened, &current)
            || opened.uid() != expected_uid
            || opened.mode() & 0o022 != 0
        {
            return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "{label} destination directory changed or became untrusted while opening"
            )));
        }
        require_no_xattrs(
            &parent,
            parent_path,
            &format!("{label} destination directory"),
        )
        .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(
            &parent,
            parent_path,
            &format!("{label} destination directory"),
        )
        .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
        let opened_after = parent.metadata().map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to re-inspect pinned {label} destination directory: {error}"
            ))
        })?;
        let path_after = fs::symlink_metadata(parent_path).map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to re-inspect {label} destination directory after custody validation: {error}"
            ))
        })?;
        if !same_metadata(&opened, &opened_after) || !same_metadata(&opened_after, &path_after) {
            return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "{label} destination directory changed during custody validation"
            )));
        }
        match rustix::fs::statat(&parent, &file_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) if require_absent => {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} destination already exists and will not be replaced: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error == rustix::io::Errno::NOENT && require_absent => {}
            Err(error) if error == rustix::io::Errno::NOENT => {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "{label} does not exist: {}",
                    path.display()
                )));
            }
            Err(error) => {
                return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                    "failed to inspect {label} destination `{}`: {error}",
                    path.display()
                )));
            }
        }
        Ok(Self {
            path: path.to_owned(),
            label,
            parent,
            file_name,
            expected_uid,
        })
    }

    /// Publish exact bytes once and run semantic verification after commit.
    #[cfg(unix)]
    #[expect(clippy::too_many_lines, reason = "ordered no-replace commit protocol")]
    pub(super) fn publish_bytes_and_verify(
        self,
        canonical_bytes: &[u8],
        verify_final: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), RootOwnedArtifactPublicationError> {
        use std::{
            io::{Read as _, Seek as _, SeekFrom, Write as _},
            os::unix::fs::MetadataExt as _,
        };

        if canonical_bytes.is_empty() {
            return Err(RootOwnedArtifactPublicationError::pre_commit(format!(
                "canonical {} bytes must not be empty",
                self.label
            )));
        }
        self.verify_parent_identity()
            .map_err(RootOwnedArtifactPublicationError::pre_commit)?;
        let mut nonce = [0_u8; 16];
        rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut nonce).map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "operating-system randomness unavailable for {} staging: {error}",
                self.label
            ))
        })?;
        let staging_name = OsString::from(format!(
            ".irohad-immutable-artifact-{}.tmp",
            hex::encode(nonce)
        ));
        let staging_path = self
            .path
            .parent()
            .expect("prepared artifact path has a parent")
            .join(&staging_name);
        let mut staging = fs::File::from(
            rustix::fs::openat(
                &self.parent,
                &staging_name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_raw_mode(0o600),
            )
            .map_err(|error| {
                RootOwnedArtifactPublicationError::pre_commit(format!(
                    "failed to create exclusive {} staging file: {error}",
                    self.label
                ))
            })?,
        );
        let initial = staging.metadata().map_err(|error| {
            RootOwnedArtifactPublicationError::pre_commit(format!(
                "failed to inspect opened {} staging file; its exclusive name was left in place: {error}",
                self.label
            ))
        })?;
        let staging_identity = (initial.dev(), initial.ino());
        let staged_result = (|| -> Result<(u64, u64), String> {
            staging
                .write_all(canonical_bytes)
                .map_err(|error| format!("failed to write {} staging file: {error}", self.label))?;
            staging
                .flush()
                .map_err(|error| format!("failed to flush {} staging file: {error}", self.label))?;
            staging
                .sync_all()
                .map_err(|error| format!("failed to sync {} staging file: {error}", self.label))?;
            rustix::fs::fchmod(&staging, rustix::fs::Mode::from_raw_mode(0o444)).map_err(
                |error| {
                    format!(
                        "failed to make {} staging file immutable: {error}",
                        self.label
                    )
                },
            )?;
            staging.sync_all().map_err(|error| {
                format!(
                    "failed to sync immutable {} staging file: {error}",
                    self.label
                )
            })?;
            let before_acl_clear = staging.metadata().map_err(|error| {
                format!("failed to inspect {} staging file: {error}", self.label)
            })?;
            let named_before_acl_clear = rustix::fs::statat(
                &self.parent,
                &staging_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| {
                format!(
                    "failed to bind {} staging name before ACL removal: {error}",
                    self.label
                )
            })?;
            if u64::try_from(named_before_acl_clear.st_dev).ok() != Some(before_acl_clear.dev())
                || named_before_acl_clear.st_ino != before_acl_clear.ino()
                || u64::from(named_before_acl_clear.st_nlink) != 1
            {
                return Err(format!(
                    "{} staging name changed before ACL removal",
                    self.label
                ));
            }
            #[cfg(target_os = "macos")]
            {
                clear_macos_extended_acl(&staging_path, &format!("{} staging file", self.label))?;
                staging.sync_all().map_err(|error| {
                    format!(
                        "failed to sync ACL-free {} staging file: {error}",
                        self.label
                    )
                })?;
                require_acl_free_pinned_path(
                    &staging,
                    &staging_path,
                    &format!("{} staging file", self.label),
                )?;
            }
            require_no_xattrs(
                &staging,
                &staging_path,
                &format!("{} staging file", self.label),
            )?;
            let metadata = staging.metadata().map_err(|error| {
                format!(
                    "failed to inspect ACL-free {} staging file: {error}",
                    self.label
                )
            })?;
            if !metadata.is_file()
                || metadata.dev() != staging_identity.0
                || metadata.ino() != staging_identity.1
                || metadata.nlink() != 1
                || metadata.uid() != self.expected_uid
                || metadata.mode() & 0o7777 != 0o444
                || metadata.len()
                    != u64::try_from(canonical_bytes.len())
                        .map_err(|_| format!("{} length does not fit u64", self.label))?
            {
                return Err(format!(
                    "{} staging file ownership, mode, links, length, or inode is invalid",
                    self.label
                ));
            }
            let named = rustix::fs::statat(
                &self.parent,
                &staging_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| format!("failed to bind {} staging name: {error}", self.label))?;
            if u64::try_from(named.st_dev).ok() != Some(metadata.dev())
                || named.st_ino != metadata.ino()
                || u64::from(named.st_nlink) != 1
            {
                return Err(format!(
                    "{} staging name no longer identifies the opened file",
                    self.label
                ));
            }
            staging.seek(SeekFrom::Start(0)).map_err(|error| {
                format!("failed to rewind {} staging file: {error}", self.label)
            })?;
            let mut readback = Vec::new();
            readback
                .try_reserve_exact(canonical_bytes.len().saturating_add(1))
                .map_err(|error| {
                    format!("failed to reserve {} staging readback: {error}", self.label)
                })?;
            std::io::Read::by_ref(&mut staging)
                .take(
                    u64::try_from(canonical_bytes.len())
                        .map_err(|_| format!("{} length does not fit u64", self.label))?
                        .saturating_add(1),
                )
                .read_to_end(&mut readback)
                .map_err(|error| {
                    format!("failed to read back {} staging file: {error}", self.label)
                })?;
            if readback != canonical_bytes {
                return Err(format!(
                    "{} staging file did not round-trip canonical bytes",
                    self.label
                ));
            }
            self.verify_parent_identity()?;
            self.parent.sync_all().map_err(|error| {
                format!(
                    "failed to sync {} destination before publication: {error}",
                    self.label
                )
            })?;
            match rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Err(error) if error == rustix::io::Errno::NOENT => {}
                Ok(_) => {
                    return Err(format!(
                        "{} destination appeared during preparation and will not be replaced: {}",
                        self.label,
                        self.path.display()
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "failed to recheck {} destination: {error}",
                        self.label
                    ));
                }
            }
            Ok((metadata.dev(), metadata.ino()))
        })();
        let (staged_device, staged_inode) = match staged_result {
            Ok(identity) => identity,
            Err(error) => {
                drop(staging);
                let cleanup = self.cleanup_owned_staging(&staging_name, staging_identity);
                return Err(RootOwnedArtifactPublicationError::pre_commit(
                    append_cleanup_result(error, cleanup),
                ));
            }
        };
        if let Err(error) = rustix::fs::renameat_with(
            &self.parent,
            &staging_name,
            &self.parent,
            &self.file_name,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            drop(staging);
            let cleanup = self.cleanup_owned_staging(&staging_name, staging_identity);
            return Err(RootOwnedArtifactPublicationError::pre_commit(
                append_cleanup_result(
                    format!(
                        "failed to publish {} without replacement: {error}",
                        self.label
                    ),
                    cleanup,
                ),
            ));
        }

        // This rename is the irreversible commit boundary. Never unlink the
        // final name or automatically retry after this point.
        let final_result = (|| -> Result<(), String> {
            let published = rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| format!("failed to inspect published {}: {error}", self.label))?;
            if u64::try_from(published.st_dev).ok() != Some(staged_device)
                || published.st_ino != staged_inode
                || u64::from(published.st_nlink) != 1
                || published.st_uid != self.expected_uid
                || u32::from(published.st_mode) & 0o7777 != 0o444
                || u64::try_from(published.st_size).ok()
                    != Some(
                        u64::try_from(canonical_bytes.len())
                            .map_err(|_| format!("{} length does not fit u64", self.label))?,
                    )
            {
                return Err(format!(
                    "published {} does not match the staged immutable inode",
                    self.label
                ));
            }
            #[cfg(target_os = "macos")]
            require_acl_free_pinned_path(
                &staging,
                &self.path,
                &format!("published {}", self.label),
            )?;
            require_no_xattrs(&staging, &self.path, &format!("published {}", self.label))?;
            self.parent.sync_all().map_err(|error| {
                format!(
                    "failed to sync {} destination after publication: {error}",
                    self.label
                )
            })?;
            staging.seek(SeekFrom::Start(0)).map_err(|error| {
                format!(
                    "failed to rewind published {} for readback: {error}",
                    self.label
                )
            })?;
            let mut readback = Vec::new();
            readback
                .try_reserve_exact(canonical_bytes.len().saturating_add(1))
                .map_err(|error| {
                    format!(
                        "failed to reserve published {} readback: {error}",
                        self.label
                    )
                })?;
            std::io::Read::by_ref(&mut staging)
                .take(
                    u64::try_from(canonical_bytes.len())
                        .map_err(|_| format!("{} length does not fit u64", self.label))?
                        .saturating_add(1),
                )
                .read_to_end(&mut readback)
                .map_err(|error| {
                    format!("failed to read back published {}: {error}", self.label)
                })?;
            if readback != canonical_bytes {
                return Err(format!(
                    "published {} did not round-trip canonical bytes",
                    self.label
                ));
            }
            verify_final(&self.path)?;
            require_no_xattrs(&staging, &self.path, &format!("published {}", self.label))?;
            #[cfg(target_os = "macos")]
            require_acl_free_pinned_path(
                &staging,
                &self.path,
                &format!("published {}", self.label),
            )?;
            let opened_after = staging.metadata().map_err(|error| {
                format!("failed to re-inspect published {}: {error}", self.label)
            })?;
            let named_after = rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| {
                format!(
                    "failed to re-inspect named published {}: {error}",
                    self.label
                )
            })?;
            let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
                format!(
                    "failed to re-inspect published {} path: {error}",
                    self.label
                )
            })?;
            if !stat_matches_metadata(&named_after, &opened_after)
                || !same_metadata(&opened_after, &path_after)
                || opened_after.dev() != staged_device
                || opened_after.ino() != staged_inode
                || opened_after.nlink() != 1
                || opened_after.uid() != self.expected_uid
                || opened_after.mode() & 0o7777 != 0o444
                || opened_after.len()
                    != u64::try_from(canonical_bytes.len())
                        .map_err(|_| format!("{} length does not fit u64", self.label))?
            {
                return Err(format!(
                    "published {} changed during final verification",
                    self.label
                ));
            }
            self.verify_parent_identity()
        })();
        final_result.map_err(
            |detail| RootOwnedArtifactPublicationError::CommitUncertain {
                label: self.label,
                path: self.path.clone(),
                detail,
            },
        )
    }

    #[cfg(not(unix))]
    /// Reject publication on platforms without the Unix custody primitive.
    pub(super) fn publish_bytes_and_verify(
        self,
        _canonical_bytes: &[u8],
        _verify_final: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), RootOwnedArtifactPublicationError> {
        Err(RootOwnedArtifactPublicationError::pre_commit(format!(
            "root-owned atomic {} publication is unsupported on this platform",
            self.label
        )))
    }

    #[cfg(unix)]
    fn cleanup_owned_staging(
        &self,
        staging_name: &OsString,
        expected: (u64, u64),
    ) -> Result<(), String> {
        let current = rustix::fs::statat(
            &self.parent,
            staging_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        );
        match current {
            Ok(current)
                if u64::try_from(current.st_dev).ok() == Some(expected.0)
                    && current.st_ino == expected.1 =>
            {
                rustix::fs::unlinkat(&self.parent, staging_name, rustix::fs::AtFlags::empty())
                    .map_err(|error| format!("failed to remove owned staging inode: {error}"))?;
                self.parent
                    .sync_all()
                    .map_err(|error| format!("failed to sync staging cleanup: {error}"))
            }
            Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
            Ok(_) => Err(
                "refused to remove a staging name that no longer identifies the owned inode"
                    .to_owned(),
            ),
            Err(error) => Err(format!("failed to inspect owned staging inode: {error}")),
        }
    }

    #[cfg(unix)]
    fn verify_parent_identity(&self) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt as _;
        let parent_path = self
            .path
            .parent()
            .expect("prepared artifact path has a parent");
        let opened_before = self.parent.metadata().map_err(|error| {
            format!(
                "failed to inspect pinned {} destination: {error}",
                self.label
            )
        })?;
        let path_before = fs::symlink_metadata(parent_path).map_err(|error| {
            format!(
                "failed to re-inspect {} destination path: {error}",
                self.label
            )
        })?;
        if path_before.file_type().is_symlink()
            || !path_before.is_dir()
            || !same_custody_metadata(&opened_before, &path_before)
            || opened_before.uid() != self.expected_uid
            || opened_before.mode() & 0o022 != 0
        {
            return Err(format!(
                "{} destination directory changed identity or trust attributes",
                self.label
            ));
        }
        require_no_xattrs(
            &self.parent,
            parent_path,
            &format!("{} destination directory", self.label),
        )?;
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(
            &self.parent,
            parent_path,
            &format!("{} destination directory", self.label),
        )?;
        let opened_after = self.parent.metadata().map_err(|error| {
            format!(
                "failed to re-inspect pinned {} destination: {error}",
                self.label
            )
        })?;
        let path_after = fs::symlink_metadata(parent_path).map_err(|error| {
            format!(
                "failed to re-inspect {} destination after custody validation: {error}",
                self.label
            )
        })?;
        if !same_custody_metadata(&opened_before, &opened_after)
            || !same_custody_metadata(&opened_after, &path_after)
        {
            return Err(format!(
                "{} destination directory changed during custody validation",
                self.label
            ));
        }
        Ok(())
    }
}

fn validate_canonical_absolute_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} path must be absolute: {}", path.display()));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(format!(
            "{label} path must not contain `.` or `..` components: {}",
            path.display()
        ));
    }
    Ok(())
}

fn append_cleanup_result(error: String, cleanup: Result<(), String>) -> String {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => format!("{error}; staging cleanup was incomplete: {cleanup}"),
    }
}
