//! Bounded final-component reads for hostile local filesystem inputs.
//!
//! On qualified Unix targets this module pins one regular final component
//! through a no-follow, nonblocking descriptor and revalidates its path-visible
//! identity after the bounded read. Other targets fail closed before reading.
//! Callers remain responsible for ancestor confinement: these pathname-based
//! opens do not claim to close a deliberately timed ancestor-directory ABA.
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
#[cfg(unix)]
use std::{
    fs::{self, OpenOptions},
    io::Read as _,
};
use std::{io, path::Path};
/// Read one exact, bounded, singly linked regular final component.
///
/// The returned bytes come from one descriptor whose type, size, link count,
/// and stable platform identity match the named path before and after the
/// read. On qualified Unix targets, nonblocking open keeps a raced FIFO from
/// hanging before its descriptor type is rejected. Other targets fail closed
/// until a stable handle-identity implementation is available.
pub(crate) fn read_bounded_single_link_regular_file_v1(
    path: &Path,
    max_bytes: u64,
) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        read_bounded_single_link_regular_file_impl_v1(path, max_bytes, |_| Ok(()))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, max_bytes);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bounded identity-stable local-file reads are unsupported on this platform",
        ))
    }
}
#[cfg(unix)]
fn read_bounded_single_link_regular_file_impl_v1<F>(
    path: &Path,
    max_bytes: u64,
    before_open: F,
) -> io::Result<Vec<u8>>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    let named_before = fs::symlink_metadata(path)?;
    if !metadata_is_single_link_regular(&named_before) || named_before.len() > max_bytes {
        return Err(invalid_local_file());
    }
    before_open(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_nonblocking(&mut options);
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if !metadata_is_single_link_regular(&opened_before)
        || opened_before.len() > max_bytes
        || !same_file_snapshot(&named_before, &opened_before)
    {
        return Err(changed_local_file());
    }
    let read_limit = max_bytes.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "bounded local-file read limit must be below u64::MAX",
        )
    })?;
    let initial_capacity = usize::try_from(opened_before.len())
        .unwrap_or(0)
        .min(64 * 1024);
    let mut bytes = Vec::with_capacity(initial_capacity);
    file.by_ref().take(read_limit).read_to_end(&mut bytes)?;
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    let bytes_len = u64::try_from(bytes.len()).map_err(|_| changed_local_file())?;
    if bytes_len > max_bytes
        || bytes_len != opened_before.len()
        || !metadata_is_single_link_regular(&opened_after)
        || !metadata_is_single_link_regular(&named_after)
        || !same_file_snapshot(&opened_before, &opened_after)
        || !same_file_snapshot(&opened_after, &named_after)
    {
        return Err(changed_local_file());
    }
    Ok(bytes)
}
/// Exercise the bounded descriptor boundary with a deterministic pre-open hook.
///
/// The hook runs after the named-file snapshot and immediately before the
/// no-follow descriptor open, allowing focused tests to coordinate a
/// final-component replacement without timing-dependent threads.
#[cfg(all(test, unix))]
pub(crate) fn read_bounded_single_link_regular_file_with_hook_v1<F>(
    path: &Path,
    max_bytes: u64,
    before_open: F,
) -> io::Result<Vec<u8>>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    read_bounded_single_link_regular_file_impl_v1(path, max_bytes, before_open)
}
#[cfg(unix)]
fn invalid_local_file() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "local input must be one bounded, singly linked regular file",
    )
}
#[cfg(unix)]
fn changed_local_file() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "local input changed while its bounded bytes were read",
    )
}
#[cfg(unix)]
fn metadata_is_single_link_regular(metadata: &fs::Metadata) -> bool {
    !metadata_is_link(metadata) && metadata.is_file() && metadata.nlink() == 1
}
#[cfg(unix)]
fn metadata_is_link(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.nlink() == 1
        && right.nlink() == 1
}
#[cfg(unix)]
fn set_no_follow_nonblocking(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag() | platform_nonblocking_flag());
}
#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("Musubi bounded local-file reads are not qualified for this Android architecture");
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Musubi bounded local-file reads are not qualified for this Unix target");
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
const fn platform_no_follow_flag() -> i32 {
    0x400000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x80
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "sparc", target_arch = "sparc64")
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4000
}
#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x800
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::{fs::File, process::Command};
    use tempfile::tempdir;
    #[cfg(unix)]
    #[test]
    fn reads_exact_regular_bytes_and_rejects_oversize_files() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        fs::write(&path, b"exact").expect("write exact input");
        assert_eq!(
            read_bounded_single_link_regular_file_v1(&path, 5).expect("bounded read"),
            b"exact"
        );
        File::create(&path)
            .expect("replace input")
            .set_len(6)
            .expect("extend sparse input");
        assert_eq!(
            read_bounded_single_link_regular_file_v1(&path, 5)
                .expect_err("oversized input must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[cfg(unix)]
    #[test]
    fn rejects_hardlinked_inputs() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        let alias = temporary.path().join("alias");
        fs::write(&path, b"exact").expect("write input");
        fs::hard_link(&path, &alias).expect("create hard link");
        assert_eq!(
            read_bounded_single_link_regular_file_v1(&path, 5)
                .expect_err("hardlinked input must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[cfg(unix)]
    #[test]
    fn raced_regular_replacement_is_rejected() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        let replacement = temporary.path().join("replacement");
        fs::write(&path, b"first").expect("write initial input");
        fs::write(&replacement, b"other").expect("write replacement input");
        let error = read_bounded_single_link_regular_file_with_hook_v1(&path, 5, |path| {
            fs::remove_file(path)?;
            fs::rename(&replacement, path)
        })
        .expect_err("raced regular replacement must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_and_device_inputs_before_reading() {
        use std::os::unix::fs::symlink;
        let temporary = tempdir().expect("temporary directory");
        let target = temporary.path().join("target");
        let symbolic = temporary.path().join("symbolic");
        fs::write(&target, b"exact").expect("write target");
        symlink(&target, &symbolic).expect("create symlink");
        assert!(read_bounded_single_link_regular_file_v1(&symbolic, 5).is_err());
        assert!(read_bounded_single_link_regular_file_v1(Path::new("/dev/null"), 5).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn raced_symlink_is_rejected_by_the_no_follow_open() {
        use std::os::unix::fs::symlink;
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        let target = temporary.path().join("target");
        fs::write(&path, b"first").expect("write initial input");
        fs::write(&target, b"other").expect("write symlink target");
        let error = read_bounded_single_link_regular_file_with_hook_v1(&path, 5, |path| {
            fs::remove_file(path)?;
            symlink(&target, path)
        })
        .expect_err("raced symlink must not be followed");
        assert_eq!(error.kind(), io::ErrorKind::FilesystemLoop);
        assert_eq!(fs::read(&target).expect("read target"), b"other");
    }
    #[cfg(not(unix))]
    #[test]
    fn unsupported_platforms_fail_closed_before_reading() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        fs::write(&path, b"exact").expect("write exact input");
        assert_eq!(
            read_bounded_single_link_regular_file_v1(&path, 5)
                .expect_err("unqualified platform must fail closed")
                .kind(),
            io::ErrorKind::Unsupported
        );
    }
    #[cfg(unix)]
    #[test]
    fn raced_fifo_is_rejected_without_a_blocking_open() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        fs::write(&path, b"exact").expect("write initial input");
        let error = read_bounded_single_link_regular_file_with_hook_v1(&path, 5, |path| {
            fs::remove_file(path)?;
            let status = Command::new("mkfifo").arg(path).status()?;
            if !status.success() {
                return Err(io::Error::other("mkfifo failed"));
            }
            Ok(())
        })
        .expect_err("raced FIFO must fail without hanging");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
