//! Real retained-file refusal tests; no private-process or platform qualification from fixtures.
use super::*;

fn file(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/qualification/enrollment-service/worker-tests");
    std::fs::create_dir_all(&root).unwrap();
    let temp = tempfile::tempdir_in(root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let directory = PrivateDirectory::open(temp.path()).unwrap();
    directory
        .write_atomic("original", bytes, iroha_fs::PublishMode::CreateNew)
        .unwrap();
    let path = temp.path().join("original");
    (temp, path)
}
#[test]
fn original_admission_binds_exact_bytes_pin_extent_and_lifetime() {
    let (_temp, path) = file(b"configured original DATA");
    let pin = Sha256::digest(b"configured original DATA").into();
    let (owner, bytes) = Original::open(&path, true, 64, Some(pin)).unwrap();
    assert_eq!(&*bytes, b"configured original DATA");
    owner.revalidate().unwrap();
    assert!(Original::open(&path, true, 1, Some(pin)).is_err());
    assert!(Original::open(&path, true, 64, Some([99; 32])).is_err());
    assert!(Original::open(&path, true, 64, Some([0; 32])).is_err());
    std::fs::write(&path, b"replaced content original").unwrap();
    assert!(owner.revalidate().is_err());
}
#[test]
fn original_loss_truncation_and_namespace_replacement_remain_unavailable() {
    for action in 0..3 {
        let (_temp, path) = file(b"configured original DATA");
        let (owner, _) = Original::open(&path, true, 64, None).unwrap();
        match action {
            0 => std::fs::remove_file(&path).unwrap(),
            1 => std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(0)
                .unwrap(),
            _ => {
                std::fs::rename(&path, path.with_extension("old")).unwrap();
                std::fs::write(&path, b"configured original DATA").unwrap();
            }
        }
        assert!(owner.revalidate().is_err());
    }
    let (_temp, path) = file(b"");
    assert!(Original::open(&path, true, 64, None).is_err());
}
#[cfg(unix)]
#[test]
fn private_original_refuses_shared_mode_links_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let (_temp, path) = file(b"secret DATA");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Original::open(&path, true, 64, None).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let linked = path.with_extension("linked");
    std::fs::hard_link(&path, &linked).unwrap();
    assert!(Original::open(&path, true, 64, None).is_err());
    std::fs::remove_file(linked).unwrap();
    let symbolic = path.with_extension("symbolic");
    symlink(&path, &symbolic).unwrap();
    assert!(Original::open(&symbolic, true, 64, None).is_err());
}
