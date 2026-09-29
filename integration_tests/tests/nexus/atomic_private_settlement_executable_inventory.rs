//! Inventory-scoped custody of verified executable bytes.
//!
//! Every PID still resolves its kernel image before and after its status read.
//! A shared image is hashed once while its opened file and path remain unchanged.
//! This is local filesystem identity checking, not remote execution attestation.

use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata},
    io::Read,
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr as _, ensure};
use sha2::{Digest as _, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ImageStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
    mode: u32,
    owner: (u32, u32),
}

impl ImageStamp {
    fn read(metadata: &Metadata) -> Result<Self> {
        ensure!(
            metadata.file_type().is_file(),
            "executable is not a regular file"
        );
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
            mode: metadata.mode(),
            owner: (metadata.uid(), metadata.gid()),
        })
    }
}

struct VerifiedImage {
    file: File,
    stamp: ImageStamp,
    sha256: String,
}

impl VerifiedImage {
    fn verify(&self, path: &Path) -> Result<()> {
        ensure!(
            ImageStamp::read(&self.file.metadata().wrap_err("inspect open executable")?)?
                == self.stamp
                && ImageStamp::read(
                    &fs::symlink_metadata(path).wrap_err("inspect executable path")?
                )? == self.stamp,
            "executable changed during process inventory"
        );
        Ok(())
    }
}

/// Lives for one complete process inventory; no verdict is reused by a later inventory.
#[derive(Default)]
pub(super) struct ExecutableInventory {
    images: BTreeMap<PathBuf, VerifiedImage>,
}

impl ExecutableInventory {
    pub(super) fn sha256(&mut self, path: &Path) -> Result<String> {
        self.sha256_with(path, hash_file)
    }

    fn sha256_with(
        &mut self,
        path: &Path,
        hash: impl FnOnce(&mut File) -> Result<String>,
    ) -> Result<String> {
        if let Some(image) = self.images.get(path) {
            image.verify(path)?;
            return Ok(image.sha256.clone());
        }
        let stamp = ImageStamp::read(&fs::symlink_metadata(path).wrap_err("inspect executable")?)?;
        let mut image = VerifiedImage {
            file: File::open(path).wrap_err("open executable")?,
            stamp,
            sha256: String::new(),
        };
        image.verify(path)?;
        image.sha256 = hash(&mut image.file)?;
        image.verify(path)?;
        let digest = image.sha256.clone();
        self.images.insert(path.to_owned(), image);
        Ok(digest)
    }

    pub(super) fn verify_all(&self) -> Result<()> {
        for (path, image) in &self.images {
            image.verify(path)?;
        }
        Ok(())
    }
}

fn hash_file(file: &mut File) -> Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).wrap_err("hash executable")?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

pub(super) fn sha256_regular_file(path: &Path) -> Result<String> {
    ExecutableInventory::default().sha256(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        os::unix::fs::{PermissionsExt as _, symlink},
    };

    #[test]
    fn shared_image_is_hashed_once_and_exact_bytes_are_bound() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image");
        fs::write(&path, b"abc").unwrap();
        let mut inventory = ExecutableInventory::default();
        let reads = Cell::new(0);
        for _ in 0..32 {
            let digest = inventory
                .sha256_with(&path, |file| {
                    reads.set(reads.get() + 1);
                    hash_file(file)
                })
                .unwrap();
            assert_eq!(
                digest,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        }
        assert_eq!(reads.get(), 1);
        inventory.verify_all().unwrap();
        assert_eq!(
            sha256_regular_file(&path).unwrap(),
            inventory.sha256(&path).unwrap()
        );
    }

    #[test]
    fn changed_or_replaced_image_never_reuses_a_verdict() {
        for replacement in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("image");
            fs::write(&path, b"first image").unwrap();
            let mut inventory = ExecutableInventory::default();
            inventory.sha256(&path).unwrap();
            if replacement {
                let next = temp.path().join("next");
                fs::write(&next, b"first image").unwrap();
                fs::rename(next, &path).unwrap();
            } else {
                fs::write(&path, b"changed image").unwrap();
            }
            assert!(inventory.sha256(&path).is_err());
            assert!(inventory.verify_all().is_err());
        }
    }

    #[test]
    fn change_while_hashing_is_rejected_before_retention() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image");
        fs::write(&path, b"abc").unwrap();
        let mut inventory = ExecutableInventory::default();
        assert!(
            inventory
                .sha256_with(&path, |file| {
                    let result = hash_file(file)?;
                    fs::write(&path, b"changed")?;
                    Ok(result)
                })
                .is_err()
        );
        assert!(inventory.images.is_empty());
    }

    #[test]
    fn symlink_directory_and_deleted_image_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image");
        let link = temp.path().join("link");
        fs::write(&path, b"abc").unwrap();
        symlink(&path, &link).unwrap();
        assert!(sha256_regular_file(&link).is_err());
        assert!(sha256_regular_file(temp.path()).is_err());
        let mut inventory = ExecutableInventory::default();
        inventory.sha256(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert!(inventory.verify_all().is_err());
    }

    #[test]
    fn restored_bytes_and_mtime_do_not_restore_the_cached_change_time() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image");
        fs::write(&path, b"abc").unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let mut inventory = ExecutableInventory::default();
        inventory.sha256(&path).unwrap();
        fs::write(&path, b"def").unwrap();
        fs::write(&path, b"abc").unwrap();
        File::open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert!(inventory.sha256(&path).is_err());
    }

    #[test]
    fn permission_change_is_rejected_and_new_inventory_rehashes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image");
        fs::write(&path, b"abc").unwrap();
        let mut inventory = ExecutableInventory::default();
        let first = inventory.sha256(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(inventory.verify_all().is_err());
        assert_eq!(sha256_regular_file(&path).unwrap(), first);
    }
}
