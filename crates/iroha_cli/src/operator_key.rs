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
#[cfg(unix)]
fn load_operator_key_pair_unix(path: &Path) -> Result<KeyPair> {
    use std::{
        fs,
        io::{Read as _, Take},
        os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    };
    fn validate_metadata(metadata: &fs::Metadata) -> Result<()> {
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.len() == 0
            || metadata.len() > MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES
        {
            bail!(
                "operator private-key file must be a non-empty, bounded, singly linked regular file"
            );
        }
        if metadata.permissions().mode() & 0o7777 != 0o600 {
            bail!("operator private-key file must have exact mode 0600");
        }
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            bail!("operator private-key file must be owned by the current user");
        }
        Ok(())
    }
    fn unchanged(before: &fs::Metadata, after: &fs::Metadata) -> bool {
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.nlink() == 1
            && after.nlink() == 1
            && before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    let path_metadata =
        fs::symlink_metadata(path).wrap_err("failed to inspect operator private-key file")?;
    validate_metadata(&path_metadata)?;
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
    validate_metadata(&before)?;
    if !unchanged(&path_metadata, &before) {
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
    validate_metadata(&after)?;
    if !unchanged(&before, &after)
        || u64::try_from(bytes.len()).ok() != Some(before.len())
        || bytes.len() > usize::try_from(MAX_OPERATOR_PRIVATE_KEY_FILE_BYTES).unwrap_or(usize::MAX)
    {
        bail!("operator private-key file changed during bounded read");
    }
    let encoded = std::str::from_utf8(&bytes)
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
    let canonical = Zeroizing::new(ExposedPrivateKey(private_key.clone()).to_string());
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
}
