//! Secure runtime loading for the CLI operator-signing key.
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
use std::path::Path;
use zeroize::Zeroizing;
const MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES: u64 = 4 * 1024;
/// Load one canonical operator private key from an owner-only runtime file.
///
/// The operator credential is intentionally unavailable through environment variables, client
/// TOML, or the account signer. On Unix, the final path component is opened with `O_NOFOLLOW` and
/// the opened descriptor must remain a stable, singly linked, owner-only regular file throughout
/// the bounded read.
pub(crate) fn load_operator_key_pair(path: &Path) -> Result<KeyPair> {
    if !path.is_absolute() {
        bail!("operator private-key file path must be absolute");
    }
    #[cfg(unix)]
    {
        load_operator_key_pair_unix(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        bail!(
            "operator private-key loading is unavailable on this platform because secure O_NOFOLLOW file opens are unsupported"
        )
    }
}

/// Load an operator key from one inherited read-only descriptor without reopening a path.
///
/// The descriptor remains owned by the caller. The loader retains a private CLOEXEC duplicate,
/// uses only positional reads, and validates exact mode 0600, ownership, link count, length and
/// metadata stability before parsing the same canonical key format as the file loader. The
/// original descriptor is marked CLOEXEC so subsequent child processes cannot inherit the key.
pub(crate) fn load_operator_key_pair_fd(fd: u32) -> Result<KeyPair> {
    if !(3..=65535).contains(&fd) {
        bail!("operator private-key fd must be an inherited descriptor in 3..=65535");
    }
    #[cfg(unix)]
    {
        let file = duplicate_operator_key_descriptor(fd)?;
        let access = rustix::fs::fcntl_getfl(&file)
            .map_err(|_| eyre!("failed to inspect operator private-key fd access mode"))?;
        if access & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY {
            bail!("operator private-key fd must be read-only");
        }
        let bytes = read_operator_key_descriptor(&file)?;
        parse_operator_private_key(&bytes)
    }
    #[cfg(not(unix))]
    {
        bail!("operator private-key fd loading requires Unix descriptors")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    unsafe_code,
    reason = "fcntl atomically validates and duplicates an untrusted inherited integer descriptor before constructing an owned File"
)]
fn duplicate_operator_key_descriptor(fd: u32) -> Result<std::fs::File> {
    use std::os::fd::FromRawFd as _;
    #[cfg(target_os = "linux")]
    const DUPFD_CLOEXEC: std::ffi::c_int = 1030;
    #[cfg(target_os = "macos")]
    const DUPFD_CLOEXEC: std::ffi::c_int = 67;
    unsafe extern "C" {
        fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...) -> std::ffi::c_int;
    }
    let original = i32::try_from(fd).map_err(|_| eyre!("invalid operator private-key fd"))?;
    // SAFETY: F_DUPFD_CLOEXEC accepts an untrusted integer and atomically validates it. No
    // Rust descriptor borrow or ownership is constructed until the kernel returns a new fd.
    let copied = unsafe { fcntl(original, DUPFD_CLOEXEC, 3 as std::ffi::c_int) };
    if copied < 0 {
        bail!("failed to retain inherited operator private-key fd");
    }
    // SAFETY: the successful duplication created a unique owned CLOEXEC descriptor.
    let retained = unsafe { std::fs::File::from_raw_fd(copied) };
    // SAFETY: F_GETFD=1, F_SETFD=2 and FD_CLOEXEC=1 are native Linux/Darwin constants.
    // These integer-only calls let the kernel reject concurrent closure without a Rust borrow.
    // They change no file-status flags, file bytes, or shared cursor.
    let protected = unsafe {
        let flags = fcntl(original, 1);
        flags >= 0 && fcntl(original, 2, flags | 1) == 0
    };
    if !protected {
        bail!("failed to protect inherited operator private-key fd lifetime");
    }
    Ok(retained)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn duplicate_operator_key_descriptor(_: u32) -> Result<std::fs::File> {
    bail!("operator private-key fd loading requires a supported atomic descriptor ABI")
}

#[cfg(unix)]
fn read_operator_key_descriptor(file: &std::fs::File) -> Result<Zeroizing<Vec<u8>>> {
    use std::os::unix::fs::FileExt as _;
    read_operator_key_descriptor_with(file, |bytes, offset| file.read_at(bytes, offset))
}

#[cfg(unix)]
fn read_operator_key_descriptor_with(
    file: &std::fs::File,
    mut read_at: impl FnMut(&mut [u8], u64) -> std::io::Result<usize>,
) -> Result<Zeroizing<Vec<u8>>> {
    let before = file
        .metadata()
        .map_err(|_| eyre!("failed to inspect operator private-key fd"))?;
    validate_operator_key_metadata(&before)?;
    let capacity = usize::try_from(before.len())
        .map_err(|_| eyre!("operator private-key fd length exceeds host width"))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| eyre!("operator private-key fd allocation failed"))?;
    bytes.resize(capacity, 0);
    let mut offset = 0;
    while offset < capacity {
        match read_at(&mut bytes[offset..], offset as u64) {
            Ok(0) => bail!("operator private-key fd changed during bounded read"),
            Ok(count) if count <= capacity - offset => offset += count,
            Ok(_) => bail!("invalid operator private-key fd read length"),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => bail!("failed to read operator private-key fd"),
        }
    }
    // Probe the exact observed boundary before the final metadata snapshot. This catches growth
    // without truncating input, changing the shared cursor, or leaving a probe byte unzeroized.
    let mut boundary = Zeroizing::new([0_u8; 1]);
    loop {
        match read_at(&mut boundary[..], before.len()) {
            Ok(0) => break,
            Ok(_) => bail!("operator private-key fd changed during bounded read"),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => bail!("failed to verify operator private-key fd boundary"),
        }
    }
    let after = file
        .metadata()
        .map_err(|_| eyre!("failed to re-inspect operator private-key fd"))?;
    validate_operator_key_metadata(&after)?;
    if !operator_key_metadata_unchanged(&before, &after) {
        bail!("operator private-key fd changed during bounded read");
    }
    Ok(bytes)
}

#[cfg(unix)]
fn load_operator_key_pair_unix(path: &Path) -> Result<KeyPair> {
    use std::{
        fs,
        io::{Read as _, Take},
    };
    let path_metadata =
        fs::symlink_metadata(path).wrap_err("failed to inspect operator private-key file")?;
    validate_operator_key_metadata(&path_metadata)?;
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err("failed to securely open operator private-key file")?;
    let mut file = fs::File::from(descriptor);
    let before = file
        .metadata()
        .wrap_err("failed to inspect opened operator private-key file")?;
    validate_operator_key_metadata(&before)?;
    if !operator_key_metadata_unchanged(&path_metadata, &before) {
        bail!("operator private-key file changed during secure open");
    }
    let capacity = usize::try_from(before.len())
        .map_err(|_| eyre!("operator private-key file length exceeds host width"))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| eyre!("operator private-key file allocation failed"))?;
    let mut bounded: Take<&mut fs::File> =
        (&mut file).take(MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES.saturating_add(1));
    bounded
        .read_to_end(&mut bytes)
        .wrap_err("failed to read operator private-key file")?;
    let after = file
        .metadata()
        .wrap_err("failed to re-inspect operator private-key file")?;
    validate_operator_key_metadata(&after)?;
    if !operator_key_metadata_unchanged(&before, &after)
        || u64::try_from(bytes.len()).ok() != Some(before.len())
        || bytes.len() > usize::try_from(MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES).unwrap_or(usize::MAX)
    {
        bail!("operator private-key file changed during bounded read");
    }
    parse_operator_private_key(&bytes)
}

#[cfg(unix)]
fn validate_operator_key_metadata(metadata: &std::fs::Metadata) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES
    {
        bail!("operator private-key file must be a non-empty, bounded, singly linked regular file");
    }
    if metadata.permissions().mode() & 0o7777 != 0o600 {
        bail!("operator private-key file must have exact mode 0600");
    }
    if metadata.uid() != rustix::process::geteuid().as_raw() {
        bail!("operator private-key file must be owned by the current user");
    }
    Ok(())
}
#[cfg(unix)]
fn operator_key_metadata_unchanged(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
        && before.nlink() == 1
        && after.nlink() == 1
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

fn parse_operator_private_key(bytes: &[u8]) -> Result<KeyPair> {
    let encoded = std::str::from_utf8(bytes)
        .map_err(|_| eyre!("operator private-key file must contain one canonical ASCII key"))?;
    let encoded = encoded.strip_suffix('\n').unwrap_or(encoded);
    if encoded.is_empty()
        || !encoded.is_ascii()
        || encoded.bytes().any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        bail!("operator private-key file must contain one canonical ASCII key");
    }
    let private_key = encoded
        .parse::<PrivateKey>()
        .map_err(|_| eyre!("operator private-key file does not contain a canonical private key"))?;
    let canonical = Zeroizing::new(
        ExposedPrivateKey(private_key.clone())
            .try_to_multihash_string()
            .map_err(|_| eyre!("operator private-key canonical encoding failed"))?,
    );
    if canonical.as_str() != encoded {
        bail!("operator private-key file does not contain a canonical private key");
    }
    KeyPair::from_private_key(private_key)
        .map_err(|_| eyre!("operator private-key file contains an invalid signing key"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    use std::{fs, path::Path};
    #[cfg(unix)]
    fn write_private_key(path: &Path, contents: &[u8]) {
        use std::os::unix::fs::PermissionsExt as _;
        fs::write(path, contents).expect("write operator key fixture");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("set exact operator key permissions");
    }
    #[cfg(unix)]
    #[test]
    fn loads_one_absolute_owner_only_operator_key() {
        let directory = tempfile::tempdir().expect("operator key directory");
        let path = directory.path().join("operator.key");
        let expected = KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
            .expect("checked operator key fixture");
        let encoded = ExposedPrivateKey(expected.private_key().clone()).to_string();
        write_private_key(&path, format!("{encoded}\n").as_bytes());
        let actual = load_operator_key_pair(&path).expect("load secure operator key");
        assert_eq!(actual.public_key(), expected.public_key());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_indirect_or_non_owner_only_operator_key_files() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let directory = tempfile::tempdir().expect("operator key directory");
        let source = directory.path().join("source.key");
        let link = directory.path().join("link.key");
        let hard_link = directory.path().join("hard-link.key");
        let key = KeyPair::try_from_seed(vec![0xB7; 32], Algorithm::Ed25519)
            .expect("checked operator key fixture");
        let encoded = ExposedPrivateKey(key.private_key().clone()).to_string();
        write_private_key(&source, encoded.as_bytes());
        symlink(&source, &link).expect("create operator key symlink");
        assert!(load_operator_key_pair(&link).is_err());
        fs::hard_link(&source, &hard_link).expect("create operator key hard link");
        assert!(load_operator_key_pair(&source).is_err());
        fs::remove_file(&hard_link).expect("remove operator key hard link");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o640))
            .expect("loosen operator key permissions");
        assert!(load_operator_key_pair(&source).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_relative_oversized_and_secret_echoing_operator_key_inputs() {
        assert!(load_operator_key_pair(Path::new("operator.key")).is_err());
        let directory = tempfile::tempdir().expect("operator key directory");
        let oversized = directory.path().join("oversized.key");
        write_private_key(
            &oversized,
            &vec![b'A'; usize::try_from(MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES).unwrap() + 1],
        );
        assert!(load_operator_key_pair(&oversized).is_err());
        let invalid = directory.path().join("invalid.key");
        let sensitive = "SENSITIVE_OPERATOR_PRIVATE_KEY_MUST_NOT_APPEAR";
        write_private_key(&invalid, sensitive.as_bytes());
        let error = load_operator_key_pair(&invalid).expect_err("invalid operator key must fail");
        assert!(!format!("{error:#}").contains(sensitive));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn loads_borrowed_operator_fd_positionally_without_reopening_its_path() {
        use std::{io::Seek as _, os::fd::AsRawFd as _};
        let directory = tempfile::tempdir().expect("operator fd directory");
        let path = directory.path().join("operator.key");
        let moved = directory.path().join("retained.key");
        let expected = KeyPair::try_from_seed(vec![0xC7; 32], Algorithm::Ed25519)
            .expect("checked operator fd fixture");
        let encoded = format!("{}\n", ExposedPrivateKey(expected.private_key().clone()));
        write_private_key(&path, encoded.as_bytes());
        let mut file = fs::File::open(&path).expect("open inherited operator fd");
        file.seek(std::io::SeekFrom::Start(7))
            .expect("set caller cursor");
        fs::rename(&path, &moved).expect("rename the borrowed source");
        write_private_key(&path, b"replacement-must-not-be-opened");
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty())
            .expect("model explicitly inherited descriptor");
        let before = file.metadata().expect("original fd metadata");
        let flags = rustix::fs::fcntl_getfl(&file).expect("original access mode");
        let fd = u32::try_from(file.as_raw_fd()).expect("positive fixture fd");
        let actual = load_operator_key_pair_fd(fd).expect("load borrowed operator key");
        assert_eq!(actual.public_key(), expected.public_key());
        assert_eq!(file.stream_position().expect("caller fd still open"), 7);
        assert_eq!(rustix::fs::fcntl_getfl(&file).unwrap(), flags);
        assert!(operator_key_metadata_unchanged(
            &before,
            &file.metadata().unwrap()
        ));
        assert_eq!(fs::read(&moved).unwrap(), encoded.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), b"replacement-must-not-be-opened");
        assert!(
            rustix::io::fcntl_getfd(&file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn rejects_operator_fd_numbers_outside_the_inherited_range() {
        for fd in [0, 1, 2, 65536, u32::MAX] {
            let error = load_operator_key_pair_fd(fd).expect_err("invalid inherited fd");
            assert!(format!("{error:#}").contains("3..=65535"));
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rejects_non_readonly_nonregular_linked_and_non_owner_only_operator_fds() {
        use std::os::{fd::AsRawFd as _, unix::fs::PermissionsExt as _};
        let directory = tempfile::tempdir().expect("operator fd custody directory");
        let path = directory.path().join("operator.key");
        let link = directory.path().join("linked.key");
        write_private_key(&path, b"secret-fixture-not-a-key");
        for readable in [false, true] {
            let file = fs::OpenOptions::new()
                .read(readable)
                .write(true)
                .open(&path)
                .unwrap();
            let error = load_operator_key_pair_fd(u32::try_from(file.as_raw_fd()).unwrap())
                .expect_err("writable fd must fail");
            assert!(format!("{error:#}").contains("read-only"));
        }
        let file = fs::File::open(&path).unwrap();
        let fd = u32::try_from(file.as_raw_fd()).unwrap();
        fs::hard_link(&path, &link).unwrap();
        assert!(load_operator_key_pair_fd(fd).is_err());
        fs::remove_file(&link).unwrap();
        for mode in [0o400, 0o640, 0o1600] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            let error = load_operator_key_pair_fd(fd).expect_err("exact mode 0600 required");
            assert!(format!("{error:#}").contains("exact mode 0600"));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let directory_fd = fs::File::open(directory.path()).unwrap();
        assert!(
            load_operator_key_pair_fd(u32::try_from(directory_fd.as_raw_fd()).unwrap()).is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"secret-fixture-not-a-key");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rejects_empty_oversized_and_noncanonical_operator_fds_without_secret_echo() {
        use std::os::fd::AsRawFd as _;
        let directory = tempfile::tempdir().expect("operator fd parsing directory");
        let path = directory.path().join("operator.key");
        let key = KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::Ed25519).unwrap();
        let encoded = ExposedPrivateKey(key.private_key().clone()).to_string();
        let secret = "FD_PRIVATE_KEY_MUST_NOT_APPEAR_IN_ERROR";
        for body in [
            Vec::new(),
            vec![b'A'; MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES as usize + 1],
            secret.as_bytes().to_vec(),
            format!(" {encoded}").into_bytes(),
            format!("{encoded}\r\n").into_bytes(),
            format!("{encoded}\n\n").into_bytes(),
            vec![0xff],
        ] {
            write_private_key(&path, &body);
            let file = fs::File::open(&path).unwrap();
            let error = load_operator_key_pair_fd(u32::try_from(file.as_raw_fd()).unwrap())
                .expect_err("noncanonical operator fd must fail");
            let report = format!("{error:#}");
            assert!(!report.contains(secret));
            assert!(!report.contains(&encoded));
            assert_eq!(fs::read(&path).unwrap(), body);
        }
    }

    #[cfg(unix)]
    #[test]
    fn positional_operator_read_rejects_mutation_before_final_metadata_check() {
        use std::os::unix::fs::{FileExt as _, PermissionsExt as _};
        let directory = tempfile::tempdir().expect("operator fd mutation directory");
        let path = directory.path().join("operator.key");
        let hard_link = directory.path().join("operator-linked.key");
        for mutation in [
            "grow",
            "truncate",
            "permissions",
            "hard_link",
            "same_length_rewrite",
        ] {
            write_private_key(&path, b"original-private-bytes");
            let file = fs::File::open(&path).unwrap();
            let before = file.metadata().unwrap();
            let mut changed = false;
            let result = read_operator_key_descriptor_with(&file, |buffer, offset| {
                let count = file.read_at(buffer, offset)?;
                if !changed {
                    changed = true;
                    match mutation {
                        "grow" => fs::write(&path, b"original-private-bytes-plus")?,
                        "truncate" => fs::write(&path, b"short")?,
                        "permissions" => {
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?
                        }
                        "hard_link" => fs::hard_link(&path, &hard_link)?,
                        "same_length_rewrite" => {
                            fs::write(&path, b"replaced-private-bytes")?;
                            // Set a distinct mtime rather than relying on filesystem clock resolution.
                            fs::OpenOptions::new().write(true).open(&path)?.set_times(
                                fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH),
                            )?;
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(count)
            });
            assert!(result.is_err(), "must reject {mutation}");
            assert!(!operator_key_metadata_unchanged(
                &before,
                &file.metadata().unwrap()
            ));
            if hard_link.exists() {
                fs::remove_file(&hard_link).unwrap();
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn positional_operator_read_handles_short_reads_and_redacts_io_errors() {
        use std::os::unix::fs::FileExt as _;
        let directory = tempfile::tempdir().expect("operator fd read directory");
        let path = directory.path().join("operator.key");
        let original = b"synthetic-private-file-bytes";
        write_private_key(&path, original);
        let file = fs::File::open(&path).unwrap();
        let bytes = read_operator_key_descriptor_with(&file, |buffer, offset| {
            let length = buffer.len().min(2);
            file.read_at(&mut buffer[..length], offset)
        })
        .expect("complete bounded positional short reads");
        assert_eq!(&bytes[..], original);
        let secret = "READ_ERROR_PRIVATE_VALUE_MUST_BE_REDACTED";
        let error =
            read_operator_key_descriptor_with(&file, |_, _| Err(std::io::Error::other(secret)))
                .expect_err("read error must fail");
        assert!(!format!("{error:#}").contains(secret));
        let error = read_operator_key_descriptor_with(&file, |_, _| Ok(0))
            .expect_err("premature EOF must fail");
        assert!(format!("{error:#}").contains("changed during bounded read"));
    }
}
