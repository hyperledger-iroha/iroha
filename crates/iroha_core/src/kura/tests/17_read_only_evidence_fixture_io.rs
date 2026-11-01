// Native fixture IO; every owner returned here is dropped before reader admission.
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
pub(super) type Snapshot = Vec<(Vec<u8>, u32, i64, i64, i64, i64, u64)>;
#[cfg(windows)]
pub(super) type Snapshot = Vec<(Vec<u8>, iroha_fs::FileSnapshot, Vec<u8>)>;

pub(super) fn root(directory: &tempfile::TempDir) -> PathBuf {
    #[cfg(unix)]
    let root = directory.path().to_path_buf();
    #[cfg(windows)]
    let root = {
        let owner = iroha_fs::PrivateDirectory::open_or_create(directory.path().join("store"))
            .expect("protected current-owner native fixture directory");
        owner.path().to_path_buf()
    };
    root.canonicalize()
        .expect("absolute real fixture directory")
}

pub(super) fn write(root: &Path, name: &str, bytes: &[u8]) {
    #[cfg(unix)]
    fs::write(root.join(name), bytes).unwrap();
    #[cfg(windows)]
    iroha_fs::PrivateDirectory::open(root)
        .unwrap()
        .write_atomic(name, bytes, iroha_fs::PublishMode::Replace)
        .expect("protected current-owner native fixture file");
}

pub(super) fn read_only(root: &Path, path: &Path) {
    #[cfg(unix)]
    {
        let _ = root;
        fs::set_permissions(path, fs::Permissions::from_mode(0o400)).unwrap();
    }
    #[cfg(windows)]
    {
        use std::io::Write as _;
        let bytes = fs::read(path).unwrap();
        fs::remove_file(path).unwrap();
        let owner = iroha_fs::PrivateDirectory::open(root).unwrap();
        let mut writer = owner
            .create_retained_private(path.file_name().unwrap(), bytes.len())
            .unwrap();
        writer.write_all(&bytes).unwrap();
        drop(
            writer
                .seal_read_only()
                .expect("exact protected owner-read-only native seal"),
        );
        owner
            .open_retained_read_only(path.file_name().unwrap(), bytes.len())
            .expect("fresh canonical read-only seal admission");
    }
}

pub(super) fn snapshot(files: &[PathBuf]) -> Snapshot {
    files
        .iter()
        .map(|path| {
            #[cfg(unix)]
            {
                let metadata = fs::metadata(path).unwrap();
                (
                    fs::read(path).unwrap(),
                    metadata.mode(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                    metadata.ino(),
                )
            }
            #[cfg(windows)]
            {
                let file = fs::File::open(path).unwrap();
                let snapshot = iroha_fs::FileSnapshot::of(&file, false).unwrap();
                let acl = std::process::Command::new("icacls.exe")
                    .arg(path)
                    .output()
                    .unwrap();
                assert!(
                    acl.status.success(),
                    "{}",
                    String::from_utf8_lossy(&acl.stderr)
                );
                (fs::read(path).unwrap(), snapshot, acl.stdout)
            }
        })
        .collect()
}

pub(super) fn directory_identity(path: &Path) -> iroha_fs::FileIdentity {
    #[cfg(unix)]
    {
        iroha_fs::FileIdentity::of(&fs::File::open(path).unwrap()).unwrap()
    }
    #[cfg(windows)]
    {
        iroha_fs::ReaderDirectory::open(path)
            .unwrap()
            .identity()
            .unwrap()
    }
}

#[cfg(windows)]
pub(super) fn grant_foreign_write(path: &Path) {
    let output = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(W)"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual ACL injection failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fixture_io_creates_and_seals_actual_private_native_originals() {
    let temporary = tempfile::tempdir().unwrap();
    let root = root(&temporary);
    let identity = directory_identity(&root);
    write(&root, "record", b"original fixture");
    #[cfg(unix)]
    {
        // The portable reader permits public-read fixtures on Unix. This additional sealing
        // control selects explicit private permissions before testing canonical private APIs.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(root.join("record"), fs::Permissions::from_mode(0o600)).unwrap();
    }
    let owner = iroha_fs::PrivateDirectory::open(&root).unwrap();
    let file = owner.open_read("record").unwrap();
    iroha_fs::FileSnapshot::private_journal(&file).unwrap();
    drop(file);
    drop(owner);
    let path = root.join("record");
    read_only(&root, &path);
    let before = snapshot(std::slice::from_ref(&path));
    let owner = iroha_fs::PrivateDirectory::open(&root).unwrap();
    let sealed = owner.open_retained_read_only("record", 16).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original fixture");
    assert_eq!(snapshot(std::slice::from_ref(&path)), before);
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert_eq!(directory_identity(&root), identity);
    drop(sealed);
}

#[cfg(windows)]
#[test]
fn fixture_io_native_acl_injection_reaches_and_invalidates_retained_originals() {
    for parent in [false, true] {
        let temporary = tempfile::tempdir().unwrap();
        let root = root(&temporary);
        write(&root, "record", b"original fixture");
        let directory = iroha_fs::ReaderDirectory::open(&root).unwrap();
        let original = directory.open_retained_regular("record").unwrap();
        original.require_current_owner().unwrap();
        let path = root.join("record");
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::rename(&path, path.with_extension("displaced")).is_err());
        original.revalidate().unwrap();
        grant_foreign_write(if parent { &root } else { &path });
        assert!(original.require_current_owner().is_err());
    }
}
