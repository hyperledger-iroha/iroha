//! Native directory placement for client publication history, separate from daemon replay custody.
//!
//! This module owns no operation format, parser, request or signing authority. Musubi owns every
//! operation journal and sidecar. Generated profiles initialize this empty directory while their
//! generation is unpublished; all later generated actions must open its original custody.
use iroha_fs::{OwnerDirectory, PrivateDirectory};
use std::{io, path::Path};

/// Fixed child directory containing Musubi client operation journals and their staged sidecars.
pub const DIRECTORY_NAME: &str = "publication-v1";

/// Initialize an original empty client journal directory beneath an existing safe parent.
///
/// This create-only entry is for the unpublished generation producer. Retained generation reads
/// and generated publication actions must use [`open_existing`], never reconstruct lost history.
/// The outer parent is not created. No operation, lock, request or authorization is produced.
///
/// # Errors
/// Refuses a missing or unsafe parent, an existing child of any kind, changed custody or native
/// creation/synchronization failures. An ambiguous failure is not permission to repair a generation.
pub fn initialize(parent: &Path) -> io::Result<PrivateDirectory> {
    let owner = OwnerDirectory::open(parent)?;
    initialize_under(&owner)
}

fn initialize_under(owner: &OwnerDirectory) -> io::Result<PrivateDirectory> {
    let directory = owner.create_private_child(DIRECTORY_NAME)?;
    directory.sync()?;
    owner.sync()?;
    owner.revalidate()?;
    directory.revalidate()?;
    Ok(directory)
}

/// Open original client publication custody without creating its parent or inner directory.
///
/// # Errors
/// Refuses absent, replaced, redirected or unsafe custody and native I/O failures.
pub fn open_existing(parent: &Path) -> io::Result<PrivateDirectory> {
    let owner = OwnerDirectory::open(parent)?;
    open_under(&owner)
}

fn open_under(owner: &OwnerDirectory) -> io::Result<PrivateDirectory> {
    let directory = PrivateDirectory::open_exact(owner.path().join(DIRECTORY_NAME))?;
    owner.revalidate()?;
    directory.revalidate()?;
    Ok(directory)
}

/// Open or initialize client history for an ordinary explicitly selected publication state root.
///
/// Generated profiles must use [`initialize`] only during creation and [`open_existing`] thereafter.
/// This ordinary first-use entry preserves explicit Musubi Begin semantics; it never creates the
/// outer parent or interprets an operation journal.
///
/// # Errors
/// Refuses unsafe or changed custody, absent parents and native creation/synchronization failures.
pub fn open_or_create(parent: &Path) -> io::Result<PrivateDirectory> {
    let owner = OwnerDirectory::open(parent)?;
    match initialize_under(&owner) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => open_under(&owner),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_is_create_only_and_existing_open_never_repairs_loss() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("operations");
        assert!(initialize(&path).is_err());
        assert!(open_existing(&path).is_err());
        assert!(open_or_create(&path).is_err());
        assert!(!path.exists());
        let parent = PrivateDirectory::open_or_create(&path).unwrap();
        assert!(open_existing(&path).is_err());
        assert!(parent.entries(1).unwrap().is_empty());
        let original = initialize(&path).unwrap();
        let identity = original.identity().unwrap();
        assert!(original.entries(1).unwrap().is_empty());
        assert!(initialize(&path).is_err());
        assert_eq!(open_existing(&path).unwrap().identity().unwrap(), identity);
        assert_eq!(open_or_create(&path).unwrap().identity().unwrap(), identity);
        drop(original);
        std::fs::remove_dir(path.join(DIRECTORY_NAME)).unwrap();
        assert!(open_existing(&path).is_err());
        assert!(parent.entries(1).unwrap().is_empty());
        drop(parent);
        std::fs::remove_dir(&path).unwrap();
        assert!(open_existing(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn ordinary_first_use_opens_same_private_directory_and_refuses_a_file() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = PrivateDirectory::open_or_create(temporary.path().join("operations")).unwrap();
        let original = open_or_create(parent.path()).unwrap();
        let identity = original.identity().unwrap();
        assert_eq!(
            open_or_create(parent.path()).unwrap().identity().unwrap(),
            identity
        );
        let path = original.path().to_owned();
        drop(original);
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"not a private directory").unwrap();
        assert!(initialize(parent.path()).is_err());
        assert!(open_existing(parent.path()).is_err());
        assert!(open_or_create(parent.path()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not a private directory");
    }
}
