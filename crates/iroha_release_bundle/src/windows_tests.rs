//! Native recovery rejects byte-identical substitutes after all publication owners close.

use super::*;
use std::{
    io::{Read as _, Write as _},
    path::Path,
    process::Command,
};

use iroha_fs::{FileIdentity, FileMetadata, SealedFlatBatch};

use crate::bundle::{MANIFEST_INDEX, inventory};

const NAME: &str = "invocation.closed";

struct ClosedBundle {
    fixture: Fixture,
    path: PathBuf,
    directory_identity: FileIdentity,
    identities: [FileIdentity; 5],
    bytes: [Vec<u8>; 5],
    lock_anchor: String,
    manifest_anchor: String,
}

impl ClosedBundle {
    fn new() -> Self {
        let fixture = Fixture::new();
        fixture.use_native_probe();
        let bundle = publish(&fixture.owner, NAME, fixture.inputs()).unwrap();
        let directory_identity = bundle.batch.identity().unwrap();
        let identities = std::array::from_fn(|index| bundle.batch.entry_identity(index).unwrap());
        let bytes = std::array::from_fn(|index| {
            let mut bytes = Vec::new();
            bundle
                .batch
                .reader(index)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        });
        let path = bundle.path().to_owned();
        let lock_anchor = bundle.manifest().cargo_lock_sha256.clone();
        let manifest_anchor = bundle.manifest_sha256().to_owned();
        bundle.revalidate().unwrap();
        drop(bundle);
        Self {
            fixture,
            path,
            directory_identity,
            identities,
            bytes,
            lock_anchor,
            manifest_anchor,
        }
    }

    fn anchors(&self) -> BundleAnchors<'_> {
        BundleAnchors {
            source: &self.fixture.source,
            cargo_lock: &self.lock_anchor,
            manifest: &self.manifest_anchor,
        }
    }

    fn recover_lower(&self) -> SealedFlatBatch<5> {
        let parent = ReaderDirectory::open(self.fixture.owner.path()).unwrap();
        let batch = parent
            .open_sealed_flat_batch(NAME, inventory().unwrap())
            .unwrap();
        for (index, expected) in self.bytes.iter().enumerate() {
            let mut actual = Vec::new();
            batch
                .reader(index)
                .unwrap()
                .read_to_end(&mut actual)
                .unwrap();
            assert_eq!(&actual, expected, "replacement changed slot {index} bytes");
        }
        batch.revalidate().unwrap();
        batch
    }

    fn reject_identity(&self, lower: &SealedFlatBatch<5>, expected_error: &str) {
        // The lower owner has already accepted the complete native security policy and
        // inventory. Refusal must come from the independent manifest's original IDs.
        let parent = ReaderDirectory::open(self.fixture.owner.path()).unwrap();
        let error = Bundle::open(&parent, NAME, self.anchors()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), expected_error);
        lower.revalidate().unwrap();
    }
}

fn set_owner_access(path: &Path, access: &str) {
    let identity = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .unwrap();
    assert!(identity.status.success());
    let identity = String::from_utf8_lossy(&identity.stdout);
    let sid = identity
        .rsplit(',')
        .next()
        .unwrap()
        .trim()
        .trim_matches('"');
    assert!(
        sid.starts_with("S-1-")
            && sid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
    );
    // Only this disposable fixture's effective owner gains temporary access. Original
    // leaves keep their canonical seal; no inherited, public or administrator ACE is added.
    // Replacing a grant can retain the creator directory's inheritable owner ACE.
    // Remove that disposable fixture ACE first; its owner retains WRITE_DAC, allowing
    // the next command to install exactly one non-inheritable canonical grant.
    let removed = Command::new("icacls")
        .arg(path)
        .arg("/remove:g")
        .arg(format!("*{sid}"))
        .arg("/Q")
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "{}{}",
        String::from_utf8_lossy(&removed.stdout),
        String::from_utf8_lossy(&removed.stderr)
    );
    let result = Command::new("icacls")
        .arg(path)
        .arg("/grant:r")
        .arg(format!("*{sid}:{access}"))
        .arg("/Q")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn reject_equal_byte_leaf(index: usize) {
    let closed = ClosedBundle::new();
    let entries = inventory().unwrap();
    let entry = &entries[index];
    let original = closed.path.join(entry.name());
    let displaced = closed.fixture.owner.path().join("displaced-original");
    set_owner_access(&closed.path, "F");
    fs::rename(&original, &displaced).unwrap();
    assert_eq!(
        FileMetadata::symlink_metadata(&displaced)
            .unwrap()
            .identity(),
        closed.identities[index]
    );
    {
        let parent = OwnerDirectory::open(&closed.path).unwrap();
        let mut writer = parent
            .create_retained_private(entry.name(), closed.bytes[index].len())
            .unwrap();
        writer.write_all(&closed.bytes[index]).unwrap();
        match entry.kind() {
            iroha_fs::FlatBatchKind::Executable => drop(writer.seal_executable().unwrap()),
            iroha_fs::FlatBatchKind::ReadOnly => drop(writer.seal_read_only().unwrap()),
        }
    }
    set_owner_access(&closed.path, "RX");
    let lower = closed.recover_lower();
    assert_eq!(lower.identity().unwrap(), closed.directory_identity);
    for (slot, expected) in closed.identities.iter().enumerate() {
        let current = lower.entry_identity(slot).unwrap();
        if slot == index {
            assert_ne!(&current, expected);
        } else {
            assert_eq!(&current, expected);
        }
    }
    closed.reject_identity(
        &lower,
        if index == MANIFEST_INDEX {
            "release directory or manifest original was substituted"
        } else {
            "release executable original was substituted"
        },
    );
    // The displaced original remains allocated throughout admission, preventing identity
    // reuse from making the positive byte-equality control accidentally inconclusive.
    assert_eq!(
        FileMetadata::symlink_metadata(&displaced)
            .unwrap()
            .identity(),
        closed.identities[index]
    );
    assert_eq!(fs::read(&displaced).unwrap(), closed.bytes[index]);
}

#[test]
fn recovered_irohad_rejects_equal_byte_sealed_substitute() {
    reject_equal_byte_leaf(BinaryKind::Irohad.index());
}

#[test]
fn recovered_iroha_rejects_equal_byte_sealed_substitute() {
    reject_equal_byte_leaf(BinaryKind::Iroha.index());
}

#[test]
fn recovered_kagami_rejects_equal_byte_sealed_substitute() {
    reject_equal_byte_leaf(BinaryKind::Kagami.index());
}

#[test]
fn recovered_irohad_taira_rejects_equal_byte_sealed_substitute() {
    reject_equal_byte_leaf(BinaryKind::IrohadTaira.index());
}

#[test]
fn recovered_manifest_rejects_equal_byte_sealed_substitute() {
    reject_equal_byte_leaf(MANIFEST_INDEX);
}

#[test]
fn recovered_directory_rejects_substitute_with_all_five_original_children() {
    let closed = ClosedBundle::new();
    let displaced = closed.fixture.owner.path().join("displaced-directory");
    fs::rename(&closed.path, &displaced).unwrap();
    assert_eq!(
        FileMetadata::symlink_metadata(&displaced)
            .unwrap()
            .identity(),
        closed.directory_identity
    );
    set_owner_access(&displaced, "F");
    {
        let replacement = closed.fixture.owner.create_child(NAME).unwrap();
        assert_ne!(replacement.identity().unwrap(), closed.directory_identity);
        for entry in inventory().unwrap() {
            let destination = replacement.path().join(entry.name());
            fs::rename(displaced.join(entry.name()), &destination).unwrap();
            // Cross-parent moves can recompute native access. Restore each original's
            // canonical seal so recovery tests directory identity, not an invalid DACL.
            set_owner_access(
                &destination,
                match entry.kind() {
                    iroha_fs::FlatBatchKind::Executable => "RX",
                    iroha_fs::FlatBatchKind::ReadOnly => "R",
                },
            );
        }
    }
    set_owner_access(&displaced, "RX");
    set_owner_access(&closed.path, "RX");
    let lower = closed.recover_lower();
    assert_ne!(lower.identity().unwrap(), closed.directory_identity);
    for (index, expected) in closed.identities.iter().enumerate() {
        assert_eq!(&lower.entry_identity(index).unwrap(), expected);
    }
    closed.reject_identity(
        &lower,
        "release directory or manifest original was substituted",
    );
    assert_eq!(
        FileMetadata::symlink_metadata(&displaced)
            .unwrap()
            .identity(),
        closed.directory_identity
    );
    assert_eq!(fs::read_dir(&displaced).unwrap().count(), 0);
}
