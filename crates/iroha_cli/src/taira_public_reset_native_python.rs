//! One explicitly selected native Python image, retained across every helper child.

use super::{host, host_pair::NativePublicFileV1, native_edge_protocol, sha256_hex};
use eyre::{Result, eyre};
use host::ProcessRunner as _;
use iroha_fs::RetainedFile;
use std::{
    io::{Read, Seek as _},
    path::{Path, PathBuf},
    time::Instant,
};

pub(super) struct NativePythonRuntime {
    image: RetainedFile,
    reference: NativePublicFileV1,
}

impl NativePythonRuntime {
    fn retain(reference: &NativePublicFileV1, owner_uid: u32) -> Result<Self> {
        reference.validate_python(owner_uid)?;
        let mut image = RetainedFile::open_regular(Path::new(&reference.file.path))?;
        if native_edge_protocol::native_file_identity(&image.file().metadata()?)?
            != reference.file.identity
        {
            return Err(eyre!(
                "native Python image differs from its selected identity"
            ));
        }
        let mut bytes = Vec::new();
        image
            .file_mut()
            .take(reference.file.identity.size + 1)
            .read_to_end(&mut bytes)?;
        image.file_mut().rewind()?;
        if u64::try_from(bytes.len())? != reference.file.identity.size
            || sha256_hex(&bytes) != reference.sha256
        {
            return Err(eyre!(
                "native Python image differs from its selected digest"
            ));
        }
        let runtime = Self {
            image,
            reference: reference.clone(),
        };
        runtime.revalidate()?;
        Ok(runtime)
    }

    pub(super) fn admit(
        reference: &NativePublicFileV1,
        owner_uid: u32,
        deadline: Instant,
    ) -> Result<Self> {
        let runtime = Self::retain(reference, owner_uid)?;
        let output = host::RealProcessRunner.run(&host::ProcessSpec::public_input(
            runtime.program()?,
            vec![
                "-B".into(),
                "-I".into(),
                "-c".into(),
                "import sys; assert sys.version_info >= (3,11)".into(),
            ],
            Vec::new(),
            deadline,
        ));
        runtime.revalidate()?;
        let output = output?;
        if !output.status.success() || !output.stdout.is_empty() || !output.stderr.is_empty() {
            return Err(eyre!(
                "selected native Python requires actual version 3.11 or later"
            ));
        }
        Ok(runtime)
    }

    pub(super) fn program(&self) -> Result<PathBuf> {
        self.revalidate()?;
        Ok(PathBuf::from(&self.reference.file.path))
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.image.revalidate()?;
        if native_edge_protocol::native_file_identity(&self.image.file().metadata()?)?
            != self.reference.file.identity
        {
            return Err(eyre!("retained native Python image changed"));
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::host_pair::NativeObservedFileV1;
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt as _, symlink},
        time::Duration,
    };

    /// This selected executable is a custody/refusal fixture, never a qualified Python runtime.
    struct ImageFixture {
        _directory: tempfile::TempDir,
        path: PathBuf,
        reference: NativePublicFileV1,
        owner_uid: u32,
    }

    impl ImageFixture {
        fn new(bytes: &[u8]) -> Self {
            let directory = tempfile::Builder::new()
                .prefix("native-python-custody-test-")
                .tempdir_in(std::env::var_os("HOME").expect("test requires its native owner home"))
                .unwrap();
            let physical = fs::canonicalize(directory.path()).unwrap();
            fs::set_permissions(&physical, fs::Permissions::from_mode(0o700)).unwrap();
            let path = physical.join("selected-image");
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            let reference = NativePublicFileV1 {
                file: NativeObservedFileV1 {
                    path: path.to_str().unwrap().to_owned(),
                    identity: native_edge_protocol::native_file_identity(
                        &fs::metadata(&path).unwrap(),
                    )
                    .unwrap(),
                },
                sha256: sha256_hex(bytes),
            };
            Self {
                _directory: directory,
                path,
                reference,
                owner_uid: rustix::process::geteuid().as_raw(),
            }
        }

        fn refresh_reference(&mut self) {
            self.reference.file.identity =
                native_edge_protocol::native_file_identity(&fs::metadata(&self.path).unwrap())
                    .unwrap();
            self.reference.sha256 = sha256_hex(&fs::read(&self.path).unwrap());
        }
    }

    #[test]
    fn native_python_custody_rejects_wrong_digest_metadata_and_unsafe_modes() {
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let runtime =
                NativePythonRuntime::retain(&fixture.reference, fixture.owner_uid).unwrap();
            assert_eq!(runtime.program().unwrap(), fixture.path);
            runtime.revalidate().unwrap();
        }
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let mut changed = fixture.reference.clone();
            changed.sha256 = "0".repeat(64);
            assert!(NativePythonRuntime::retain(&changed, fixture.owner_uid).is_err());
        }
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let mut changed = fixture.reference.clone();
            changed.file.identity.size += 1;
            assert!(NativePythonRuntime::retain(&changed, fixture.owner_uid).is_err());
        }
        {
            for mode in [0o644, 0o777, 0o4755] {
                let mut fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
                fs::set_permissions(&fixture.path, fs::Permissions::from_mode(mode)).unwrap();
                fixture.refresh_reference();
                assert!(
                    NativePythonRuntime::retain(&fixture.reference, fixture.owner_uid).is_err(),
                    "mode {mode:o}"
                );
            }
        }
    }

    #[test]
    fn native_python_direct_selection_refuses_symlink_leaf_and_ancestor() {
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let alias = fixture.path.parent().unwrap().join("image-alias");
            symlink(&fixture.path, &alias).unwrap();
            let mut changed = fixture.reference.clone();
            changed.file.path = alias.to_str().unwrap().to_owned();
            assert!(NativePythonRuntime::retain(&changed, fixture.owner_uid).is_err());
        }
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let parent = fixture.path.parent().unwrap();
            let physical = parent.join("physical");
            fs::create_dir(&physical).unwrap();
            fs::set_permissions(&physical, fs::Permissions::from_mode(0o700)).unwrap();
            let moved = physical.join("selected-image");
            fs::rename(&fixture.path, &moved).unwrap();
            let alias = parent.join("ancestor-alias");
            symlink(&physical, &alias).unwrap();
            let mut changed = fixture.reference.clone();
            changed.file.path = alias.join("selected-image").to_str().unwrap().to_owned();
            changed.file.identity =
                native_edge_protocol::native_file_identity(&fs::metadata(&moved).unwrap()).unwrap();
            assert!(NativePythonRuntime::retain(&changed, fixture.owner_uid).is_err());
        }
    }

    #[test]
    fn native_python_retained_image_refuses_content_change_and_path_replacement() {
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let runtime =
                NativePythonRuntime::retain(&fixture.reference, fixture.owner_uid).unwrap();
            fs::write(&fixture.path, b"#!/bin/sh\nexit 77\n").unwrap();
            assert!(runtime.revalidate().is_err());
            assert!(runtime.program().is_err());
        }
        {
            let fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let runtime =
                NativePythonRuntime::retain(&fixture.reference, fixture.owner_uid).unwrap();
            let replacement = fixture.path.parent().unwrap().join("replacement");
            fs::write(&replacement, b"#!/bin/sh\nexit 7\n").unwrap();
            fs::set_permissions(&replacement, fs::Permissions::from_mode(0o755)).unwrap();
            fs::rename(replacement, &fixture.path).unwrap();
            assert!(runtime.revalidate().is_err());
            assert!(runtime.program().is_err());
        }
    }

    #[test]
    fn native_python_actual_finite_child_failure_and_expired_deadline_are_refused() {
        {
            let mut fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let marker = fixture.path.parent().unwrap().join("attempted");
            let script = format!(
                "#!/bin/sh\nprintf attempted > '{}'\nexit 7\n",
                marker.display()
            );
            fs::write(&fixture.path, script.as_bytes()).unwrap();
            fixture.refresh_reference();
            assert!(
                NativePythonRuntime::admit(
                    &fixture.reference,
                    fixture.owner_uid,
                    Instant::now() + Duration::from_secs(10)
                )
                .is_err()
            );
            assert_eq!(fs::read(marker).unwrap(), b"attempted");
        }
        {
            let mut fixture = ImageFixture::new(b"#!/bin/sh\nexit 7\n");
            let marker = fixture.path.parent().unwrap().join("attempted");
            let script = format!(
                "#!/bin/sh\nprintf attempted > '{}'\nexit 7\n",
                marker.display()
            );
            fs::write(&fixture.path, script.as_bytes()).unwrap();
            fixture.refresh_reference();
            assert!(
                NativePythonRuntime::admit(
                    &fixture.reference,
                    fixture.owner_uid,
                    Instant::now() - Duration::from_secs(1)
                )
                .is_err()
            );
            assert!(!marker.exists());
        }
    }
}
