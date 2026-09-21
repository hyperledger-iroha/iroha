//! Native private-key file admission: one bounded, private, immutable descriptor observation.

use eyre::{Result, bail, eyre};
use iroha_crypto::{ExposedPrivateKey, PrivateKey};
use std::path::Path;

const MAX_BYTES: usize = 4096;

#[cfg(unix)]
pub(super) fn read(path: &Path) -> Result<PrivateKey> {
    use rustix::fs::{Mode, OFlags};
    use std::fs::File;
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent_path = path
        .parent()
        .ok_or_else(|| eyre!("private key input must name a regular file"))?
        .canonicalize()?;
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("private key input must name a regular file"))?;
    let parent = File::from(rustix::fs::open(
        &parent_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    let file = File::from(rustix::fs::openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    read_opened(file, &parent, &parent_path, name)
}

#[cfg(not(unix))]
pub(super) fn read(_: &Path) -> Result<PrivateKey> {
    bail!("private-key files require native no-follow descriptor support on this platform")
}

#[cfg(unix)]
fn validate_file(metadata: &std::fs::Metadata) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.nlink() != 1
        || !matches!(metadata.mode() & 0o7777, 0o400 | 0o600)
    {
        bail!(
            "private-key file must be a current-owner single-link regular file with mode 0400 or 0600"
        );
    }
    if metadata.len() > MAX_BYTES as u64 {
        bail!("private-key file exceeds its 4096-byte bound");
    }
    Ok(())
}

#[cfg(unix)]
fn read_opened(
    mut file: std::fs::File,
    parent: &std::fs::File,
    parent_path: &Path,
    name: &std::ffi::OsStr,
) -> Result<PrivateKey> {
    use std::{io::Read, os::unix::fs::MetadataExt};
    let before = file.metadata()?;
    validate_file(&before)?;
    let mut encoded = zeroize::Zeroizing::new(Vec::new());
    std::io::Read::by_ref(&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut encoded)?;
    let after = file.metadata()?;
    let current = std::fs::symlink_metadata(parent_path.join(name))?;
    let current_parent = std::fs::symlink_metadata(parent_path)?;
    let held_parent = parent.metadata()?;
    validate_file(&after)?;
    validate_file(&current)?;
    if encoded.len() > MAX_BYTES
        || before.dev() != current.dev()
        || before.ino() != current.ino()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || !current_parent.is_dir()
        || current_parent.dev() != held_parent.dev()
        || current_parent.ino() != held_parent.ino()
    {
        bail!("private-key file or its parent changed during the retained read");
    }
    parse(&encoded)
}

fn parse(encoded: &[u8]) -> Result<PrivateKey> {
    let text =
        std::str::from_utf8(encoded).map_err(|_| eyre!("private-key file must contain UTF-8"))?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() || text.contains(['\r', '\n']) {
        bail!("private-key file must contain one canonical private key and optional final LF");
    }
    let key = text
        .parse::<PrivateKey>()
        .map_err(|_| eyre!("private-key file does not contain a canonical private key"))?;
    if ExposedPrivateKey(key.clone()).to_string() != text {
        bail!("private-key file does not contain a canonical private key");
    }
    Ok(key)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, PrivateKey) {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("private.key");
        let key =
            iroha_crypto::KeyPair::try_from_seed(vec![42; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap()
                .private_key()
                .clone();
        fs::write(&path, format!("{}\n", ExposedPrivateKey(key.clone()))).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        (temporary, path, key)
    }

    #[test]
    fn native_key_reader_accepts_only_private_canonical_single_link_files() {
        let (temporary, path, key) = fixture();
        assert_eq!(read(&path).unwrap(), key);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert_eq!(read(&path).unwrap(), key);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let link = temporary.path().join("linked");
        symlink(&path, &link).unwrap();
        assert!(read(&link).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&path, &link).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(&link).unwrap();
        for mode in [0o644, 0o660, 0o700, 0o1600] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            assert!(read(&path).is_err());
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(&path).unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(read(&path).is_err());
    }

    #[test]
    fn native_key_reader_rejects_path_replacement_while_holding_original_file() {
        let (temporary, path, _) = fixture();
        let file = std::fs::File::open(&path).unwrap();
        let parent_path = temporary.path().canonicalize().unwrap();
        let parent = std::fs::File::open(&parent_path).unwrap();
        fs::rename(&path, temporary.path().join("original.key")).unwrap();
        fs::write(&path, "not-the-opened-file").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_opened(file, &parent, &parent_path, path.file_name().unwrap()).is_err());
    }

    #[test]
    fn native_key_parse_errors_never_echo_key_material() {
        for input in [
            b"private-marker-must-not-leak".as_slice(),
            b"one\ntwo",
            b"key\r\n",
            &[0xff],
            b"",
        ] {
            let error = parse(input).unwrap_err();
            assert!(!format!("{error:#}").contains("private-marker-must-not-leak"));
        }
    }
}
