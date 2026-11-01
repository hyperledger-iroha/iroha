//! Read canonical CLI signing keys through retained Windows private-file custody.
//!
//! The original local-disk file and its ancestry survive the bounded read and canonical parse.
//! Admission never creates inputs, repairs ACLs or substitutes another signing credential.

use std::{
    io::{self, Read as _},
    path::Path,
};

use super::{
    KeyPair, MAX_PRIVATE_KEY_FILE_BYTES, Result, Zeroizing, bail, eyre, parse_private_key,
};

pub(super) fn load(path: &Path) -> Result<KeyPair> {
    load_observed(path, || Ok(()))
}

// Private dependency injection exercises the real final custody boundary. Production supplies
// no operation; this is neither a configuration switch nor a replacement filesystem reader.
fn load_observed(path: &Path, after_read: impl FnOnce() -> io::Result<()>) -> Result<KeyPair> {
    // The caller checks absolute spelling. The native owner admits the complete local path
    // before opening it; do not probe UNC/device names with pathname metadata first.
    let mut original = iroha_fs::RetainedFile::open_private(path).map_err(|_| {
        eyre!("private-key file requires a regular file with private current-owner native custody")
    })?;
    let before = original
        .snapshot()
        .map_err(|_| eyre!("failed to inspect private-key file custody"))?;
    let length = original
        .file()
        .metadata()
        .map_err(|_| eyre!("failed to inspect private-key file length"))?
        .len();
    if length == 0 || length > MAX_PRIVATE_KEY_FILE_BYTES {
        bail!("private-key file must be a non-empty, bounded, singly linked regular file");
    }
    let capacity =
        usize::try_from(length).map_err(|_| eyre!("private-key file length exceeds host width"))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| eyre!("private-key file allocation failed"))?;
    bytes.resize(capacity, 0);
    original
        .file_mut()
        .read_exact(&mut bytes)
        .map_err(|_| eyre!("failed to read complete private-key file"))?;
    let mut boundary = Zeroizing::new([0_u8; 1]);
    loop {
        match original.file_mut().read(&mut boundary[..]) {
            Ok(0) => break,
            Ok(_) => bail!("private-key file changed during bounded read"),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => bail!("failed to verify private-key file boundary"),
        }
    }
    after_read().map_err(|_| eyre!("private-key file read observation failed"))?;
    let after = original
        .snapshot()
        .map_err(|_| eyre!("private-key file custody changed during bounded read"))?;
    if before != after {
        bail!("private-key file changed during bounded read");
    }
    original
        .revalidate()
        .map_err(|_| eyre!("private-key file custody changed after reading"))?;
    let key = parse_private_key(&bytes)?;
    original
        .revalidate()
        .map_err(|_| eyre!("private-key file custody changed during canonical parsing"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write as _, path::PathBuf, process::Command};

    use iroha_crypto::{Algorithm, ExposedPrivateKey};

    use super::*;
    use crate::operator_key::{load_operator_key_pair, load_owner_key_pair};

    fn root() -> (tempfile::TempDir, PathBuf) {
        let temporary = tempfile::tempdir().expect("operator key fixture root");
        let path = temporary.path().canonicalize().expect("canonical root");
        (temporary, path)
    }

    fn key(algorithm: Algorithm) -> (KeyPair, String) {
        let key = KeyPair::try_from_seed(vec![0xE5; 32], algorithm).unwrap();
        let encoded = ExposedPrivateKey(key.private_key().clone()).to_string();
        (key, encoded)
    }

    fn write_fixture(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = directory.join(name);
        let mut writer = iroha_fs::RetainedFile::create_new_private(&path).unwrap();
        writer.file_mut().write_all(bytes).unwrap();
        writer.file().sync_all().unwrap();
        writer.revalidate().unwrap();
        drop(writer);
        path
    }

    fn acl(path: &Path) -> Vec<u8> {
        let output = Command::new("icacls.exe").arg(path).output().unwrap();
        assert!(output.status.success(), "fixture ACL query must succeed");
        output.stdout
    }

    fn grant(path: &Path, permission: &str) {
        let output = Command::new("icacls.exe")
            .arg(path)
            .args(["/grant", permission])
            .output()
            .unwrap();
        assert!(output.status.success(), "fixture ACL mutation must succeed");
    }

    #[test]
    fn native_operator_key_reads_canonical_private_original_without_mutation() {
        let (_temporary, directory) = root();
        let (expected, encoded) = key(Algorithm::Ed25519);
        for (index, bytes) in [
            encoded.as_bytes().to_vec(),
            format!("{encoded}\n").into_bytes(),
        ]
        .into_iter()
        .enumerate()
        {
            let path = write_fixture(&directory, &format!("private-{index}.key"), &bytes);
            let before = iroha_fs::RetainedFile::open_private(&path)
                .unwrap()
                .snapshot()
                .unwrap();
            let file_acl = acl(&path);
            let directory_acl = acl(&directory);
            assert_eq!(
                load_operator_key_pair(&path).unwrap().public_key(),
                expected.public_key()
            );
            assert_eq!(
                load_owner_key_pair(&path).unwrap().public_key(),
                expected.public_key()
            );
            assert!(
                fs::read(&path).unwrap() == bytes,
                "input bytes must not change"
            );
            assert_eq!(acl(&path), file_acl);
            assert_eq!(acl(&directory), directory_acl);
            assert_eq!(
                iroha_fs::RetainedFile::open_private(&path)
                    .unwrap()
                    .snapshot()
                    .unwrap(),
                before
            );
        }
    }

    #[test]
    fn native_operator_key_preserves_distinct_owner_algorithm_policy() {
        let (_temporary, directory) = root();
        let (expected, encoded) = key(Algorithm::Secp256k1);
        let path = write_fixture(&directory, "operator.key", encoded.as_bytes());
        assert_eq!(
            load_operator_key_pair(&path).unwrap().public_key(),
            expected.public_key()
        );
        let error = load_owner_key_pair(&path)
            .err()
            .expect("owner requires Ed25519");
        assert!(format!("{error:#}").contains("Ed25519"));
        assert!(load_owner_key_pair(&directory.join("missing.key")).is_err());
        assert!(
            fs::read(&path).unwrap() == encoded.as_bytes(),
            "no fallback or rewrite"
        );
    }

    #[test]
    fn native_operator_key_preserves_bounds_canonical_parser_and_redaction() {
        let (_temporary, directory) = root();
        let (_, encoded) = key(Algorithm::Ed25519);
        let sensitive = "OPERATOR_PRIVATE_VALUE_MUST_NOT_APPEAR";
        for (index, bytes) in [
            Vec::new(),
            vec![b'A'; MAX_PRIVATE_KEY_FILE_BYTES as usize + 1],
            vec![b'A'; MAX_PRIVATE_KEY_FILE_BYTES as usize],
            sensitive.as_bytes().to_vec(),
            format!(" {encoded}").into_bytes(),
            format!("{encoded}\r\n").into_bytes(),
            format!("{encoded}\n\n").into_bytes(),
            vec![0xff],
        ]
        .into_iter()
        .enumerate()
        {
            let path = write_fixture(&directory, &format!("invalid-{index}.key"), &bytes);
            let error = load_operator_key_pair(&path)
                .err()
                .expect("invalid key must refuse");
            let message = format!("{error:#}");
            assert!(!message.contains(sensitive));
            assert!(!message.contains(&encoded));
            assert!(
                fs::read(&path).unwrap() == bytes,
                "refusal must not rewrite input"
            );
            assert!(
                fs::OpenOptions::new().write(true).open(&path).is_ok(),
                "original must close on parser refusal"
            );
        }
    }

    #[test]
    fn native_operator_key_refuses_relative_and_nonlocal_names() {
        assert!(load_operator_key_pair(Path::new("operator.key")).is_err());
        for path in [
            r"\\server\share\operator.key",
            r"\\?\UNC\server\share\operator.key",
            r"\\.\C:\operator.key",
        ] {
            let mut observed = false;
            let error = load_observed(Path::new(path), || {
                observed = true;
                Ok(())
            })
            .err()
            .expect("native namespace must refuse");
            assert!(format!("{error:#}").contains("native custody"));
            assert!(
                !observed,
                "post-read callback must not run for refused names"
            );
        }
    }

    #[test]
    fn native_operator_key_refuses_inherited_and_foreign_acl_without_repair() {
        let (_temporary, directory) = root();
        let (_, encoded) = key(Algorithm::Ed25519);
        let inherited = directory.join("inherited.key");
        fs::write(&inherited, encoded.as_bytes()).unwrap();
        let foreign = write_fixture(&directory, "foreign.key", encoded.as_bytes());
        grant(&foreign, "*S-1-1-0:(R)");
        for path in [&inherited, &foreign] {
            let before = acl(path);
            assert!(load_operator_key_pair(path).is_err());
            assert_eq!(acl(path), before);
            assert!(
                fs::read(path).unwrap() == encoded.as_bytes(),
                "refusal must not mutate input"
            );
        }
    }

    #[test]
    fn native_operator_key_refuses_hardlinks_and_unsafe_parent() {
        let (_temporary, directory) = root();
        let (_, encoded) = key(Algorithm::Ed25519);
        let path = write_fixture(&directory, "operator.key", encoded.as_bytes());
        let alias = directory.join("alias.key");
        fs::hard_link(&path, &alias).unwrap();
        for source in [&path, &alias] {
            assert!(load_operator_key_pair(source).is_err());
            assert!(
                fs::read(source).unwrap() == encoded.as_bytes(),
                "original bytes preserved"
            );
        }
        fs::remove_file(alias).unwrap();
        assert!(load_operator_key_pair(&path).is_ok());
        grant(&directory, "*S-1-1-0:(W)");
        let before = acl(&directory);
        assert!(load_operator_key_pair(&path).is_err());
        assert_eq!(acl(&directory), before);
    }

    #[test]
    fn native_operator_key_refuses_junction_parent_and_ancestor() {
        let (_temporary, directory) = root();
        let (_, encoded) = key(Algorithm::Ed25519);
        let real = directory.join("real");
        fs::create_dir(&real).unwrap();
        let path = write_fixture(&real, "operator.key", encoded.as_bytes());
        let nested = real.join("nested");
        fs::create_dir(&nested).unwrap();
        write_fixture(&nested, "nested.key", encoded.as_bytes());
        let junction = directory.join("junction");
        let output = Command::new("cmd.exe")
            .current_dir(&directory)
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&real)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "actual junction fixture must be created"
        );
        for source in [
            junction.join("operator.key"),
            junction.join("nested/nested.key"),
        ] {
            let mut observed = false;
            assert!(
                load_observed(&source, || {
                    observed = true;
                    Ok(())
                })
                .is_err()
            );
            assert!(!observed);
        }
        assert!(load_operator_key_pair(&path).is_ok());
        assert!(
            fs::read(&path).unwrap() == encoded.as_bytes(),
            "outside original unchanged"
        );
        fs::remove_dir(junction).expect("remove only owned junction");
    }

    #[test]
    fn native_operator_key_refuses_writer_and_excludes_replacement_during_read() {
        let (_temporary, directory) = root();
        let (expected, encoded) = key(Algorithm::Ed25519);
        let path = directory.join("writer.key");
        let mut writer = iroha_fs::RetainedFile::create_new_private(&path).unwrap();
        writer.file_mut().write_all(encoded.as_bytes()).unwrap();
        writer.file().sync_all().unwrap();
        assert!(load_operator_key_pair(&path).is_err());
        drop(writer);
        let mut observed = false;
        let actual = load_observed(&path, || {
            observed = true;
            assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
            assert!(fs::rename(&path, directory.join("displaced.key")).is_err());
            assert!(fs::remove_file(&path).is_err());
            assert!(fs::rename(&directory, directory.with_extension("displaced")).is_err());
            Ok(())
        })
        .expect("held original survives read and parse");
        assert!(observed);
        assert_eq!(actual.public_key(), expected.public_key());
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        assert!(
            fs::read(&path).unwrap() == encoded.as_bytes(),
            "original bytes preserved"
        );
    }

    #[test]
    fn native_operator_key_rechecks_late_file_and_parent_acl_mutation() {
        for target in ["file", "parent"] {
            let (_temporary, directory) = root();
            let (_, encoded) = key(Algorithm::Ed25519);
            let path = write_fixture(&directory, "changing.key", encoded.as_bytes());
            let mut observed = false;
            let error = load_observed(&path, || {
                observed = true;
                if target == "file" {
                    grant(&path, "*S-1-1-0:(R)");
                } else {
                    grant(&directory, "*S-1-1-0:(W)");
                }
                Ok(())
            })
            .err()
            .expect("late native custody change must refuse populated key bytes");
            assert!(observed);
            assert!(format!("{error:#}").contains("custody changed"));
            assert!(
                fs::read(&path).unwrap() == encoded.as_bytes(),
                "bytes still unchanged"
            );
            assert!(
                load_operator_key_pair(&path).is_err(),
                "loader must not repair ACL"
            );
        }
    }

    #[test]
    fn native_operator_key_observation_error_and_unwind_release_originals() {
        let (_temporary, directory) = root();
        let (_, encoded) = key(Algorithm::Ed25519);
        let path = write_fixture(&directory, "original.key", encoded.as_bytes());
        let secret = "CALLBACK_PRIVATE_VALUE_MUST_NOT_APPEAR";
        let error = load_observed(&path, || Err(io::Error::other(secret)))
            .err()
            .expect("observation failure must refuse");
        assert_eq!(
            format!("{error:#}"),
            "private-key file read observation failed"
        );
        assert!(!format!("{error:#}").contains(secret));
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        let panic = std::panic::catch_unwind(|| {
            let _ = load_observed(&path, || -> io::Result<()> {
                panic!("public fixture unwind")
            });
        });
        assert!(panic.is_err());
        let moved = directory.join("moved.key");
        fs::rename(&path, &moved).expect("unwound reader releases original file");
        assert!(load_operator_key_pair(&moved).is_ok());
        fs::remove_file(moved).unwrap();
        fs::remove_dir(&directory).expect("unwound reader releases original ancestors");
    }
}
