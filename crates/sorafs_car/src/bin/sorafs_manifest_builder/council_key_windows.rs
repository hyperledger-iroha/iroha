//! Read one raw council seed through existing Windows private-file custody.
//!
//! This is authenticated software custody: the original local file and its ancestry remain
//! retained through the bounded read. Admission never creates files or repairs permissions.

use std::{
    io::{self, Read as _},
    path::Path,
};

use super::{COUNCIL_SIGNING_KEY_SEED_BYTES, CouncilSigningSeed};

pub(super) fn read(path: &Path) -> Result<CouncilSigningSeed, String> {
    read_observed(path, || Ok(()))
}

// The callback is private dependency injection for the final custody boundary, not a runtime
// policy toggle. Production supplies no operation; tests exercise actual retained originals.
fn read_observed(
    path: &Path,
    after_read: impl FnOnce() -> io::Result<()>,
) -> Result<CouncilSigningSeed, String> {
    // Reject placeholders lexically, then let the native owner validate the entire path before
    // opening anything. In particular, do not probe UNC/device paths through std metadata first.
    if path.as_os_str().is_empty() || path == Path::new("-") {
        return Err("--council-signing-key-file requires a regular file path".to_owned());
    }
    let mut file = iroha_fs::RetainedFile::open_private(path).map_err(|_| {
        "council signing key requires a regular file with private current-owner native custody"
            .to_owned()
    })?;
    let before = file
        .snapshot()
        .map_err(|_| "failed to inspect retained council signing key custody".to_owned())?;
    let length = file
        .file()
        .metadata()
        .map_err(|_| "failed to inspect retained council signing key length".to_owned())?
        .len();
    if length != COUNCIL_SIGNING_KEY_SEED_BYTES as u64 {
        return Err(format!(
            "council signing key file must contain exactly {COUNCIL_SIGNING_KEY_SEED_BYTES} raw bytes"
        ));
    }
    let mut seed = CouncilSigningSeed([0_u8; COUNCIL_SIGNING_KEY_SEED_BYTES]);
    file.file_mut()
        .read_exact(&mut seed.0)
        .map_err(|_| "failed to read complete retained council signing key".to_owned())?;
    let mut trailing = [0_u8; 1];
    if file
        .file_mut()
        .read(&mut trailing)
        .map_err(|_| "failed to finish reading retained council signing key".to_owned())?
        != 0
    {
        return Err("council signing key changed length while being read".to_owned());
    }
    after_read().map_err(|_| "council signing key read observation failed".to_owned())?;
    let after = file
        .snapshot()
        .map_err(|_| "council signing key custody changed while being read".to_owned())?;
    if before != after {
        return Err("council signing key changed while being read".to_owned());
    }
    file.revalidate()
        .map_err(|_| "council signing key custody changed after reading".to_owned())?;
    Ok(seed)
}

#[cfg(test)]
pub(super) fn write_fixture(directory: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    use std::io::Write as _;

    let path = directory.join(name);
    let mut original = iroha_fs::RetainedFile::create_new_private(&path)
        .expect("create private current-owner seed fixture");
    original.file_mut().write_all(bytes).expect("write fixture");
    original.file().sync_all().expect("sync fixture");
    original.revalidate().expect("fixture original custody");
    drop(original);
    path
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write as _, process::Command};

    use super::*;

    fn root() -> (tempfile::TempDir, std::path::PathBuf) {
        let temporary = tempfile::tempdir().expect("fixture root");
        let path = temporary.path().canonicalize().expect("canonical root");
        (temporary, path)
    }

    fn acl(path: &Path) -> Vec<u8> {
        let output = Command::new("icacls.exe")
            .arg(path)
            .output()
            .expect("read ACL");
        assert!(output.status.success(), "fixture ACL query failed");
        output.stdout
    }

    fn grant(path: &Path, permission: &str) {
        let output = Command::new("icacls.exe")
            .arg(path)
            .args(["/grant", permission])
            .output()
            .expect("change only owned fixture ACL");
        assert!(output.status.success(), "fixture ACL grant failed");
    }

    #[test]
    fn native_council_key_refuses_nonlocal_paths_before_read_observation() {
        for path in [
            r"\\server\share\key.seed",
            r"\\?\UNC\server\share\key.seed",
            r"\\.\C:\key.seed",
        ] {
            let mut observed = false;
            let error = read_observed(Path::new(path), || {
                observed = true;
                Ok(())
            })
            .err()
            .expect("nonlocal namespace must refuse");
            assert!(error.contains("native custody"));
            assert!(!observed, "no seed read may precede namespace admission");
        }
    }

    #[test]
    fn native_council_key_reads_private_original_without_acl_or_byte_changes() {
        let (_temporary, directory) = root();
        let path = write_fixture(&directory, "private.seed", &[0xA5; 32]);
        let before = iroha_fs::RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap();
        let file_acl = acl(&path);
        let directory_acl = acl(&directory);
        let seed = read(&path).expect("private raw seed");
        assert_eq!(seed.as_bytes(), &[0xA5; 32]);
        assert_eq!(fs::read(&path).unwrap(), [0xA5; 32]);
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

    #[test]
    fn native_council_key_refuses_inherited_and_foreign_acl_without_repair() {
        let (_temporary, directory) = root();
        let inherited = directory.join("inherited.seed");
        fs::write(&inherited, [0xA5; 32]).unwrap();
        let foreign = write_fixture(&directory, "foreign.seed", &[0xA5; 32]);
        grant(&foreign, "*S-1-1-0:(R)");
        for path in [&inherited, &foreign] {
            let before = acl(path);
            let error = read(path).err().expect("nonprivate source must refuse");
            assert!(error.contains("private current-owner native custody"));
            assert_eq!(acl(path), before, "admission must never repair ACLs");
            assert_eq!(fs::read(path).unwrap(), [0xA5; 32]);
        }
    }

    #[test]
    fn native_council_key_refuses_hardlinks_and_unsafe_parent_without_repair() {
        let (_temporary, directory) = root();
        let path = write_fixture(&directory, "linked.seed", &[0xA5; 32]);
        let alias = directory.join("alias.seed");
        fs::hard_link(&path, &alias).unwrap();
        for source in [&path, &alias] {
            assert!(read(source).err().unwrap().contains("native custody"));
            assert_eq!(fs::read(source).unwrap(), [0xA5; 32]);
        }
        fs::remove_file(alias).unwrap();
        assert!(read(&path).is_ok());
        grant(&directory, "*S-1-1-0:(W)");
        let before = acl(&directory);
        assert!(read(&path).err().unwrap().contains("native custody"));
        assert_eq!(acl(&directory), before);
        assert_eq!(fs::read(&path).unwrap(), [0xA5; 32]);
    }

    #[test]
    fn native_council_key_refuses_junction_parent_and_ancestor() {
        let (_temporary, directory) = root();
        let real = directory.join("real");
        fs::create_dir(&real).unwrap();
        let path = write_fixture(&real, "private.seed", &[0xA5; 32]);
        let link = directory.join("junction");
        let output = Command::new("cmd.exe")
            .current_dir(&directory)
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .unwrap();
        assert!(output.status.success(), "create native junction fixture");
        assert!(read(&link.join("private.seed")).is_err());
        let nested = real.join("nested");
        fs::create_dir(&nested).unwrap();
        write_fixture(&nested, "nested.seed", &[0xA5; 32]);
        assert!(read(&link.join("nested/nested.seed")).is_err());
        assert_eq!(read(&path).unwrap().as_bytes(), &[0xA5; 32]);
        fs::remove_dir(link).expect("remove only owned junction");
    }

    #[test]
    fn native_council_key_refuses_live_writer_and_excludes_replacement_during_read() {
        let (_temporary, directory) = root();
        let path = directory.join("writer.seed");
        let mut writer = iroha_fs::RetainedFile::create_new_private(&path).unwrap();
        writer.file_mut().write_all(&[0xA5; 32]).unwrap();
        writer.file().sync_all().unwrap();
        assert!(read(&path).err().unwrap().contains("native custody"));
        drop(writer);
        let mut observed = false;
        let seed = read_observed(&path, || {
            observed = true;
            assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
            assert!(fs::rename(&path, directory.join("displaced.seed")).is_err());
            assert!(fs::remove_file(&path).is_err());
            Ok(())
        })
        .expect("original remains protected through final read boundary");
        assert!(observed);
        assert_eq!(seed.as_bytes(), &[0xA5; 32]);
        assert_eq!(fs::read(&path).unwrap(), [0xA5; 32]);
        assert!(
            fs::OpenOptions::new().write(true).open(&path).is_ok(),
            "reader releases its original on return"
        );
    }

    #[test]
    fn native_council_key_rechecks_actual_acl_after_read_before_returning_seed() {
        let (_temporary, directory) = root();
        let path = write_fixture(&directory, "changing.seed", &[0xA5; 32]);
        let mut observed = false;
        let error = read_observed(&path, || {
            observed = true;
            grant(&path, "*S-1-1-0:(R)");
            Ok(())
        })
        .err()
        .expect("late custody change must refuse the populated seed");
        assert!(observed);
        assert!(error.contains("custody changed"));
        assert_eq!(fs::read(&path).unwrap(), [0xA5; 32]);
        assert!(
            read(&path).is_err(),
            "failed read must not repair the altered ACL"
        );
    }

    #[test]
    fn native_council_key_observation_failure_releases_original_without_leaking_content() {
        let (_temporary, directory) = root();
        let public_fixture = b"public-test-seed-not-a-real-key!";
        assert_eq!(public_fixture.len(), 32);
        let path = write_fixture(&directory, "failure.seed", public_fixture);
        let error = read_observed(&path, || {
            Err(io::Error::other("public-test-seed-not-a-real-key!"))
        })
        .err()
        .expect("injected post-read error must refuse");
        assert_eq!(error, "council signing key read observation failed");
        assert!(!error.contains("public-test-seed"));
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        assert_eq!(fs::read(&path).unwrap(), public_fixture);
    }
}
