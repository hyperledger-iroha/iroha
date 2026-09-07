//! Platform flags for nonblocking filesystem opens that reject final-component symlinks.
//!
//! This module is available on Linux; Android `aarch64`, `arm`, `riscv64`, `x86` and `x86_64`; macOS;
//! iOS; FreeBSD; OpenBSD; NetBSD; and `DragonFly`. Other targets expose no filesystem flag module.
//! Callers must fail closed when these operations are unavailable.
//!
//! `O_NOFOLLOW` protects only the final path component. Callers remain responsible for ancestor
//! confinement and validating the opened descriptor's type, ownership, link count, and identity.
//! `O_NONBLOCK` prevents a substituted FIFO from hanging before that descriptor validation.

/// Return flags for a nonblocking open that rejects final-component symlinks.
///
/// Use with [`std::os::unix::fs::OpenOptionsExt::custom_flags`]. These flags do not require a
/// regular file: validate the opened descriptor before reading, writing, or trusting its data.
#[cfg(unix)]
pub const fn secure_no_follow_nonblocking_flags() -> i32 {
    libc::O_NOFOLLOW | libc::O_NONBLOCK
}

/// Return flags for a nonblocking, directory-only open that rejects final-component symlinks.
///
/// Use with [`std::os::unix::fs::OpenOptionsExt::custom_flags`]. Ancestor confinement and
/// descriptor ownership/identity validation remain the caller's responsibility.
#[cfg(unix)]
pub const fn secure_directory_open_flags() -> i32 {
    secure_no_follow_nonblocking_flags() | libc::O_DIRECTORY
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        io::{self, Read as _},
        os::unix::fs::{FileTypeExt as _, OpenOptionsExt as _, symlink},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use super::{secure_directory_open_flags, secure_no_follow_nonblocking_flags};

    #[test]
    fn regular_file_opens_but_final_component_symlinks_do_not() {
        let root = tempfile::tempdir().expect("temporary directory");
        let target = root.path().join("target");
        let link = root.path().join("link");
        fs::write(&target, b"owned bytes").expect("write target");
        symlink(&target, &link).expect("link target");
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(secure_no_follow_nonblocking_flags());
        let mut opened = options.open(&target).expect("open regular file");
        let mut bytes = Vec::new();
        opened.read_to_end(&mut bytes).expect("read regular file");
        assert_eq!(bytes, b"owned bytes");
        assert!(
            options.open(&link).is_err(),
            "must not follow a final symlink"
        );
        fs::remove_file(&target).expect("remove symlink target");
        assert!(
            options.open(&link).is_err(),
            "must reject dangling symlinks"
        );
    }

    #[test]
    fn opened_descriptor_keeps_its_identity_after_path_replacement() {
        let root = tempfile::tempdir().expect("temporary directory");
        let target = root.path().join("target");
        let replacement = root.path().join("replacement");
        fs::write(&target, b"original").expect("write original");
        let mut opened = OpenOptions::new()
            .read(true)
            .custom_flags(secure_no_follow_nonblocking_flags())
            .open(&target)
            .expect("open original descriptor");
        fs::write(&replacement, b"replacement").expect("write replacement");
        fs::rename(&replacement, &target).expect("replace named path");
        let mut bytes = Vec::new();
        opened
            .read_to_end(&mut bytes)
            .expect("read original descriptor");
        assert_eq!(bytes, b"original");
        assert_eq!(fs::read(&target).expect("read named path"), b"replacement");
    }

    #[test]
    fn directory_only_flags_reject_regular_files_and_linked_directories() {
        let root = tempfile::tempdir().expect("temporary directory");
        let file = root.path().join("file");
        let directory = root.path().join("directory");
        let link = root.path().join("link");
        fs::write(&file, b"not a directory").expect("write regular file");
        fs::create_dir(&directory).expect("create directory");
        symlink(&directory, &link).expect("link directory");
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(secure_directory_open_flags());
        assert!(
            options
                .open(&directory)
                .expect("open directory")
                .metadata()
                .expect("directory metadata")
                .is_dir()
        );
        assert!(options.open(&file).is_err(), "must require a directory");
        assert!(
            options.open(&link).is_err(),
            "must not follow a directory symlink"
        );
    }

    #[test]
    fn fifo_open_and_empty_read_are_nonblocking() {
        let flags = secure_no_follow_nonblocking_flags();
        // Reject a missing flag before the FIFO operation, so a regression cannot hang the suite.
        assert_eq!(flags & libc::O_NONBLOCK, libc::O_NONBLOCK);
        let directory_flags = secure_directory_open_flags();
        assert_eq!(directory_flags & libc::O_NONBLOCK, libc::O_NONBLOCK);
        assert_eq!(directory_flags & libc::O_DIRECTORY, libc::O_DIRECTORY);
        let root = tempfile::tempdir().expect("temporary directory");
        let fifo = root.path().join("fifo");
        let mut child = Command::new("mkfifo")
            .arg(&fifo)
            .stdin(Stdio::null())
            .spawn()
            .expect("launch mkfifo");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll mkfifo") {
                assert!(status.success(), "mkfifo must succeed");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().expect("stop timed-out mkfifo child");
                child.wait().expect("reap timed-out mkfifo child");
                panic!("mkfifo exceeded its five-second deadline");
            }
            thread::sleep(Duration::from_millis(10));
        }
        // No writer exists. A blocking read-only open would wait for one indefinitely.
        let mut reader = OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&fifo)
            .expect("open FIFO without a writer");
        assert!(
            reader
                .metadata()
                .expect("FIFO metadata")
                .file_type()
                .is_fifo()
        );
        let writer = OpenOptions::new()
            .write(true)
            .custom_flags(flags)
            .open(&fifo)
            .expect("open FIFO writer");
        assert_eq!(
            reader
                .read(&mut [0_u8; 1])
                .expect_err("empty FIFO must not block")
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(
            OpenOptions::new()
                .read(true)
                .custom_flags(directory_flags)
                .open(&fifo)
                .is_err()
        );
        drop(writer);
        assert_eq!(reader.read(&mut [0_u8; 1]).expect("FIFO EOF"), 0);
    }
}
