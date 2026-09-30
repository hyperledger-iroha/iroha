//! Command custody I/O regression tests without inherited live runtime descriptors.

use super::*;
use std::{fs, io::Write as _, os::unix::fs::PermissionsExt as _};

#[test]
fn prepared_custody_retained_descriptor_is_nondestructive_and_rejects_shared_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("incumbent");
    let mut file = File::create(&path).unwrap();
    file.write_all(b"incumbent credential bytes").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        *read_retained_frame(File::open(&path).unwrap()).unwrap(),
        before
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::hard_link(&path, directory.path().join("alias")).unwrap();
    assert!(read_retained_frame(File::open(&path).unwrap()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_file(directory.path().join("alias")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(read_retained_frame(File::open(&path).unwrap()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn prepared_custody_publication_is_complete_exclusive_and_preserves_incumbent() {
    // Secure Directory intentionally rejects /tmp's writable ancestors.
    let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let parent = Directory::open(&fs::canonicalize(root.path()).unwrap()).unwrap();
    publish_generation(
        &parent,
        OsStr::new("generation-1"),
        b"credential-1",
        b"catalog-1",
        b"receipt-1",
    )
    .unwrap();
    assert_eq!(
        fs::read(root.path().join("generation-1").join(FILES[0])).unwrap(),
        b"credential-1"
    );
    assert_eq!(
        fs::metadata(root.path().join("generation-1").join(FILES[0]))
            .unwrap()
            .mode()
            & 0o7777,
        0o600
    );
    assert!(
        publish_generation(
            &parent,
            OsStr::new("generation-1"),
            b"credential-2",
            b"catalog-2",
            b"receipt-2"
        )
        .is_err()
    );
    assert_eq!(
        fs::read(root.path().join("generation-1").join(FILES[0])).unwrap(),
        b"credential-1"
    );
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        1,
        "failed staging is removed"
    );
    publish_generation(
        &parent,
        OsStr::new("generation-2"),
        b"credential-2",
        b"catalog-2",
        b"receipt-2",
    )
    .unwrap();
    for (name, expected) in
        FILES
            .iter()
            .zip([b"credential-2".as_slice(), b"catalog-2", b"receipt-2"])
    {
        assert_eq!(
            fs::read(root.path().join("generation-2").join(name)).unwrap(),
            expected
        );
    }
    assert!(
        publish_generation(
            &parent,
            OsStr::new("../escaped"),
            b"credential-3",
            b"catalog-3",
            b"receipt-3"
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn prepared_custody_cli_requires_external_pins_and_exposes_no_secret_argument() {
    use clap::CommandFactory as _;
    let help = Args::command().render_long_help().to_string();
    assert!(help.contains("--chain-id"));
    assert!(help.contains("--finality-allocated-bytes"));
    assert!(!help.contains("--trusted-context-id"));
    assert!(!help.contains("--anchor-height"));
    assert!(help.contains("--transition-id"));
    assert!(help.contains("--current-catalog"));
    assert!(!help.contains("--private-key"));
    assert!(!help.contains("--seed"));
    assert!(Args::try_parse_from(["beacon-prepare-custody", "--output", "/tmp/new"]).is_err());
}
